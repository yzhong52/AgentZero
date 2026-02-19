mod models;
mod store;
mod db;
mod images;
mod parser;

use axum::{
    Json, Router,
    extract::{Query, State, Path},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
};
use object_store::{ObjectStoreExt, local::LocalFileSystem, path::Path as ObjectPath};
use std::sync::Arc;
use tower_http::services::ServeDir;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::Html;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

use parser::{
    ParseResult, extract_description, extract_image_urls, extract_images, extract_json_ld,
    extract_lot_size, extract_property, extract_title, meta_map,
};

#[derive(Clone)]
struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
    /// Prefix for public image URLs. Local: "/images". S3: "https://bucket.s3…".
    images_url_prefix: String,
}

#[derive(Deserialize)]
struct SaveRequest {
    url: String,
}

#[derive(Deserialize)]
struct NotesRequest {
    notes: Option<String>,
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

    // `Html` is !Send, so extract everything in a block and drop it before the next await.
    let (json_ld, title) = {
        let document = Html::parse_document(&html);
        (extract_json_ld(&document), extract_title(&document))
    };

    let mut property = extract_property(parsed.as_str(), &title, &json_ld)
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "No RealEstateListing found in page".to_string()))?;
    property.land_sqft = extract_lot_size(&html);
    let image_urls = extract_image_urls(&json_ld);

    let saved = db::save(&state.db, &property)
        .await
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
    // Fetch existing listing to get the URL
    let listings = db::list(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    let property = listings
        .iter()
        .find(|p| p.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Listing not found".to_string()))?;

    let url = &property.url;
    let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL in listing".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    // Extract everything in a block
    let (json_ld, title) = {
        let document = Html::parse_document(&html);
        (extract_json_ld(&document), extract_title(&document))
    };

    let mut updated = extract_property(parsed.as_str(), &title, &json_ld)
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "No RealEstateListing found in page".to_string()))?;
    updated.land_sqft = extract_lot_size(&html);
    updated.id = id;
    updated.url = url.to_string();
    let image_urls = extract_image_urls(&json_ld);

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

/// Deletes a listing and all its images. `id` is the property/listing ID.
async fn delete_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    db::delete(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Updates user-tracked details for a listing. `id` is the property/listing ID.
async fn patch_details(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<db::UserDetails>,
) -> Result<StatusCode, (StatusCode, String)> {
    db::update_details(&state.db, id, &body)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
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
    use crate::{db, parser};

    #[test]
    fn snapshot_redfin_3662_oak_st() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);

        let result = parser::ParseResult {
            url: "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332"
                .to_string(),
            title: parser::extract_title(&document),
            description: parser::extract_description(&document),
            images: parser::extract_images(&document),
            raw_json_ld: parser::extract_json_ld(&document),
            meta: parser::meta_map(&document),
        };

        insta::assert_json_snapshot!(result);
    }

    #[test]
    fn snapshot_extract_property() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);
        let json_ld = parser::extract_json_ld(&document);
        let title = parser::extract_title(&document);
        let url = "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332";

        let image_urls = parser::extract_image_urls(&json_ld);
        let images: Vec<db::ImageEntry> = image_urls
            .into_iter()
            .enumerate()
            .map(|(i, url)| db::ImageEntry { id: i as i64, url, created_at: String::new() })
            .collect();
        let property = db::Property {
            images,
            ..parser::extract_property(url, &title, &json_ld).unwrap()
        };

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
        .route("/api/listings/:id", put(refresh_listing).delete(delete_listing))
        .route("/api/listings/:id/notes", patch(patch_notes))
        .route("/api/listings/:id/details", patch(patch_details))
        .route("/api/listings/:id/history", get(get_history))
        .route("/api/listings/:id/images/:image_id", delete(delete_image))
        .nest_service("/images", ServeDir::new(&images_dir))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
