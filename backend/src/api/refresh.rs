use crate::images;
use crate::fetching::fetch::fetch_html;
use crate::fetching::html_snapshots::save_listing_html;
use crate::fetching::url::parse_listing_url;
use crate::models::property::Property;
use crate::parsers;
use crate::finance as property_finance;
use crate::store::{history_store, image_store, open_house_store, property_store};
use crate::AppState;
use axum::http::StatusCode;
use axum::{
    extract::{Path, State},
    Json,
};

/// Returns `true` when the stored title has been set by the user and should be
/// preserved during a refresh. An empty string means the user never set one (or
/// cleared it), so the parser title is allowed through.
fn is_title_exist(title: &str) -> bool {
    !title.is_empty()
}

/// Collects all non-`None` source URLs from a stored property in a consistent
/// order (Redfin → Realtor → REW → Zillow). The resulting slice is passed to
/// [`fetch_sources`] to re-fetch and re-parse during a refresh or preview.
fn stored_source_urls(stored: &Property) -> Vec<String> {
    [
        stored.redfin_url.clone(),
        stored.realtor_url.clone(),
        stored.rew_url.clone(),
        stored.zillow_url.clone(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

async fn fetch_sources(
    state: &AppState,
    urls: &[String],
    context: &str,
    log_success: bool,
) -> Result<Vec<parsers::SourceInput>, (StatusCode, String)> {
    if urls.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No source URL stored for this listing".to_string(),
        ));
    }

    let mut sources: Vec<parsers::SourceInput> = Vec::new();
    for url in urls {
        let parsed_url = parse_listing_url(url).ok_or((
            StatusCode::BAD_REQUEST,
            format!("Invalid stored URL: {url}"),
        ))?.url;
        match fetch_html(&state.client, &parsed_url).await {
            Ok(html) => {
                if log_success {
                    tracing::info!(
                        "{}: fetched source url={}",
                        context,
                        parsed_url.as_str()
                    );
                }
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html,
                });
            }
            Err(e) => {
                tracing::warn!("{}: failed to fetch {}: {}", context, url, e);
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html: String::new(),
                });
            }
        }
    }

    Ok(sources)
}

/// Merges a freshly parsed property with the stored record into the final value
/// that gets written back to the DB (or returned as a preview).
///
/// Every field of [`Property`] is listed explicitly so that adding a new field
/// to the struct produces a compile error here, forcing a conscious merge decision.
///
/// The three merge rules used below are:
/// - **Identity / user-owned** — always taken from `stored`; the parser has no
///   knowledge of these (e.g. `search_profile_id`, `status`, skytrain data).
/// - **Parser wins, stored as fallback** — parser value used when present;
///   stored value kept when the parser returns `None` (schools, HOA fee).
/// - **Parser wins** — everything else; the fresh parse is the source of truth.
///
/// `id` is passed separately because callers know the DB row id while the
/// parsed struct always carries `0`.
fn merge_with_stored(parsed: Property, stored: &Property, id: i64) -> Property {
    // Pre-compute fields that are both stored in the struct and fed into the
    // mortgage calculation, so each value is stated exactly once.
    let hoa_monthly = parsed.hoa_monthly.or(stored.hoa_monthly);
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years = stored.amortization_years.unwrap_or(25);
    let finance = property_finance::compute(
        parsed.price,
        stored.offer_price,
        down_pct,
        rate,
        years,
        parsed.property_tax,
        hoa_monthly,
    );

    Property {
        // ── Identity ──────────────────────────────────────────────────────────
        // These fields have no representation in the parsed HTML.
        // search_profile_id must come from stored; writing the struct default (0)
        // violates the FK constraint on the search_profiles table.
        id,
        search_profile_id: stored.search_profile_id,

        // ── Source URLs ───────────────────────────────────────────────────────
        // Users may manually link additional sources; preserve them all.
        redfin_url: stored.redfin_url.clone(),
        realtor_url: stored.realtor_url.clone(),
        rew_url: stored.rew_url.clone(),
        zillow_url: stored.zillow_url.clone(),

        // ── User-owned: parser never produces these ───────────────────────────
        status: stored.status,
        notes: stored.notes.clone(),
        offer_price: stored.offer_price,
        skytrain_station: stored.skytrain_station.clone(),
        skytrain_walk_min: stored.skytrain_walk_min,
        has_rental_suite: stored.has_rental_suite,
        rental_income: stored.rental_income,

        // ── User-set mortgage parameters ──────────────────────────────────────
        down_payment_pct: Some(down_pct),
        mortgage_interest_rate: Some(rate),
        amortization_years: Some(years),

        // ── Parser wins, stored as fallback ───────────────────────────────────
        title: if is_title_exist(&stored.title) { stored.title.clone() } else { parsed.title },
        school_elementary: parsed.school_elementary.or(stored.school_elementary.clone()),
        school_elementary_rating: parsed.school_elementary_rating.or(stored.school_elementary_rating),
        school_middle: parsed.school_middle.or(stored.school_middle.clone()),
        school_middle_rating: parsed.school_middle_rating.or(stored.school_middle_rating),
        school_secondary: parsed.school_secondary.or(stored.school_secondary.clone()),
        school_secondary_rating: parsed.school_secondary_rating.or(stored.school_secondary_rating),
        hoa_monthly,

        // ── Parser wins ───────────────────────────────────────────────────────
        description: parsed.description,
        price: parsed.price,
        price_currency: parsed.price_currency,
        street_address: parsed.street_address,
        city: parsed.city,
        region: parsed.region,
        postal_code: parsed.postal_code,
        country: parsed.country,
        lat: parsed.lat,
        lon: parsed.lon,
        property_type: parsed.property_type,
        bedrooms: parsed.bedrooms,
        bathrooms: parsed.bathrooms,
        sqft: parsed.sqft,
        land_sqft: parsed.land_sqft,
        year_built: parsed.year_built,
        parking_total: parsed.parking_total,
        parking_garage: parsed.parking_garage,
        parking_carport: parsed.parking_carport,
        parking_pad: parsed.parking_pad,
        radiant_floor_heating: parsed.radiant_floor_heating,
        ac: parsed.ac,
        laundry_in_unit: parsed.laundry_in_unit,
        property_tax: parsed.property_tax,
        mls_number: parsed.mls_number,
        listed_date: parsed.listed_date,

        // ── Computed ──────────────────────────────────────────────────────────
        mortgage_monthly: finance.mortgage_monthly,
        monthly_total: finance.monthly_total,
        monthly_cost: finance.monthly_cost,

        // ── System metadata ───────────────────────────────────────────────────
        created_at: stored.created_at.clone(),
        updated_at: stored.updated_at.clone(),
        images: vec![],      // repopulated from images_cache after save
        open_houses: vec![], // repopulated from open_houses table after save
    }
}

/// PUT /api/listings/:id/refresh
///
/// Re-fetches the stored source URLs, re-parses, and saves the updated data.
/// The parsed result is merged with the stored record via [`merge_with_stored`]
/// before saving: user-owned fields survive, parser fields are overwritten, and
/// shared fields (schools, HOA) fall back to the stored value when the parser
/// returns nothing.
pub(crate) async fn refresh_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Property>, (StatusCode, String)> {
    // ── 1. Load the stored record ──────────────────────────────────────────────
    let stored = property_store::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // ── 2. Fetch HTML for each stored source URL ───────────────────────────────
    let source_urls = stored_source_urls(&stored);
    let sources = fetch_sources(&state, &source_urls, "refresh_listing", true).await?;

    // ── 3. Parse ───────────────────────────────────────────────────────────────
    let listing = parsers::parse_multi(&sources).ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "No recognized listing format found in page".to_string(),
    ))?;
    let image_urls = listing.image_urls;
    let open_houses = listing.open_houses;
    tracing::info!(
        "refresh_listing: parse result property_tax={:?}, price={:?}",
        listing.property.property_tax,
        listing.property.price
    );

    // ── 4. Merge parsed result with stored record ──────────────────────────────
    // merge_with_stored carries forward user-owned fields and computes mortgage
    // values inline — no further mutation needed.
    let updated = merge_with_stored(listing.property, &stored, id);

    // ── 6. Record price-change history ────────────────────────────────────────
    if stored.price != updated.price {
        let old = stored.price.map(|v| v.to_string());
        let new = updated.price.map(|v| v.to_string());
        let _ = history_store::insert_change(&state.db, id, "price", old.as_deref(), new.as_deref()).await;
    }

    // ── 7. Persist parsed fields ──────────────────────────────────────────────
    // `update_by_id` intentionally omits user-only columns (notes, status,
    // has_rental_suite, rental_income, skytrain_*) so those remain intact in the DB.
    let saved = property_store::update_by_id(&state.db, id, &updated)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    tracing::info!(
        "refresh_listing: saved listing id={} property_tax={:?} price={:?}",
        saved.id,
        saved.property_tax,
        saved.price
    );

    // ── 8. Upsert open house events ───────────────────────────────────────────
    if !open_houses.is_empty() {
        if let Err(e) = open_house_store::upsert_open_houses(&state.db, id, &open_houses).await {
            tracing::warn!("refresh_listing: failed to save open houses for id={}: {}", id, e);
        }
    }

    // ── 9. Refresh image cache ────────────────────────────────────────────────
    // Upsert the freshly parsed image URLs, then download any that are not yet cached.

    // Save raw HTML snapshots for offline inspection / parser backfills.
    for source in &sources {
        if let Some(site) = parsers::ListingSite::from_url(&source.url) {
            save_listing_html(id, site, &source.html).await;
        }
    }

    tracing::info!(
        "refresh_listing: id={} registering {} image URL(s)",
        id,
        image_urls.len()
    );
    for (position, url) in image_urls.iter().enumerate() {
        let _ = image_store::insert_image_url(&state.db, id, url, position as i64).await;
    }
    let cached = images::cache_images(&state.db, &state.client, state.store.as_ref(), id).await;
    tracing::info!("refresh_listing: id={} cached {} new image(s)", id, cached);

    // ── 10. Return the refreshed record with image metadata ───────────────────
    let images = image_store::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    let open_houses = open_house_store::list_open_houses(&state.db, saved.id)
        .await
        .unwrap_or_default();

    Ok(Json(Property { images, open_houses, ..saved }))
}

/// GET /api/listings/:id/preview
///
/// Fetches and parses a listing without saving — used to build the refresh diff preview.
/// Applies the same field-preservation rules as `refresh_listing` so the diff accurately
/// reflects what would change if the user confirmed the refresh.
pub(crate) async fn preview_refresh(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Property>, (StatusCode, String)> {
    // ── 1. Load the stored record ──────────────────────────────────────────────
    let stored = property_store::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // ── 2. Fetch HTML for each stored source URL ───────────────────────────────
    let stored_urls = stored_source_urls(&stored);
    let sources = fetch_sources(&state, &stored_urls, "preview_refresh", false).await?;

    // ── 3. Parse ───────────────────────────────────────────────────────────────
    let listing = parsers::parse_multi(&sources).ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "No recognized listing format found in page".to_string(),
    ))?;
    // ── 4. Merge using the same rules as refresh ──────────────────────────────
    let preview = merge_with_stored(listing.property, &stored, stored.id);

    Ok(Json(preview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::property::ListingStatus;

    /// Minimal valid `Property` with every field at its zero/None value.
    /// Tests should override only the fields they care about.
    fn base_property() -> Property {
        Property {
            id: 0,
            search_profile_id: 1,
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
            lat: None,
            lon: None,
            property_type: None,
            bedrooms: None,
            bathrooms: None,
            sqft: None,
            land_sqft: None,
            year_built: None,
            parking_total: None,
            parking_garage: None,
            parking_carport: None,
            parking_pad: None,
            radiant_floor_heating: None,
            ac: None,
            laundry_in_unit: None,
            skytrain_station: None,
            skytrain_walk_min: None,
            property_tax: None,
            hoa_monthly: None,
            down_payment_pct: None,
            mortgage_interest_rate: None,
            amortization_years: None,
            mortgage_monthly: None,
            monthly_total: None,
            monthly_cost: None,
            has_rental_suite: None,
            rental_income: None,
            school_elementary: None,
            school_elementary_rating: None,
            school_middle: None,
            school_middle_rating: None,
            school_secondary: None,
            school_secondary_rating: None,
            redfin_url: None,
            realtor_url: None,
            rew_url: None,
            zillow_url: None,
            mls_number: None,
            listed_date: None,
            status: ListingStatus::Interested,
            notes: None,
            images: vec![],
            open_houses: vec![],
            created_at: String::new(),
            updated_at: None,
        }
    }

    // ── is_title_exist ────────────────────────────────────────────────────────

    #[test]
    fn test_is_title_exist_empty_string() {
        assert!(!is_title_exist(""));
    }

    #[test]
    fn test_is_title_exist_non_empty_string() {
        assert!(is_title_exist("123 Main St"));
    }

    // ── stored_source_urls ────────────────────────────────────────────────────

    #[test]
    fn test_stored_source_urls_all_set() {
        let p = Property {
            redfin_url: Some("https://redfin.ca/1".to_string()),
            realtor_url: Some("https://realtor.ca/1".to_string()),
            rew_url: Some("https://rew.ca/1".to_string()),
            zillow_url: Some("https://zillow.com/1".to_string()),
            ..base_property()
        };
        let urls = stored_source_urls(&p);
        assert_eq!(urls.len(), 4);
        assert_eq!(urls[0], "https://redfin.ca/1");
        assert_eq!(urls[1], "https://realtor.ca/1");
        assert_eq!(urls[2], "https://rew.ca/1");
        assert_eq!(urls[3], "https://zillow.com/1");
    }

    #[test]
    fn test_stored_source_urls_partial() {
        let p = Property {
            redfin_url: Some("https://redfin.ca/1".to_string()),
            zillow_url: Some("https://zillow.com/1".to_string()),
            ..base_property()
        };
        let urls = stored_source_urls(&p);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://redfin.ca/1");
        assert_eq!(urls[1], "https://zillow.com/1");
    }

    #[test]
    fn test_stored_source_urls_none() {
        let urls = stored_source_urls(&base_property());
        assert!(urls.is_empty());
    }

    // ── merge_with_stored ─────────────────────────────────────────────────────

    #[test]
    fn test_merge_identity_fields_from_stored() {
        let parsed = Property {
            redfin_url: Some("https://wrong.com".to_string()),
            search_profile_id: 0,
            ..base_property()
        };
        let stored = Property {
            redfin_url: Some("https://redfin.ca/correct".to_string()),
            realtor_url: Some("https://realtor.ca/correct".to_string()),
            rew_url: None,
            zillow_url: Some("https://zillow.com/correct".to_string()),
            search_profile_id: 3,
            ..base_property()
        };

        let result = merge_with_stored(parsed, &stored, 42);

        assert_eq!(result.id, 42);
        assert_eq!(result.redfin_url.as_deref(), Some("https://redfin.ca/correct"));
        assert_eq!(result.realtor_url.as_deref(), Some("https://realtor.ca/correct"));
        assert_eq!(result.rew_url, None);
        assert_eq!(result.zillow_url.as_deref(), Some("https://zillow.com/correct"));
    }

    /// Regression: parser produces search_profile_id = 0 (struct default),
    /// which has no matching row in search_profiles → FK constraint failure.
    #[test]
    fn test_merge_preserves_search_profile_id() {
        let parsed = Property { search_profile_id: 0, ..base_property() };
        let stored = Property { search_profile_id: 5, ..base_property() };

        let result = merge_with_stored(parsed, &stored, 1);

        assert_eq!(result.search_profile_id, 5);
    }

    #[test]
    fn test_merge_user_owned_fields_from_stored() {
        let parsed = Property {
            status: ListingStatus::Interested,
            skytrain_station: None,
            skytrain_walk_min: None,
            has_rental_suite: None,
            rental_income: None,
            offer_price: None,
            notes: None,
            ..base_property()
        };
        let stored = Property {
            status: ListingStatus::Buyable,
            skytrain_station: Some("Main St".to_string()),
            skytrain_walk_min: Some(5),
            has_rental_suite: Some(true),
            rental_income: Some(1500),
            offer_price: Some(800_000),
            notes: Some("Great location".to_string()),
            ..base_property()
        };

        let result = merge_with_stored(parsed, &stored, 1);

        assert_eq!(result.status, ListingStatus::Buyable);
        assert_eq!(result.skytrain_station.as_deref(), Some("Main St"));
        assert_eq!(result.skytrain_walk_min, Some(5));
        assert_eq!(result.has_rental_suite, Some(true));
        assert_eq!(result.rental_income, Some(1500));
        assert_eq!(result.offer_price, Some(800_000));
        assert_eq!(result.notes.as_deref(), Some("Great location"));
    }

    #[test]
    fn test_merge_parser_fields_win() {
        let parsed = Property {
            price: Some(1_200_000),
            bedrooms: Some(4),
            bathrooms: Some(3),
            property_tax: Some(7_000),
            ..base_property()
        };

        let result = merge_with_stored(parsed, &base_property(), 1);

        assert_eq!(result.price, Some(1_200_000));
        assert_eq!(result.bedrooms, Some(4));
        assert_eq!(result.bathrooms, Some(3));
        assert_eq!(result.property_tax, Some(7_000));
    }

    #[test]
    fn test_merge_school_parser_wins_over_stored() {
        let parsed = Property {
            school_elementary: Some("Parser Elementary".to_string()),
            school_elementary_rating: Some(9.0),
            ..base_property()
        };
        let stored = Property {
            school_elementary: Some("Stored Elementary".to_string()),
            school_elementary_rating: Some(7.0),
            ..base_property()
        };

        let result = merge_with_stored(parsed, &stored, 1);

        assert_eq!(result.school_elementary.as_deref(), Some("Parser Elementary"));
        assert_eq!(result.school_elementary_rating, Some(9.0));
    }

    #[test]
    fn test_merge_school_stored_fallback_when_parser_empty() {
        let stored = Property {
            school_elementary: Some("Stored Elementary".to_string()),
            school_elementary_rating: Some(7.0),
            school_middle: Some("Stored Middle".to_string()),
            school_middle_rating: Some(6.5),
            school_secondary: Some("Stored Secondary".to_string()),
            school_secondary_rating: Some(8.0),
            ..base_property()
        };

        let result = merge_with_stored(base_property(), &stored, 1);

        assert_eq!(result.school_elementary.as_deref(), Some("Stored Elementary"));
        assert_eq!(result.school_elementary_rating, Some(7.0));
        assert_eq!(result.school_middle.as_deref(), Some("Stored Middle"));
        assert_eq!(result.school_middle_rating, Some(6.5));
        assert_eq!(result.school_secondary.as_deref(), Some("Stored Secondary"));
        assert_eq!(result.school_secondary_rating, Some(8.0));
    }

    #[test]
    fn test_merge_hoa_parser_wins() {
        let parsed = Property { hoa_monthly: Some(500), ..base_property() };
        let stored = Property { hoa_monthly: Some(300), ..base_property() };

        assert_eq!(merge_with_stored(parsed, &stored, 1).hoa_monthly, Some(500));
    }

    #[test]
    fn test_merge_hoa_stored_fallback_when_parser_empty() {
        let stored = Property { hoa_monthly: Some(300), ..base_property() };

        assert_eq!(merge_with_stored(base_property(), &stored, 1).hoa_monthly, Some(300));
    }

    #[test]
    fn test_merge_title_preserved_when_user_set() {
        let parsed = Property { title: "Parser Title".to_string(), ..base_property() };
        let stored = Property { title: "User Title".to_string(), ..base_property() };

        assert_eq!(merge_with_stored(parsed, &stored, 1).title, "User Title");
    }

    #[test]
    fn test_merge_title_uses_parser_when_stored_empty() {
        let parsed = Property { title: "Parser Title".to_string(), ..base_property() };

        assert_eq!(merge_with_stored(parsed, &base_property(), 1).title, "Parser Title");
    }
}
