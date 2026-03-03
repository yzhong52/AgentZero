//! POST /api/listings — add a new listing.
//!
//! Fetches and parses the given listing URL, saves the result to the DB,
//! downloads images, and returns the saved record.
//!
//! # Supported parsers
//!
//! | Source      | Status  | Notes                                      |
//! |-------------|---------|--------------------------------------------|  
//! | Redfin      | ✅ Works | Primary source; best structured data       |
//! | REW.ca      | ✅ Works | Good supplement; includes property tax     |
//! | Zillow      | ❌ Blocked | PerimeterX / CloudFront (403)            |
//! | Realtor.ca  | ❌ Blocked | Imperva Incapsula                        |
//!
//! # Blocked-host handling
//!
//! When the submitted URL is from a known-blocked host (Zillow, Realtor.ca),
//! a stub listing is saved containing only the URL so the user can fill in
//! details manually via the edit panel.

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use url::Url;

use crate::{
    compute_initial_monthly_interest, compute_monthly_cost, compute_monthly_total,
    compute_mortgage, db, fetch_html, images, parsers, safe_url, AppState,
};

#[derive(Deserialize)]
pub struct AddRequest {
    /// The listing URL to add.
    pub url: String,
    /// Search to assign this listing to.
    pub search_criteria_id: i64,
}

pub async fn add_listing(
    State(state): State<AppState>,
    Json(body): Json<AddRequest>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let mut parsed_url = safe_url(body.url.trim()).ok_or((
        StatusCode::BAD_REQUEST,
        format!("Invalid URL: {}", body.url.trim()),
    ))?;
    parsed_url.set_query(None);

    // Check for duplicate source URLs before fetching.
    if let Ok(Some(existing)) = db::find_by_source_url(&state.db, parsed_url.as_str()).await {
        let body = serde_json::json!({
            "duplicate": true,
            "existing_id": existing.id,
            "existing_title": existing.title,
            "mls_number": existing.mls_number,
        });
        return Err((StatusCode::CONFLICT, body.to_string()));
    }

    // Fetch HTML.
    // `fetch_html` tries a direct HTTP request first.  For bot-protected
    // hosts (Zillow, Realtor.ca) it automatically falls back to Safari via
    // AppleScript.  If even that fails, we save an empty stub so the user
    // can fill in details manually.
    let source = match fetch_html(&state.client, &parsed_url).await {
        Ok(html) => parsers::SourceInput {
            url: parsed_url.to_string(),
            html,
        },
        Err(e) if is_blocked_host(&parsed_url) => {
            tracing::info!(
                "add_listing: fetch failed for {} ({}), saving stub",
                parsed_url,
                e
            );
            parsers::SourceInput {
                url: parsed_url.to_string(),
                html: String::new(),
            }
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch {}: {}", parsed_url, e),
            ))
        }
    };

    let listing_opt = parsers::parse_multi(std::slice::from_ref(&source));

    let (mut property, image_urls, open_houses) = match listing_opt {
        Some(listing) => (listing.property, listing.image_urls, listing.open_houses),
        None => {
            // Parsing yielded nothing.  Only save a stub for known-blocked hosts;
            // for unrecognised URLs return 422.
            if !is_blocked_host(&parsed_url) {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "No recognized listing format found in page".to_string(),
                ));
            }
            tracing::info!(
                "add_listing: all URLs are from blocked hosts, saving stub for {:?}",
                parsed_url.as_str(),
            );
            let mut stub = blank_stub();
            // Populate whichever URL fields we know about.
            match parsed_url.host_str().unwrap_or("") {
                h if h.contains("zillow.com") => stub.zillow_url = Some(parsed_url.to_string()),
                h if h.contains("realtor.ca") => stub.realtor_url = Some(parsed_url.to_string()),
                _ => {}
            }
            (stub, vec![], vec![])
        }
    };

    // Auto-calculate mortgage with defaults on first save.
    let down_pct = property.down_payment_pct.unwrap_or(0.20);
    let rate = property.mortgage_interest_rate.unwrap_or(0.04);
    let years = property.amortization_years.unwrap_or(25);
    property.down_payment_pct = Some(down_pct);
    property.mortgage_interest_rate = Some(rate);
    property.amortization_years = Some(years);
    let base_price = property.offer_price.or(property.price);
    if let Some(price) = base_price {
        property.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }
    property.monthly_total = compute_monthly_total(
        property.mortgage_monthly,
        property.property_tax,
        property.hoa_monthly,
    );
    let initial_interest = base_price.map(|p| compute_initial_monthly_interest(p, down_pct, rate));
    property.monthly_cost = compute_monthly_cost(
        initial_interest,
        property.property_tax,
        property.hoa_monthly,
    );

    // Check for duplicate MLS number before inserting.
    if let Some(ref mls) = property.mls_number {
        if let Ok(Some(existing)) = db::find_by_mls(&state.db, mls).await {
            let body = serde_json::json!({
                "duplicate": true,
                "existing_id": existing.id,
                "existing_title": existing.title,
                "mls_number": mls,
            });
            return Err((StatusCode::CONFLICT, body.to_string()));
        }
    }

    // Assign to search.
    property.search_criteria_id = body.search_criteria_id;

    let saved = db::add_listing(&state.db, &property).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;

    // Save any parsed open house events (upsert — ignore duplicates).
    if !open_houses.is_empty() {
        if let Err(e) = db::upsert_open_houses(&state.db, saved.id, &open_houses).await {
            tracing::warn!("add_listing: failed to save open houses for id={}: {}", saved.id, e);
        }
    }

    // Register image URLs in images_cache, preserving parser ordering.
    tracing::info!(
        "add_listing: id={} registering {} image URL(s)",
        saved.id,
        image_urls.len()
    );

    // Save raw HTML snapshots for offline inspection / parser backfills.
    if let Some(kind) = parsers::SourceKind::from_url(&source.url) {
        crate::html_snapshots::save_listing_html(saved.id, kind, &source.html).await;
    }

    for (position, url) in image_urls.iter().enumerate() {
        let _ = db::insert_image_url(&state.db, saved.id, url, position as i64).await;
    }

    // Download any pending images.
    let cached = images::cache_images(&state.db, &state.client, state.store.as_ref(), saved.id).await;
    tracing::info!("add_listing: id={} cached {} new image(s)", saved.id, cached);

    let images = db::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    let open_houses = db::list_open_houses(&state.db, saved.id)
        .await
        .unwrap_or_default();
    Ok(Json(db::Property { images, open_houses, ..saved }))
}

/// Returns `true` for hosts that are known to block programmatic HTTP
/// requests at the infrastructure level (bot-protection CDNs), making
/// HTML scraping impossible without a real browser.
///
/// - **zillow.com** — PerimeterX via CloudFront (`x-px-blocked: 1`)
/// - **realtor.ca** — Imperva Incapsula
fn is_blocked_host(url: &Url) -> bool {
    match url.host_str().unwrap_or("") {
        h if h.contains("zillow.com") => true,
        h if h.contains("realtor.ca") => true,
        _ => false,
    }
}

/// Constructs a blank `Property` with all fields zeroed/None and mortgage
/// defaults pre-filled.  Used as a base for stub listings when scraping is
/// blocked.  Callers should set the relevant URL field(s) after calling this.
fn blank_stub() -> db::Property {
    db::Property {
        id: 0,
        search_criteria_id: 0, // overwritten by the caller before insert
        redfin_url: None,
        realtor_url: None,
        rew_url: None,
        zillow_url: None,
        title: String::new(),
        description: String::new(),
        price: None,
        price_currency: None,
        offer_price: None,
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
        images: vec![],
        open_houses: vec![],
        created_at: String::new(),
        updated_at: None,
        notes: None,
        parking_total: None,
        parking_garage: None,
        parking_carport: None,
        parking_pad: None,
        land_sqft: None,
        property_tax: None,
        skytrain_station: None,
        skytrain_walk_min: None,
        radiant_floor_heating: None,
        ac: None,
        down_payment_pct: Some(0.20),
        mortgage_interest_rate: Some(0.04),
        amortization_years: Some(25),
        mortgage_monthly: None,
        hoa_monthly: None,
        monthly_total: None,
        monthly_cost: None,
        has_rental_suite: None,
        rental_income: None,
        status: crate::models::property::ListingStatus::Interested,
        school_elementary: None,
        school_elementary_rating: None,
        school_middle: None,
        school_middle_rating: None,
        school_secondary: None,
        school_secondary_rating: None,
        property_type: None,
        listed_date: None,
        mls_number: None,
        laundry_in_unit: None,
    }
}
