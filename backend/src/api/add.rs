//! POST /api/listings — add a new listing.
//!
//! Fetches and parses one or more listing URLs for the same property, saves
//! the merged result to the DB, downloads images, and returns the saved record.
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
//! When **all** submitted URLs are from known-blocked hosts (Zillow,
//! Realtor.ca), a stub listing is saved containing only the URL(s) so the
//! user can fill in details manually via the edit panel.  A mix of blocked
//! and unrecognised URLs still returns 422.

use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use url::Url;

use crate::{
    AppState, IMAGES_URL_PREFIX,
    db, images, parsers,
    safe_url, fetch_html,
    compute_mortgage, compute_monthly_total, compute_initial_monthly_interest, compute_monthly_cost,
};

#[derive(Deserialize)]
pub struct AddRequest {
    /// One or more listing URLs for the same property (e.g. redfin + rew).
    pub urls: Vec<String>,
}

pub async fn add_listing(
    State(state): State<AppState>,
    Json(body): Json<AddRequest>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    if body.urls.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "At least one URL is required".to_string()));
    }

    // Validate all URLs upfront.
    let parsed_urls: Vec<Url> = body
        .urls
        .iter()
        .map(|raw| safe_url(raw.trim()).ok_or((StatusCode::BAD_REQUEST, format!("Invalid URL: {}", raw.trim()))))
        .collect::<Result<_, _>>()?;

    // Fetch HTML for each URL.
    // Zillow (PerimeterX / CloudFront) and Realtor.ca (Imperva Incapsula)
    // block all programmatic HTTP requests regardless of headers.  On any
    // fetch error for a known-blocked host we fall through with empty HTML;
    // the stub path below saves the listing so the user can fill in details
    // manually.  Non-blocked hosts still return 502 on fetch failure.
    let mut sources: Vec<(String, String)> = Vec::new();
    for url in &parsed_urls {
        match fetch_html(&state.client, url).await {
            Ok(html) => sources.push((url.to_string(), html)),
            Err(e) if is_blocked_host(url) => {
                tracing::info!("add_listing: fetch blocked ({}), will save stub for {}", e, url);
                sources.push((url.to_string(), String::new()));
            }
            Err(e) => return Err((StatusCode::BAD_GATEWAY, format!("Failed to fetch {}: {}", url, e))),
        }
    }

    let source_refs: Vec<(&str, &str)> = sources.iter().map(|(u, h)| (u.as_str(), h.as_str())).collect();
    let listing_opt = parsers::parse_multi(&source_refs);

    let (mut property, image_urls) = match listing_opt {
        Some(listing) => (listing.property, listing.image_urls),
        None => {
            // Parsing yielded nothing.  Only save a stub when ALL URLs are
            // from known-blocked hosts — for unrecognised URLs return 422.
            if !parsed_urls.iter().all(is_blocked_host) {
                return Err((StatusCode::UNPROCESSABLE_ENTITY,
                    "No recognized listing format found in page".to_string()));
            }
            tracing::info!(
                "add_listing: all URLs are from blocked hosts, saving stub for {:?}",
                parsed_urls.iter().map(|u| u.as_str()).collect::<Vec<_>>(),
            );
            let mut stub = blank_stub();
            // Populate whichever URL fields we know about.
            for u in &parsed_urls {
                match u.host_str().unwrap_or("") {
                    h if h.contains("zillow.com")   => stub.zillow_url   = Some(u.to_string()),
                    h if h.contains("realtor.ca")   => stub.realtor_url  = Some(u.to_string()),
                    _ => {}
                }
            }
            (stub, vec![])
        }
    };

    // Auto-calculate mortgage with defaults on first save.
    let down_pct = property.down_payment_pct.unwrap_or(0.20);
    let rate     = property.mortgage_interest_rate.unwrap_or(0.04);
    let years    = property.amortization_years.unwrap_or(25);
    property.down_payment_pct       = Some(down_pct);
    property.mortgage_interest_rate = Some(rate);
    property.amortization_years     = Some(years);
    let base_price = property.offer_price.or(property.price);
    if let Some(price) = base_price {
        property.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }
    property.monthly_total = compute_monthly_total(property.mortgage_monthly, property.property_tax, property.hoa_monthly);
    let initial_interest = base_price.map(|p| compute_initial_monthly_interest(p, down_pct, rate));
    property.monthly_cost = compute_monthly_cost(initial_interest, property.property_tax, property.hoa_monthly);

    let saved = db::add_listing(&state.db, &property)
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
        IMAGES_URL_PREFIX,
    )
    .await;

    let images = db::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(Json(db::Property { images, ..saved }))
}

/// Returns `true` for hosts that are known to block programmatic HTTP
/// requests at the infrastructure level (bot-protection CDNs), making
/// HTML scraping impossible without a real browser.
///
/// - **zillow.com** — PerimeterX via CloudFront (`x-px-blocked: 1`)
/// - **realtor.ca** — Imperva Incapsula
fn is_blocked_host(url: &Url) -> bool {
    match url.host_str().unwrap_or("") {
        h if h.contains("zillow.com")  => true,
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
        created_at: String::new(),
        updated_at: None,
        notes: None,
        parking_garage: None,
        parking_covered: None,
        parking_open: None,
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
        status: None,
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
