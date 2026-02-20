mod models;
mod store;
mod db;
mod images;
mod parsers;

use axum::{
    Json, Router,
    extract::{Query, State, Path},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use object_store::{ObjectStoreExt, local::LocalFileSystem, path::Path as ObjectPath};
use std::sync::Arc;
use tokio::fs;
use tower_http::services::ServeDir;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::Html;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

use url::Url;
use parsers::{ParseResult, extract_description, extract_images, extract_json_ld, extract_title, meta_map};

#[derive(Clone)]
struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
    /// Prefix for public image URLs. Local: "/images". S3: "https://bucket.s3…".
    images_url_prefix: String,
    /// Root directory where image files are written (local filesystem only).
    images_dir: String,
}

#[derive(Deserialize)]
struct SaveRequest {
    url: String,
}

#[derive(Deserialize)]
struct NotesRequest {
    notes: Option<String>,
}

#[derive(Deserialize)]
struct NicknameRequest {
    nickname: Option<String>,
}

/// Standard amortisation formula: monthly payment on a fixed-rate mortgage.
/// Returns 0 if price is 0 or rate is 0 (handled gracefully).
fn compute_mortgage(price: i64, down_pct: f64, annual_rate: f64, years: i64) -> i64 {
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

fn safe_url(input: &str) -> Option<Url> {
    if let Ok(u) = Url::parse(input) {
        match u.scheme() {
            "http" | "https" => Some(u),
            _ => None,
        }
    } else {
        None
    }
}

async fn fetch_html(client: &Client, url: &Url) -> Result<String, reqwest::Error> {
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

async fn parse(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ParseResult>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or((StatusCode::BAD_REQUEST, "Missing 'url' query parameter".to_string()))?;
    let url = url.trim();
    let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    let document = Html::parse_document(&html);
    let json_ld = extract_json_ld(&document);
    let meta = meta_map(&document);
    let title = extract_title(&document);
    let description = extract_description(&document);
    let images = extract_images(&document);

    Ok(Json(ParseResult {
        url: parsed.to_string(),
        title,
        description,
        images,
        raw_json_ld: json_ld,
        meta,
    }))
}

async fn save_listing(
    State(state): State<AppState>,
    Json(body): Json<SaveRequest>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let url = body.url.trim();
    let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    let listing = parsers::parse(parsed.as_str(), &html)
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "No recognized listing format found in page".to_string()))?;
    let mut property = listing.property;
    let image_urls = listing.image_urls;

    // Auto-calculate mortgage with defaults on first save.
    let down_pct = 0.20_f64;
    let rate     = 0.04_f64;
    let years    = 25_i64;
    property.down_payment_pct       = Some(down_pct);
    property.mortgage_interest_rate = Some(rate);
    property.amortization_years     = Some(years);
    if let Some(price) = property.price {
        property.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }

    let saved = if property.realtor_url.is_some() {
        db::save_realtor(&state.db, &property).await
    } else if property.rew_url.is_some() {
        db::save_rew(&state.db, &property).await
    } else {
        db::save_redfin(&state.db, &property).await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // Register image URLs in images_cache, preserving parser ordering.
    for (position, url) in image_urls.iter().enumerate() {
        let _ = db::insert_image_url(&state.db, saved.id, url, position as i64).await;
    }

    // Download any pending images.
    images::cache_images(
        &state.db,
        &state.client,
        state.store.as_ref(),
        saved.id,
        &state.images_url_prefix,
    )
    .await;

    let images = db::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(db::Property { images, ..saved }))
}

/// Refreshes a saved listing by re-fetching it from source. `id` is the property/listing ID.
async fn refresh_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let property = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    let url = property.redfin_url.clone()
        .or_else(|| property.realtor_url.clone())
        .or_else(|| property.rew_url.clone())
        .ok_or((StatusCode::BAD_REQUEST, "No source URL stored for this listing".to_string()))?;
    let parsed = safe_url(&url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL in listing".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    let listing = parsers::parse(parsed.as_str(), &html)
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "No recognized listing format found in page".to_string()))?;
    let mut updated = listing.property;
    updated.id = id;
    // Preserve all source URLs from the stored property.
    updated.redfin_url = property.redfin_url.clone();
    updated.realtor_url = property.realtor_url.clone();
    updated.rew_url = property.rew_url.clone();
    let image_urls = listing.image_urls;

    // Preserve the user's mortgage parameters; re-calculate monthly payment.
    let down_pct = property.down_payment_pct.unwrap_or(0.20);
    let rate     = property.mortgage_interest_rate.unwrap_or(0.04);
    let years    = property.amortization_years.unwrap_or(25);
    updated.down_payment_pct       = Some(down_pct);
    updated.mortgage_interest_rate = Some(rate);
    updated.amortization_years     = Some(years);
    if let Some(price) = updated.price {
        updated.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }

    // Record price change history before overwriting.
    if property.price != updated.price {
        let old = property.price.map(|v| v.to_string());
        let new = updated.price.map(|v| v.to_string());
        let _ = db::insert_change(&state.db, id, "price", old.as_deref(), new.as_deref()).await;
    }

    let saved = db::update_by_id(&state.db, id, &updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // Update image URLs in images_cache
    for (position, url) in image_urls.iter().enumerate() {
        let _ = db::insert_image_url(&state.db, id, url, position as i64).await;
    }

    // Download any pending images
    images::cache_images(
        &state.db,
        &state.client,
        state.store.as_ref(),
        id,
        &state.images_url_prefix,
    )
    .await;

    let images = db::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(db::Property { images, ..saved }))
}

async fn delete_image(
    State(state): State<AppState>,
    Path((listing_id, image_id)): Path<(i64, i64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Verify the image exists and belongs to this listing; get its local_path.
    let local_path = db::get_image_local_path(&state.db, image_id, listing_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?
        .ok_or((StatusCode::NOT_FOUND, "Image not found".to_string()))?;

    // Delete the file from the object store when it was successfully downloaded.
    if let Some(path) = local_path {
        // path looks like "/images/1/abc123.jpg"; strip prefix to get object key.
        let object_key = path
            .strip_prefix(&format!("{}/", state.images_url_prefix))
            .unwrap_or(&path);
        if let Err(e) = state.store.delete(&ObjectPath::from(object_key)).await {
            tracing::warn!("Failed to delete image file {}: {}", object_key, e);
            // Proceed to remove the DB record even if file deletion fails.
        }
    }

    db::delete_image_record(&state.db, image_id, listing_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // If no images remain for this listing, remove the empty per-listing directory.
    let dir = format!("{}/{}", state.images_dir, listing_id);
    if let Err(e) = fs::remove_dir(&dir).await {
        // Not empty (other images remain) or already gone — both are fine.
        tracing::debug!("Could not remove image dir {}: {}", dir, e);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Updates the personal notes for a listing. `id` is the property/listing ID.
async fn patch_notes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NotesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    db::update_notes(&state.db, id, body.notes.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a listing: removes image files from the object store, clears the
/// images_cache rows, then removes the listing row itself.
/// `id` is the property/listing ID.
async fn delete_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // 1. Delete locally-cached image files from the object store.
    let cached = db::list_cached_images(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    for img in &cached {
        let object_key = img.local_path
            .strip_prefix(&format!("{}/", state.images_url_prefix))
            .unwrap_or(&img.local_path);
        if let Err(e) = state.store.delete(&ObjectPath::from(object_key)).await {
            tracing::warn!("delete_listing: could not remove image file {}: {}", object_key, e);
            // Continue — file may already be gone; don't block the delete.
        }
    }

    // 2. Remove the per-listing image directory (now empty after step 1).
    let dir = format!("{}/{}", state.images_dir, id);
    if let Err(e) = fs::remove_dir(&dir).await {
        tracing::debug!("delete_listing: could not remove image dir {}: {}", dir, e);
    }

    // 3. Remove images_cache rows (no CASCADE on this FK).
    db::delete_all_image_records(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // 4. Delete the listing row (listing_history cascades automatically).
    db::delete(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Updates the nickname for a listing. `id` is the property/listing ID.
async fn patch_nickname(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NicknameRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    db::update_nickname(&state.db, id, body.nickname.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Returns a single listing by ID.
async fn get_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let p = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;
    Ok(Json(p))
}

/// Fetches and parses a listing without saving — used for the refresh diff preview.
async fn preview_refresh(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let stored = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    let source_url = stored.redfin_url.clone()
        .or_else(|| stored.realtor_url.clone())
        .or_else(|| stored.rew_url.clone())
        .ok_or((StatusCode::BAD_REQUEST, "No source URL stored for this listing".to_string()))?;
    let parsed_url = safe_url(&source_url)
        .ok_or((StatusCode::BAD_REQUEST, "Invalid URL in listing".to_string()))?;

    let html = fetch_html(&state.client, &parsed_url)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    let listing = parsers::parse(parsed_url.as_str(), &html)
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "No recognized listing format found in page".to_string()))?;

    // Return the parsed result without saving; mortgage params carried from stored.
    let mut preview = listing.property;
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate     = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years    = stored.amortization_years.unwrap_or(25);
    preview.down_payment_pct       = Some(down_pct);
    preview.mortgage_interest_rate = Some(rate);
    preview.amortization_years     = Some(years);
    if let Some(price) = preview.price {
        preview.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }

    Ok(Json(preview))
}

/// Updates user-tracked details for a listing. `id` is the property/listing ID.
/// Records a history entry if the price changed. Returns the updated property.
async fn patch_details(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<db::UserDetails>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    // Fetch current price before overwriting (for history).
    let current = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    let updated = db::update_details(&state.db, id, &body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    if current.price != updated.price {
        let old = current.price.map(|v| v.to_string());
        let new = updated.price.map(|v| v.to_string());
        let _ = db::insert_change(&state.db, id, "price", old.as_deref(), new.as_deref()).await;
    }

    // Re-attach images (update_details doesn't load them).
    let images = db::list_images_with_meta(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(db::Property { images, ..updated }))
}

/// Returns price/field change history for a listing. `id` is the property/listing ID.
async fn get_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<db::HistoryEntry>>, (StatusCode, String)> {
    let entries = db::list_history(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(Json(entries))
}

async fn list_listings(
    State(state): State<AppState>,
) -> Result<Json<Vec<db::Property>>, (StatusCode, String)> {
    let listings = db::list(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(listings))
}

#[cfg(test)]
mod tests {
    use scraper::Html;
    use crate::{db, parsers};

    #[test]
    fn snapshot_redfin_3662_oak_st() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);

        let result = parsers::ParseResult {
            url: "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332"
                .to_string(),
            title: parsers::extract_title(&document),
            description: parsers::extract_description(&document),
            images: parsers::extract_images(&document),
            raw_json_ld: parsers::extract_json_ld(&document),
            meta: parsers::meta_map(&document),
        };

        insta::assert_json_snapshot!(result);
    }

    #[test]
    fn snapshot_extract_property() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let url = "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332";

        let listing = parsers::redfin::parse(url, &html).expect("parse failed");
        let images: Vec<db::ImageEntry> = listing.image_urls
            .into_iter()
            .enumerate()
            .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
            .collect();
        let property = db::Property { images, ..listing.property };

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
    let images_url_prefix =
        std::env::var("IMAGES_URL_PREFIX").unwrap_or_else(|_| "/images".to_string());

    let db = db::init(&database_url).await;

    // Local filesystem store — swap for AmazonS3::new() / GoogleCloudStorage::new()
    // when ready to move to cloud. Set IMAGES_URL_PREFIX to the bucket's public URL.
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
        images_url_prefix,
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
        .route("/api/parse", get(parse))
        .route("/api/listings", post(save_listing).get(list_listings))
        .route("/api/listings/:id", get(get_listing).put(refresh_listing).delete(delete_listing))
        .route("/api/listings/:id/preview", get(preview_refresh))
        .route("/api/listings/:id/notes", patch(patch_notes))
        .route("/api/listings/:id/nickname", patch(patch_nickname))
        .route("/api/listings/:id/details", patch(patch_details))
        .route("/api/listings/:id/history", get(get_history))
        .route("/api/listings/:id/images/:image_id", delete(delete_image))
        .nest_service("/images", ServeDir::new(&state.images_dir))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
