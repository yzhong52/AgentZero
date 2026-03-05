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

/// Merges user-curated data back into a freshly parsed property.
///
/// Some fields can originate from both the parser and the user:
///
/// - **Schools / ratings** — the parser value wins when present; the stored
///   value is kept as a fallback so manual entries survive a re-parse that
///   does not include school data.
/// - **HOA fee** — same fallback rule as schools.
/// - **`offer_price`** — always taken from `stored`; the parser never sets
///   this field and the user's intended offer must survive every refresh.
/// - **`title`** — if the stored title is non-empty (the user has typed a
///   custom title), it is preserved; an empty stored title means the parser
///   title is used.
fn apply_shared_preserved_fields(target: &mut Property, stored: &Property) {
    target.school_elementary = target
        .school_elementary
        .clone()
        .or(stored.school_elementary.clone());
    target.school_elementary_rating = target
        .school_elementary_rating
        .or(stored.school_elementary_rating);
    target.school_middle = target.school_middle.clone().or(stored.school_middle.clone());
    target.school_middle_rating = target.school_middle_rating.or(stored.school_middle_rating);
    target.school_secondary = target
        .school_secondary
        .clone()
        .or(stored.school_secondary.clone());
    target.school_secondary_rating = target
        .school_secondary_rating
        .or(stored.school_secondary_rating);

    target.hoa_monthly = target.hoa_monthly.or(stored.hoa_monthly);
    target.offer_price = stored.offer_price;

    if is_title_exist(&stored.title) {
        target.title = stored.title.clone();
    }
}

/// Restores fields from the stored record that must survive every refresh and
/// cannot be derived from parser output.
///
/// - `id` is forced to the DB row id (parsers always produce `0`).
/// - Source URLs are kept from `stored` because the user may have manually
///   linked additional sources that a parser cannot re-derive from a single page.
/// - `search_profile_id` is kept from `stored`; the parsed `Property` carries
///   no search-profile context, and writing `0` (the struct default) would
///   violate the FK constraint on the `search_profiles` table.
fn apply_refresh_identity_fields(target: &mut Property, stored: &Property, id: i64) {
    target.id = id;
    target.redfin_url = stored.redfin_url.clone();
    target.realtor_url = stored.realtor_url.clone();
    target.rew_url = stored.rew_url.clone();
    target.zillow_url = stored.zillow_url.clone();
    target.search_profile_id = stored.search_profile_id;
}

/// PUT /api/listings/:id/refresh
///
/// Re-fetches the stored source URLs, re-parses, and saves the updated data.
/// Parser-produced fields are overwritten. User-only fields (notes, status,
/// skytrain info, rental details) are never touched by `update_by_id` and remain intact.
/// Fields the parser may produce but that users can also set manually (schools, HOA fee)
/// are preserved from the stored record when the parser returns nothing for them.
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
    let mut updated = listing.property;
    let image_urls = listing.image_urls;
    let open_houses = listing.open_houses;
    tracing::info!(
        "refresh_listing: parse result property_tax={:?}, price={:?}",
        updated.property_tax,
        updated.price
    );

    // ── 4. Merge identity and user-preserved fields ────────────────────────────
    // Keep the DB id and the stored source URLs — users may link additional
    // sources that the parser cannot re-derive.
    apply_refresh_identity_fields(&mut updated, &stored, id);
    apply_shared_preserved_fields(&mut updated, &stored);

    // ── 5. Recalculate mortgage ────────────────────────────────────────────────
    // Carry forward the user's saved mortgage parameters and recompute the monthly
    // payment against the (potentially updated) price.
    property_finance::recompute_with_stored_terms(&mut updated, &stored);

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
    let mut preview = listing.property;

    // ── 4. Apply the same field-preservation rules as refresh ─────────────────
    apply_shared_preserved_fields(&mut preview, &stored);

    // ── 5. Recalculate mortgage ────────────────────────────────────────────────
    property_finance::recompute_with_stored_terms(&mut preview, &stored);

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

    // ── apply_refresh_identity_fields ─────────────────────────────────────────

    #[test]
    fn test_identity_fields_copies_id_and_urls() {
        let mut target = Property {
            redfin_url: Some("https://wrong.com".to_string()),
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

        apply_refresh_identity_fields(&mut target, &stored, 42);

        assert_eq!(target.id, 42);
        assert_eq!(target.redfin_url, stored.redfin_url);
        assert_eq!(target.realtor_url, stored.realtor_url);
        assert_eq!(target.rew_url, stored.rew_url);
        assert_eq!(target.zillow_url, stored.zillow_url);
    }

    /// Regression test for the FK constraint failure: the parser produces
    /// `search_profile_id = 0` (struct default), which has no matching row in
    /// `search_profiles`. The fix copies it from the stored record.
    #[test]
    fn test_identity_fields_preserves_search_profile_id() {
        let mut target = base_property(); // search_profile_id = 1 (base default)
        let stored = Property { search_profile_id: 5, ..base_property() };

        apply_refresh_identity_fields(&mut target, &stored, 1);

        assert_eq!(target.search_profile_id, 5);
    }

    #[test]
    fn test_identity_fields_does_not_touch_parsed_fields() {
        let mut target = Property {
            price: Some(1_000_000),
            title: "Parser Title".to_string(),
            ..base_property()
        };
        apply_refresh_identity_fields(&mut target, &base_property(), 1);

        assert_eq!(target.price, Some(1_000_000));
        assert_eq!(target.title, "Parser Title");
    }

    // ── apply_shared_preserved_fields ─────────────────────────────────────────

    #[test]
    fn test_shared_fields_parser_school_wins_over_stored() {
        let mut target = Property {
            school_elementary: Some("Parser Elementary".to_string()),
            school_elementary_rating: Some(9.0),
            ..base_property()
        };
        let stored = Property {
            school_elementary: Some("Stored Elementary".to_string()),
            school_elementary_rating: Some(7.0),
            ..base_property()
        };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.school_elementary.as_deref(), Some("Parser Elementary"));
        assert_eq!(target.school_elementary_rating, Some(9.0));
    }

    #[test]
    fn test_shared_fields_stored_school_fallback_when_parser_empty() {
        let mut target = base_property(); // all school fields None
        let stored = Property {
            school_elementary: Some("Stored Elementary".to_string()),
            school_elementary_rating: Some(7.0),
            school_middle: Some("Stored Middle".to_string()),
            school_middle_rating: Some(6.5),
            school_secondary: Some("Stored Secondary".to_string()),
            school_secondary_rating: Some(8.0),
            ..base_property()
        };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.school_elementary.as_deref(), Some("Stored Elementary"));
        assert_eq!(target.school_elementary_rating, Some(7.0));
        assert_eq!(target.school_middle.as_deref(), Some("Stored Middle"));
        assert_eq!(target.school_middle_rating, Some(6.5));
        assert_eq!(target.school_secondary.as_deref(), Some("Stored Secondary"));
        assert_eq!(target.school_secondary_rating, Some(8.0));
    }

    #[test]
    fn test_shared_fields_hoa_parser_wins() {
        let mut target = Property { hoa_monthly: Some(500), ..base_property() };
        let stored = Property { hoa_monthly: Some(300), ..base_property() };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.hoa_monthly, Some(500));
    }

    #[test]
    fn test_shared_fields_hoa_stored_fallback_when_parser_empty() {
        let mut target = base_property(); // hoa_monthly = None
        let stored = Property { hoa_monthly: Some(300), ..base_property() };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.hoa_monthly, Some(300));
    }

    #[test]
    fn test_shared_fields_offer_price_always_from_stored() {
        let mut target = Property { offer_price: Some(999_999), ..base_property() };
        let stored = Property { offer_price: Some(500_000), ..base_property() };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.offer_price, Some(500_000));
    }

    #[test]
    fn test_shared_fields_offer_price_none_when_stored_is_none() {
        let mut target = Property { offer_price: Some(999_999), ..base_property() };

        apply_shared_preserved_fields(&mut target, &base_property());

        assert_eq!(target.offer_price, None);
    }

    #[test]
    fn test_shared_fields_title_preserved_when_user_set() {
        let mut target = Property { title: "Parser Title".to_string(), ..base_property() };
        let stored = Property { title: "User Title".to_string(), ..base_property() };

        apply_shared_preserved_fields(&mut target, &stored);

        assert_eq!(target.title, "User Title");
    }

    #[test]
    fn test_shared_fields_title_uses_parser_when_stored_empty() {
        let mut target = Property { title: "Parser Title".to_string(), ..base_property() };
        // stored.title is "" (base default — user never set a title)

        apply_shared_preserved_fields(&mut target, &base_property());

        assert_eq!(target.title, "Parser Title");
    }
}
