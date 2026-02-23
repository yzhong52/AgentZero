use axum::{extract::{State, Path}, Json};
use axum::http::StatusCode;
use crate::db;
use crate::images;
use crate::parsers;
use crate::{safe_url, fetch_html, compute_mortgage, compute_monthly_total, AppState};

/// Refreshes a saved listing by re-fetching it from source. `id` is the property/listing ID.
pub async fn refresh_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let property = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // Collect all stored source URLs and fetch each one.
    let stored_urls: Vec<String> = [
        property.redfin_url.clone(),
        property.realtor_url.clone(),
        property.rew_url.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();

    if stored_urls.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No source URL stored for this listing".to_string()));
    }

    let mut sources: Vec<(String, String)> = Vec::new();
    for url in &stored_urls {
        let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL in listing".to_string()))?;
        let html = fetch_html(&state.client, &parsed)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch {}: {}", url, e)))?;
        sources.push((parsed.to_string(), html));
    }

    let source_refs: Vec<(&str, &str)> = sources.iter().map(|(u, h)| (u.as_str(), h.as_str())).collect();
    let listing = parsers::parse_multi(&source_refs)
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
    updated.monthly_total = compute_monthly_total(updated.mortgage_monthly, updated.property_tax, updated.hoa_monthly);

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
