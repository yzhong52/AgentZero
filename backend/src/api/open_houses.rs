//! Open house endpoints.
//!
//! - GET   /api/listings/:id/open-houses            — list open houses for a listing
//! - PATCH /api/listings/:id/open-houses/:oh_id     — toggle visited flag

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{db, AppState};

/// GET /api/listings/:id/open-houses
pub async fn get_open_houses(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<db::OpenHouse>>, (StatusCode, String)> {
    let entries = db::list_open_houses(&state.db, id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB error: {}", e),
        )
    })?;
    Ok(Json(entries))
}

#[derive(Deserialize)]
pub struct PatchVisitedRequest {
    pub visited: bool,
}

/// PATCH /api/listings/:id/open-houses/:oh_id
pub async fn patch_open_house(
    State(state): State<AppState>,
    Path((listing_id, oh_id)): Path<(i64, i64)>,
    Json(body): Json<PatchVisitedRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let found = db::patch_open_house_visited(&state.db, listing_id, oh_id, body.visited)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
        })?;
    if !found {
        return Err((StatusCode::NOT_FOUND, "Open house not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}
