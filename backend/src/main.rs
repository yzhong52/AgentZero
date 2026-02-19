mod db;
mod images;

use axum::{
    Json, Router,
    extract::{Query, State, Path},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use object_store::{ObjectStoreExt, local::LocalFileSystem, path::Path as ObjectPath};
use std::sync::Arc;
use tower_http::services::ServeDir;
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

#[derive(Clone)]
struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
    /// Prefix for public image URLs. Local: "/images". S3: "https://bucket.s3…".
    images_url_prefix: String,
}

#[derive(Serialize)]
struct ParseResult {
    url: String,
    title: String,
    description: String,
    images: Vec<String>,
    raw_json_ld: Vec<JsonValue>,
    meta: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct SaveRequest {
    url: String,
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

fn extract_json_ld(document: &Html) -> Vec<JsonValue> {
    let selector = Selector::parse("script[type=\"application/ld+json\"]").unwrap();
    let mut out = Vec::new();
    for el in document.select(&selector) {
        if let Some(text) = el.first_child().and_then(|n| n.value().as_text()) {
            let s = text.trim();
            if s.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<JsonValue>(s) {
                if v.is_array() {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            out.push(item.clone());
                        }
                    }
                } else {
                    out.push(v);
                }
            }
        }
    }
    out
}

fn meta_map(document: &Html) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let selector = Selector::parse("meta").unwrap();
    for el in document.select(&selector) {
        let name = el
            .value()
            .attr("property")
            .or_else(|| el.value().attr("name"))
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if let Some(content) = el.value().attr("content") {
            m.insert(name.to_string(), content.to_string());
        }
    }
    m
}

fn extract_title(document: &Html) -> String {
    let og = Selector::parse("meta[property=\"og:title\"]").unwrap();
    if let Some(el) = document.select(&og).next() {
        if let Some(content) = el.value().attr("content") {
            return content.to_string();
        }
    }
    let title = Selector::parse("title").unwrap();
    if let Some(el) = document.select(&title).next() {
        return el.text().collect::<Vec<_>>().join("").trim().to_string();
    }
    String::new()
}

fn extract_description(document: &Html) -> String {
    let sel =
        Selector::parse("meta[property=\"og:description\"], meta[name=\"description\"]").unwrap();
    if let Some(el) = document.select(&sel).next() {
        if let Some(content) = el.value().attr("content") {
            return content.to_string();
        }
    }
    String::new()
}

fn extract_images(document: &Html) -> Vec<String> {
    let sel = Selector::parse("meta[property=\"og:image\"]").unwrap();
    let mut out = Vec::new();
    for el in document.select(&sel) {
        if let Some(content) = el.value().attr("content") {
            out.push(content.to_string());
        }
    }
    out
}

/// Extracts structured property fields from JSON-LD blocks.
/// Looks for the item whose @type includes "RealEstateListing".
/// `images` is always left empty here — call `extract_image_urls` separately.
fn extract_property(url: &str, title: &str, json_ld: &[JsonValue]) -> db::Property {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    });

    let mut p = db::Property {
        id: 0,
        url: url.to_string(),
        title: title.to_string(),
        description: String::new(),
        price: None,
        price_currency: None,
        street_address: None,
        city: None,
        region: None,
        postal_code: None,
        country: None,
        bedrooms: None,
        bathrooms: None,
        sqft: None,
        year_built: None,
        lat: None,
        lon: None,
        images: Vec::new(),
        created_at: String::new(),
    };

    let Some(listing) = listing else {
        return p;
    };

    // description
    if let Some(d) = listing["description"].as_str() {
        p.description = d.to_string();
    }

    // offers
    p.price = listing["offers"]["price"].as_i64();
    p.price_currency = listing["offers"]["priceCurrency"].as_str().map(str::to_string);

    let entity = &listing["mainEntity"];

    // address
    let addr = &entity["address"];
    p.street_address = addr["streetAddress"].as_str().map(str::to_string);
    p.city = addr["addressLocality"].as_str().map(str::to_string);
    p.region = addr["addressRegion"].as_str().map(str::to_string);
    p.postal_code = addr["postalCode"].as_str().map(str::to_string);
    p.country = addr["addressCountry"].as_str().map(str::to_string);

    // rooms
    p.bedrooms = entity["numberOfBedrooms"].as_i64();
    p.bathrooms = entity["numberOfBathroomsTotal"].as_i64();
    p.sqft = entity["floorSize"]["value"].as_i64();
    p.year_built = entity["yearBuilt"].as_i64();

    // geo
    p.lat = entity["geo"]["latitude"].as_f64();
    p.lon = entity["geo"]["longitude"].as_f64();

    p
}

/// Extracts image source URLs from mainEntity.image[] in the RealEstateListing JSON-LD block.
fn extract_image_urls(json_ld: &[JsonValue]) -> Vec<String> {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    });
    listing
        .and_then(|l| l["mainEntity"]["image"].as_array())
        .map(|imgs| {
            imgs.iter()
                .filter_map(|img| img["url"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
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

    let property = extract_property(parsed.as_str(), &title, &json_ld);
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

    let mut updated = extract_property(parsed.as_str(), &title, &json_ld);
    updated.id = id;
    updated.url = url.to_string();
    let image_urls = extract_image_urls(&json_ld);

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
    use super::*;
    use scraper::Html;

    #[test]
    fn snapshot_redfin_3662_oak_st() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);

        let result = ParseResult {
            url: "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332"
                .to_string(),
            title: extract_title(&document),
            description: extract_description(&document),
            images: extract_images(&document),
            raw_json_ld: extract_json_ld(&document),
            meta: meta_map(&document),
        };

        insta::assert_json_snapshot!(result);
    }

    #[test]
    fn snapshot_extract_property() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);
        let json_ld = extract_json_ld(&document);
        let title = extract_title(&document);
        let url = "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332";

        let image_urls = extract_image_urls(&json_ld);
        let images: Vec<db::ImageEntry> = image_urls
            .into_iter()
            .enumerate()
            .map(|(i, url)| db::ImageEntry { id: i as i64, url, created_at: String::new() })
            .collect();
        let property = db::Property {
            images,
            ..extract_property(url, &title, &json_ld)
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
        .route("/api/listings/:id", put(refresh_listing))
        .route("/api/listings/:id/images/:image_id", delete(delete_image))
        .nest_service("/images", ServeDir::new(&images_dir))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
