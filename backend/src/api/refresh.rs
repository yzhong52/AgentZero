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
