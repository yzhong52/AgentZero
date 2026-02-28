use crate::db;
use crate::images;
use crate::parsers;
use crate::{
    compute_initial_monthly_interest, compute_monthly_cost, compute_monthly_total,
    compute_mortgage, fetch_html, safe_url, AppState,
};
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

/// PUT /api/listings/:id/refresh
///
/// Re-fetches the stored source URLs, re-parses, and saves the updated data.
/// Parser-produced fields are overwritten. User-only fields (notes, status,
/// skytrain info, rental details) are never touched by `update_by_id` and remain intact.
/// Fields the parser may produce but that users can also set manually (schools, HOA fee)
/// are preserved from the stored record when the parser returns nothing for them.
pub async fn refresh_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    // ── 1. Load the stored record ──────────────────────────────────────────────
    let stored = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // ── 2. Fetch HTML for each stored source URL ───────────────────────────────
    let source_urls: Vec<String> = [
        stored.redfin_url.clone(),
        stored.realtor_url.clone(),
        stored.rew_url.clone(),
        stored.zillow_url.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if source_urls.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No source URL stored for this listing".to_string(),
        ));
    }

    let mut sources: Vec<parsers::SourceInput> = Vec::new();
    for url in &source_urls {
        let parsed_url = safe_url(url).ok_or((
            StatusCode::BAD_REQUEST,
            format!("Invalid stored URL: {url}"),
        ))?;
        match fetch_html(&state.client, &parsed_url).await {
            Ok(html) => {
                tracing::info!("refresh_listing: fetched source url={}", parsed_url.as_str());
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html,
                });
            }
            Err(e) => {
                tracing::warn!("refresh_listing: failed to fetch {}: {}", url, e);
                // Skip this source but continue with others.
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html: String::new(),
                });
            }
        }
    }

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
    updated.id = id;
    updated.redfin_url = stored.redfin_url.clone();
    updated.realtor_url = stored.realtor_url.clone();
    updated.rew_url = stored.rew_url.clone();
    updated.zillow_url = stored.zillow_url.clone();

    // Parsers currently do not populate school fields — users enter them manually.
    // Fall back to whatever the user already stored.
    updated.school_elementary = updated
        .school_elementary
        .or(stored.school_elementary.clone());
    updated.school_elementary_rating = updated
        .school_elementary_rating
        .or(stored.school_elementary_rating);
    updated.school_middle = updated.school_middle.or(stored.school_middle.clone());
    updated.school_middle_rating = updated.school_middle_rating.or(stored.school_middle_rating);
    updated.school_secondary = updated.school_secondary.or(stored.school_secondary.clone());
    updated.school_secondary_rating = updated
        .school_secondary_rating
        .or(stored.school_secondary_rating);

    // Preserve the user's HOA fee when the parser finds nothing (strata fees are
    // sometimes scraped but often absent — don't clobber a manually entered value).
    updated.hoa_monthly = updated.hoa_monthly.or(stored.hoa_monthly);

    // Preserve the user's offer price — parser never sets this.
    updated.offer_price = stored.offer_price;

    // Preserve a user-edited title; only let the parser title through when the
    // stored title is blank (i.e. the user never set one, or cleared it).
    if is_title_exist(&stored.title) {
        updated.title = stored.title.clone();
    }

    // ── 5. Recalculate mortgage ────────────────────────────────────────────────
    // Carry forward the user's saved mortgage parameters and recompute the monthly
    // payment against the (potentially updated) price.
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years = stored.amortization_years.unwrap_or(25);
    updated.down_payment_pct = Some(down_pct);
    updated.mortgage_interest_rate = Some(rate);
    updated.amortization_years = Some(years);
    if let Some(price) = updated.offer_price.or(updated.price) {
        updated.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }
    updated.monthly_total = compute_monthly_total(
        updated.mortgage_monthly,
        updated.property_tax,
        updated.hoa_monthly,
    );
    updated.monthly_cost = compute_monthly_cost(
        updated
            .offer_price
            .or(updated.price)
            .map(|price| compute_initial_monthly_interest(price, down_pct, rate)),
        updated.property_tax,
        updated.hoa_monthly,
    );

    // ── 6. Record price-change history ────────────────────────────────────────
    if stored.price != updated.price {
        let old = stored.price.map(|v| v.to_string());
        let new = updated.price.map(|v| v.to_string());
        let _ = db::insert_change(&state.db, id, "price", old.as_deref(), new.as_deref()).await;
    }

    // ── 7. Persist parsed fields ──────────────────────────────────────────────
    // `update_by_id` intentionally omits user-only columns (notes, status,
    // has_rental_suite, rental_income, skytrain_*) so those remain intact in the DB.
    let saved = db::update_by_id(&state.db, id, &updated)
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
        if let Err(e) = db::upsert_open_houses(&state.db, id, &open_houses).await {
            tracing::warn!("refresh_listing: failed to save open houses for id={}: {}", id, e);
        }
    }

    // ── 9. Refresh image cache ────────────────────────────────────────────────
    // Upsert the freshly parsed image URLs, then download any that are not yet cached.
    for (position, url) in image_urls.iter().enumerate() {
        let _ = db::insert_image_url(&state.db, id, url, position as i64).await;
    }
    images::cache_images(&state.db, &state.client, state.store.as_ref(), id).await;

    // ── 10. Return the refreshed record with image metadata ───────────────────
    let images = db::list_images_with_meta(&state.db, saved.id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(db::Property { images, ..saved }))
}

/// GET /api/listings/:id/preview
///
/// Fetches and parses a listing without saving — used to build the refresh diff preview.
/// Applies the same field-preservation rules as `refresh_listing` so the diff accurately
/// reflects what would change if the user confirmed the refresh.
pub async fn preview_refresh(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    // ── 1. Load the stored record ──────────────────────────────────────────────
    let stored = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // ── 2. Fetch HTML for each stored source URL ───────────────────────────────
    let stored_urls: Vec<String> = [
        stored.redfin_url.clone(),
        stored.realtor_url.clone(),
        stored.rew_url.clone(),
        stored.zillow_url.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if stored_urls.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No source URL stored for this listing".to_string(),
        ));
    }

    let mut sources: Vec<parsers::SourceInput> = Vec::new();
    for url in &stored_urls {
        let parsed_url = safe_url(url).ok_or((
            StatusCode::BAD_REQUEST,
            format!("Invalid stored URL: {url}"),
        ))?;
        match fetch_html(&state.client, &parsed_url).await {
            Ok(html) => {
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html,
                });
            }
            Err(e) => {
                tracing::warn!("preview_refresh: failed to fetch {}: {}", url, e);
                sources.push(parsers::SourceInput {
                    url: parsed_url.to_string(),
                    html: String::new(),
                });
            }
        }
    }

    // ── 3. Parse ───────────────────────────────────────────────────────────────
    let listing = parsers::parse_multi(&sources).ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "No recognized listing format found in page".to_string(),
    ))?;
    let mut preview = listing.property;

    // ── 4. Apply the same field-preservation rules as refresh ─────────────────
    // School fields are user-entered; parsers never populate them.
    preview.school_elementary = preview
        .school_elementary
        .or(stored.school_elementary.clone());
    preview.school_elementary_rating = preview
        .school_elementary_rating
        .or(stored.school_elementary_rating);
    preview.school_middle = preview.school_middle.or(stored.school_middle.clone());
    preview.school_middle_rating = preview.school_middle_rating.or(stored.school_middle_rating);
    preview.school_secondary = preview.school_secondary.or(stored.school_secondary.clone());
    preview.school_secondary_rating = preview
        .school_secondary_rating
        .or(stored.school_secondary_rating);

    // Keep a manually-entered HOA fee when the parser has nothing to say.
    preview.hoa_monthly = preview.hoa_monthly.or(stored.hoa_monthly);

    // Preserve the user's offer price — the parser never sets this.
    preview.offer_price = stored.offer_price;

    // Preserve a user-edited title (same rule as refresh_listing).
    if is_title_exist(&stored.title) {
        preview.title = stored.title.clone();
    }

    // ── 5. Recalculate mortgage ────────────────────────────────────────────────
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years = stored.amortization_years.unwrap_or(25);
    preview.down_payment_pct = Some(down_pct);
    preview.mortgage_interest_rate = Some(rate);
    preview.amortization_years = Some(years);
    if let Some(price) = preview.offer_price.or(preview.price) {
        preview.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }
    preview.monthly_total = compute_monthly_total(
        preview.mortgage_monthly,
        preview.property_tax,
        preview.hoa_monthly,
    );
    preview.monthly_cost = compute_monthly_cost(
        preview
            .offer_price
            .or(preview.price)
            .map(|price| compute_initial_monthly_interest(price, down_pct, rate)),
        preview.property_tax,
        preview.hoa_monthly,
    );

    Ok(Json(preview))
}
