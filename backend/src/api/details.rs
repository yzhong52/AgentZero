//! Handlers for editing listing details and reading audit history.
//!
//! - PATCH /api/listings/:id/notes    — update the notes field
//! - PATCH /api/listings/:id/details  — update any user-editable field
//! - GET   /api/listings/:id/history  — field change history

use axum::{Json, extract::{State, Path}, http::StatusCode};
use serde::Deserialize;

use crate::{
    AppState, db,
    compute_initial_monthly_interest, compute_monthly_cost, compute_monthly_total,
};

#[derive(Deserialize)]
pub struct NotesRequest {
    pub notes: Option<String>,
}

/// PATCH /api/listings/:id/notes
///
/// Updates the personal notes for a listing. `id` is the property/listing ID.
pub async fn patch_notes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NotesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    db::update_notes(&state.db, id, body.notes.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
}

/// PATCH /api/listings/:id/details
///
/// Updates user-tracked details for a listing. `id` is the property/listing ID.
/// Records a history entry if the price changed. Returns the updated property.
pub async fn patch_details(
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
    if let Some(s) = body.status { updated.status = s; }
    updated.school_elementary = body.school_elementary.clone().or(updated.school_elementary.clone());
    updated.school_elementary_rating = body.school_elementary_rating.or(updated.school_elementary_rating);
    updated.school_middle = body.school_middle.clone().or(updated.school_middle.clone());
    updated.school_middle_rating = body.school_middle_rating.or(updated.school_middle_rating);
    updated.school_secondary = body.school_secondary.clone().or(updated.school_secondary.clone());
    updated.school_secondary_rating = body.school_secondary_rating.or(updated.school_secondary_rating);
    updated.property_type = body.property_type.clone().or(updated.property_type.clone());
    updated.laundry_in_unit = body.laundry_in_unit.or(updated.laundry_in_unit);
    updated.mls_number = body.mls_number.clone().or(updated.mls_number.clone());

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

    // Re-attach images (update_by_id doesn't load them).
    let images = db::list_images_with_meta(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(db::Property { images, ..updated }))
}

/// GET /api/listings/:id/history
///
/// Returns price/field change history for a listing. `id` is the property/listing ID.
pub async fn get_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<db::HistoryEntry>>, (StatusCode, String)> {
    let entries = db::list_history(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(Json(entries))
}
