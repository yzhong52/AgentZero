mod api;
mod db;
mod image_paths;
mod images;
mod models;
mod parsers;
mod store;

use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use object_store::local::LocalFileSystem;
use rquest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, REFERER, USER_AGENT,
};
use rquest::{Client, Impersonate};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use url::Url;

use agent_zero_backend::{IMAGES_LOCAL_DIR, IMAGES_URL_PREFIX};

#[derive(Clone)]
pub(crate) struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
}

/// Sums mortgage + monthly property tax + HOA into a total monthly cost.
pub(crate) fn compute_monthly_total(
    mortgage_monthly: Option<i64>,
    property_tax_annual: Option<i64>,
    hoa_monthly: Option<i64>,
) -> Option<i64> {
    let mortgage_monthly = mortgage_monthly?; // require at least a mortgage payment
    let property_tax_monthly = property_tax_annual.map(|t| t / 12).unwrap_or(0);
    let hoa_monthly = hoa_monthly.unwrap_or(0);
    Some(mortgage_monthly + property_tax_monthly + hoa_monthly)
}

/// Initial monthly mortgage interest only (principal * annual_rate / 12).
pub(crate) fn compute_initial_monthly_interest(price: i64, down_pct: f64, annual_rate: f64) -> i64 {
    let loan = price as f64 * (1.0 - down_pct);
    if loan <= 0.0 {
        return 0;
    }
    ((loan * annual_rate) / 12.0).round() as i64
}

/// Sums initial monthly interest + monthly property tax + HOA.
pub(crate) fn compute_monthly_cost(
    initial_monthly_interest: Option<i64>,
    property_tax_annual: Option<i64>,
    hoa_monthly: Option<i64>,
) -> Option<i64> {
    let initial_monthly_interest = initial_monthly_interest?;
    let property_tax_monthly = property_tax_annual.map(|t| t / 12).unwrap_or(0);
    let hoa_monthly = hoa_monthly.unwrap_or(0);
    Some(initial_monthly_interest + property_tax_monthly + hoa_monthly)
}

/// Standard amortisation formula: monthly payment on a fixed-rate mortgage.
/// Returns 0 if price is 0 or rate is 0 (handled gracefully).
pub(crate) fn compute_mortgage(price: i64, down_pct: f64, annual_rate: f64, years: i64) -> i64 {
    let loan = price as f64 * (1.0 - down_pct);
    if loan <= 0.0 {
        return 0;
    }
    let n = (years * 12) as f64;
    if annual_rate == 0.0 {
        return (loan / n).round() as i64;
    }
    let r = annual_rate / 12.0;
    let payment = loan * r * (1.0 + r).powf(n) / ((1.0 + r).powf(n) - 1.0);
    payment.round() as i64
}

pub(crate) fn safe_url(input: &str) -> Option<Url> {
    if let Ok(u) = Url::parse(input) {
        match u.scheme() {
            "http" | "https" => Some(u),
            _ => None,
        }
    } else {
        None
    }
}

/// Fetch HTML using the rquest HTTP client (with browser TLS impersonation).
async fn fetch_html_direct(client: &Client, url: &Url) -> Result<String, rquest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br"),
    );
    if let Ok(rv) = HeaderValue::from_str(url.as_str()) {
        headers.insert(REFERER, rv);
    }

    let resp = client.get(url.as_str()).headers(headers).send().await?;
    resp.error_for_status_ref()?;
    resp.text().await
}

/// Fetch HTML by opening the URL in Safari via AppleScript.
///
/// Safari passes bot-protection checks (Incapsula, PerimeterX) that block
/// plain HTTP clients because it runs the full JS challenge in a real browser
/// context.  The page is opened in a new tab, allowed to settle for ~20 s,
/// and the rendered DOM source is returned.
async fn fetch_html_safari(url: &Url) -> Result<String, String> {
    let script = format!(
        r#"
tell application "Safari"
    activate
    make new document with properties {{URL:"{url}"}}
    delay 20
    set pageSource to source of document 1
    close document 1
    return pageSource
end tell
"#,
        url = url.as_str()
    );
    let output = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .await
        .map_err(|e| format!("osascript failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript error: {stderr}"));
    }
    let html = String::from_utf8_lossy(&output.stdout).to_string();
    if html.len() < 2000 {
        return Err(format!("Safari returned only {} bytes (likely blocked)", html.len()));
    }
    Ok(html)
}

/// Returns `true` when the HTML looks like a bot-protection challenge page
/// rather than real listing content.
fn is_challenge_page(html: &str) -> bool {
    html.len() < 2000
        && (html.contains("Incapsula")
            || html.contains("_Incapsula_Resource")
            || html.contains("px-blocked")
            || html.contains("PerimeterX"))
}

/// Returns `true` for hosts known to use aggressive bot protection that
/// blocks plain HTTP clients (even with TLS impersonation).
fn is_bot_protected_host(url: &Url) -> bool {
    match url.host_str().unwrap_or("") {
        h if h.contains("zillow.com") => true,
        h if h.contains("realtor.ca") => true,
        _ => false,
    }
}

/// Fetch HTML for a listing URL.
///
/// Strategy:
/// 1. Try the fast `rquest` HTTP client (with Chrome TLS impersonation).
/// 2. If that fails with a 403 or returns a bot-challenge page for a known
///    protected host, fall back to Safari via AppleScript.
pub(crate) async fn fetch_html(client: &Client, url: &Url) -> Result<String, String> {
    // Fast path: direct HTTP fetch.
    match fetch_html_direct(client, url).await {
        Ok(html) if !is_challenge_page(&html) => return Ok(html),
        Ok(html) if !is_bot_protected_host(url) => return Ok(html),
        Ok(_challenge) => {
            tracing::info!(
                "fetch_html: direct fetch returned challenge page for {}, trying Safari",
                url
            );
        }
        Err(e) if is_bot_protected_host(url) => {
            tracing::info!(
                "fetch_html: direct fetch failed for {} ({}), trying Safari",
                url,
                e
            );
        }
        Err(e) => return Err(format!("Failed to fetch {url}: {e}")),
    }

    // Slow path: Safari via AppleScript (macOS only).
    fetch_html_safari(url).await
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://listings.db".to_string());
    let db = db::init(&database_url).await;

    // Local filesystem store.
    images::ensure_images_dir(IMAGES_LOCAL_DIR).await;
    let store: Arc<dyn object_store::ObjectStore> = Arc::new(
        LocalFileSystem::new_with_prefix(std::path::Path::new(IMAGES_LOCAL_DIR))
            .expect("Failed to initialize local image store"),
    );

    let client = Client::builder()
        .impersonate(Impersonate::Chrome130)
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let state = AppState { db, client, store };

    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<axum::http::HeaderValue>()
                .unwrap(),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Utility
        .route("/api/parse", get(api::parse::parse))
        // Listings collection
        .route(
            "/api/listings",
            post(api::add::add_listing).get(api::listings::list_listings),
        )
        // Single listing
        .route("/api/listings/:id", get(api::listings::get_listing))
        .route(
            "/api/listings/:id/delete",
            delete(api::listings::delete_listing),
        )
        .route(
            "/api/listings/:id/refresh",
            put(api::refresh::refresh_listing),
        )
        .route(
            "/api/listings/:id/preview",
            get(api::refresh::preview_refresh),
        )
        .route("/api/listings/:id/notes", patch(api::details::patch_notes))
        .route(
            "/api/listings/:id/details",
            patch(api::details::patch_details),
        )
        .route("/api/listings/:id/history", get(api::details::get_history))
        .route(
            "/api/listings/:id/images/:image_id",
            delete(api::images::delete_image),
        )
        // Static image files
        .nest_service(IMAGES_URL_PREFIX, ServeDir::new(IMAGES_LOCAL_DIR))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
