mod models;
mod store;
mod db;
mod images;
mod parsers;
mod api;

use axum::{Router, routing::{delete, get, patch, post, put}};
use object_store::local::LocalFileSystem;
use std::sync::Arc;
use tower_http::services::ServeDir;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

pub(crate) const IMAGES_URL_PREFIX: &str = "/images";

#[derive(Clone)]
pub(crate) struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
    /// Root directory where image files are written (local filesystem only).
    images_dir: String,
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
    if loan <= 0.0 { return 0; }
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
    if loan <= 0.0 { return 0; }
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

pub(crate) async fn fetch_html(client: &Client, url: &Url) -> Result<String, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br"));
    if let Ok(rv) = HeaderValue::from_str(url.as_str()) {
        headers.insert(REFERER, rv);
    }

    let resp = client.get(url.as_str()).headers(headers).send().await?;
    resp.error_for_status_ref()?;
    resp.text().await
}


#[cfg(test)]
mod tests {
    use crate::{db, parsers};

    #[test]
    fn snapshot_extract_property_829_e14th() {
        let html = std::fs::read_to_string("fixtures/829 E 14th Ave, Vancouver, BC V5T 2N5 _ MLS# R3090427 _ Redfin.html")
            .expect("fixture not found — run from backend/");
        let url = "https://www.redfin.ca/bc/vancouver/829-E-14th-Ave-V5T-2N5/home/155809679";

        let listing = parsers::redfin::parse(url, &html).expect("parse failed");
        let images: Vec<db::ImageEntry> = listing.image_urls
            .into_iter()
            .enumerate()
            .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
            .collect();
        let property = db::Property { images, ..listing.property };

        insta::assert_json_snapshot!(property);
    }

    #[test]
    fn snapshot_redfin_788_w8th_unit_l01() {
        let html = std::fs::read_to_string("fixtures/788 W 8th Ave Unit L01, Vancouver, BC V5Z 1E1 _ MLS# R3086230 _ Redfin.html")
            .expect("fixture not found — run from backend/");
        let url = "https://www.redfin.ca/bc/vancouver/788-W-8th-Ave-V5Z-1E1/home/";

        let listing = parsers::redfin::parse(url, &html).expect("parse failed");
        let images: Vec<db::ImageEntry> = listing.image_urls
            .into_iter()
            .enumerate()
            .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
            .collect();
        let property = db::Property { images, ..listing.property };

        // HOA must be present for this condo listing
        assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
        insta::assert_json_snapshot!(property);
    }

    #[test]
    fn snapshot_rew_788_w8th_unit_l01() {
        let html = std::fs::read_to_string("fixtures/For Sale_ L01-788 W 8th Avenue, Vancouver, BC - REW.html")
            .expect("fixture not found — run from backend/");
        let url = "https://www.rew.ca/properties/l01-788-w-8th-avenue-vancouver-bc";

        let listing = parsers::rew::parse(url, &html).expect("parse failed");
        let images: Vec<db::ImageEntry> = listing.image_urls
            .into_iter()
            .enumerate()
            .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
            .collect();
        let property = db::Property { images, ..listing.property };

        // HOA (strata maintenance fee) must be present for this condo listing
        assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
        insta::assert_json_snapshot!(property);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://listings.db".to_string());
    let images_dir =
        std::env::var("IMAGES_DIR").unwrap_or_else(|_| "listings_images".to_string());
    let db = db::init(&database_url).await;

    // Local filesystem store.
    images::ensure_images_dir(&images_dir).await;
    let store: Arc<dyn object_store::ObjectStore> = Arc::new(
        LocalFileSystem::new_with_prefix(std::path::Path::new(&images_dir))
            .expect("Failed to initialize local image store"),
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let state = AppState {
        db,
        client,
        store,
        images_dir,
    };

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
        .route("/api/parse",                          get(api::parse::parse))
        // Listings collection
        .route("/api/listings",                       post(api::add::add_listing).get(api::listings::list_listings))
        // Single listing
        .route("/api/listings/:id",                   get(api::listings::get_listing))
        .route("/api/listings/:id/delete",            delete(api::listings::delete_listing))
        .route("/api/listings/:id/refresh",           put(api::refresh::refresh_listing))
        .route("/api/listings/:id/preview",           get(api::refresh::preview_refresh))
        .route("/api/listings/:id/notes",             patch(api::details::patch_notes))
        .route("/api/listings/:id/details",           patch(api::details::patch_details))
        .route("/api/listings/:id/history",           get(api::details::get_history))
        .route("/api/listings/:id/images/:image_id",  delete(api::images::delete_image))
        // Static image files
        .nest_service("/images", ServeDir::new(&state.images_dir))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
