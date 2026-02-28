//! Search CRUD handlers.
//!
//! - GET    /api/searches         — list all searches
//! - POST   /api/searches         — create a new search
//! - GET    /api/searches/:id     — get a single search
//! - PATCH  /api/searches/:id     — update title / description
//! - DELETE /api/searches/:id     — delete a search (detaches listings)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::models::search::Search;
use crate::store::search_store;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateSearchRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize)]
pub struct UpdateSearchRequest {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct ReorderRequest {
    /// Search IDs in the desired display order.
    pub ids: Vec<i64>,
}

/// GET /api/searches
pub async fn list_searches(
    State(state): State<AppState>,
) -> Result<Json<Vec<Search>>, (StatusCode, String)> {
    let searches = search_store::list_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(searches))
}

/// POST /api/searches
pub async fn create_search(
    State(state): State<AppState>,
    Json(body): Json<CreateSearchRequest>,
) -> Result<Json<Search>, (StatusCode, String)> {
    if body.title.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "title is required".to_string()));
    }
    let search = search_store::create(&state.db, body.title.trim(), &body.description)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(search))
}

/// GET /api/searches/:id
pub async fn get_search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Search>, (StatusCode, String)> {
    let search = search_store::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Search not found: {e}")))?;
    Ok(Json(search))
}

/// PATCH /api/searches/:id
pub async fn update_search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateSearchRequest>,
) -> Result<Json<Search>, (StatusCode, String)> {
    let search = search_store::update(
        &state.db,
        id,
        body.title.as_deref(),
        body.description.as_deref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(Json(search))
}

/// PUT /api/searches/reorder
pub async fn reorder_searches(
    State(state): State<AppState>,
    Json(body): Json<ReorderRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if body.ids.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "ids must not be empty".to_string()));
    }
    search_store::reorder(&state.db, &body.ids)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/searches/:id
pub async fn delete_search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    search_store::delete(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}
