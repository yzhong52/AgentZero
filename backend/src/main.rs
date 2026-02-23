mod models;
mod store;
mod db;
mod images;
mod parsers;
mod api;

use axum::{
    Json, Router,
    extract::{Query, State, Path},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
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

pub(crate) const IMAGES_URL_PREFIX: &str = "/images";

#[derive(Clone)]
pub(crate) struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
    store: Arc<dyn object_store::ObjectStore>,
    /// Root directory where image files are written (local filesystem only).
    images_dir: String,
}

#[derive(Deserialize)]
struct NotesRequest {
    notes: Option<String>,
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

/// GET /api/parse?url=<url>
///
/// Fetches the given URL and runs all parsers, returning the raw parsed fields
/// (title, description, images, JSON-LD, meta tags). Does not write to the DB.
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

/// DELETE /api/listings/:id/images/:image_id
///
/// Removes a single cached image: deletes the file from the object store and
/// the row from `images_cache`. Silently removes the per-listing directory if
/// it becomes empty.
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
            .strip_prefix(&format!("{}/", IMAGES_URL_PREFIX))
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
            .strip_prefix(&format!("{}/", IMAGES_URL_PREFIX))
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

/// Updates user-tracked details for a listing. `id` is the property/listing ID.
/// Records a history entry if the price changed. Returns the updated property.
async fn patch_details(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<db::UserDetails>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    // Load the stored record once; used both as the merge base and for price-history comparison.
    let current = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // Merge every provided field from the request body over the stored values.
    // Fields absent from the body (None) are left unchanged.
    let mut updated = current.clone();

    updated.title = body.title.clone().unwrap_or(updated.title.clone());

    if body.redfin_url.is_some() { updated.redfin_url = body.redfin_url.clone(); }
    if body.realtor_url.is_some() { updated.realtor_url = body.realtor_url.clone(); }
    if body.rew_url.is_some() { updated.rew_url = body.rew_url.clone(); }
    if body.zillow_url.is_some() { updated.zillow_url = body.zillow_url.clone(); }

    updated.price = body.price.or(updated.price);
    updated.price_currency = body.price_currency.clone().or(updated.price_currency.clone());
    updated.offer_price = body.offer_price.or(updated.offer_price);
    updated.street_address = body.street_address.clone().or(updated.street_address.clone());
    updated.city = body.city.clone().or(updated.city.clone());
    updated.region = body.region.clone().or(updated.region.clone());
    updated.postal_code = body.postal_code.clone().or(updated.postal_code.clone());
    updated.bedrooms = body.bedrooms.or(updated.bedrooms);
    updated.bathrooms = body.bathrooms.or(updated.bathrooms);
    updated.sqft = body.sqft.or(updated.sqft);
    updated.year_built = body.year_built.or(updated.year_built);
    updated.parking_garage = body.parking_garage.or(updated.parking_garage);
    updated.parking_covered = body.parking_covered.or(updated.parking_covered);
    updated.parking_open = body.parking_open.or(updated.parking_open);
    updated.land_sqft = body.land_sqft.or(updated.land_sqft);
    updated.property_tax = body.property_tax.or(updated.property_tax);
    updated.skytrain_station = body.skytrain_station.clone().or(updated.skytrain_station.clone());
    updated.skytrain_walk_min = body.skytrain_walk_min.or(updated.skytrain_walk_min);
    updated.radiant_floor_heating = body.radiant_floor_heating.or(updated.radiant_floor_heating);
    updated.ac = body.ac.or(updated.ac);
    updated.down_payment_pct = body.down_payment_pct.or(updated.down_payment_pct);
    updated.mortgage_interest_rate = body.mortgage_interest_rate.or(updated.mortgage_interest_rate);
    updated.amortization_years = body.amortization_years.or(updated.amortization_years);
    updated.mortgage_monthly = body.mortgage_monthly.or(updated.mortgage_monthly);
    updated.hoa_monthly = body.hoa_monthly.or(updated.hoa_monthly);
    updated.monthly_total = body.monthly_total.or(updated.monthly_total);
    updated.monthly_cost = body.monthly_cost.or(updated.monthly_cost);
    updated.has_rental_suite = body.has_rental_suite.or(updated.has_rental_suite);
    updated.rental_income = body.rental_income.or(updated.rental_income);
    updated.status = body.status.clone().or(updated.status.clone());
    updated.school_elementary = body.school_elementary.clone().or(updated.school_elementary.clone());
    updated.school_elementary_rating = body.school_elementary_rating.or(updated.school_elementary_rating);
    updated.school_middle = body.school_middle.clone().or(updated.school_middle.clone());
    updated.school_middle_rating = body.school_middle_rating.or(updated.school_middle_rating);
    updated.school_secondary = body.school_secondary.clone().or(updated.school_secondary.clone());
    updated.school_secondary_rating = body.school_secondary_rating.or(updated.school_secondary_rating);

    let mut updated = db::update_by_id(&state.db, id, &updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // Recompute monthly_total/monthly_cost from the freshly saved values.
    updated.monthly_total = compute_monthly_total(updated.mortgage_monthly, updated.property_tax, updated.hoa_monthly);
    let base_price = updated.offer_price.or(updated.price);
    let initial_interest = base_price.map(|price| {
        compute_initial_monthly_interest(
            price,
            updated.down_payment_pct.unwrap_or(0.20),
            updated.mortgage_interest_rate.unwrap_or(0.05),
        )
    });
    updated.monthly_cost = compute_monthly_cost(initial_interest, updated.property_tax, updated.hoa_monthly);

    let _ = sqlx::query("UPDATE listings SET monthly_total = ?, monthly_cost = ? WHERE id = ?")
        .bind(updated.monthly_total)
        .bind(updated.monthly_cost)
        .bind(id)
        .execute(&state.db)
        .await;

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

/// GET /api/listings
///
/// Returns all saved properties, newest first. Each record includes cached
/// image metadata (id, local_path, position).
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
        .route("/api/parse",                          get(parse))
        // Listings collection
        .route("/api/listings",                       post(api::add::add_listing).get(list_listings))
        // Single listing
        .route("/api/listings/:id",                   get(get_listing))
        .route("/api/listings/:id/delete",            delete(delete_listing))
        .route("/api/listings/:id/refresh",           put(api::refresh::refresh_listing))
        .route("/api/listings/:id/preview",           get(api::refresh::preview_refresh))
        .route("/api/listings/:id/notes",             patch(patch_notes))
        .route("/api/listings/:id/details",           patch(patch_details))
        .route("/api/listings/:id/history",           get(get_history))
        .route("/api/listings/:id/images/:image_id",  delete(delete_image))
        // Static image files
        .nest_service("/images", ServeDir::new(&state.images_dir))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
