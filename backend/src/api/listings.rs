//! Collection and lifecycle handlers for listings.
//!
//! - GET    /api/listings      — list all properties
//! - GET    /api/listings/:id  — get a single property
//! - DELETE /api/listings/:id  — delete a property and its images

use axum::{Json, extract::{State, Path}, http::StatusCode};
use object_store::{ObjectStoreExt, path::Path as ObjectPath};
use tokio::fs;

use crate::{AppState, IMAGES_URL_PREFIX, db};

/// GET /api/listings
///
/// Returns all saved properties, newest first. Each record includes cached
/// image metadata (id, local_path, position).
pub async fn list_listings(
    State(state): State<AppState>,
) -> Result<Json<Vec<db::Property>>, (StatusCode, String)> {
    let listings = db::list(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;
    Ok(Json(listings))
}

/// GET /api/listings/:id
///
/// Returns a single listing by ID (includes images and metadata).
pub async fn get_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let p = db::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;
    Ok(Json(p))
}

/// DELETE /api/listings/:id
///
/// Deletes a listing: removes image files from the object store, clears the
/// images_cache rows, then removes the listing row itself.
pub async fn delete_listing(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // 1. Delete locally-cached image files from the object store.
    let cached = db::list_cached_images(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    for img in &cached {
        let object_key = img.local_path
            .strip_prefix(&format!("{}/", IMAGES_URL_PREFIX))
            .unwrap_or(&img.local_path);
        if let Err(e) = state.store.delete(&ObjectPath::from(object_key)).await {
            tracing::warn!("delete_listing: could not remove image file {}: {}", object_key, e);
            // Continue — file may already be gone; don't block the delete.
        }
    }

    // 2. Remove the per-listing image directory (now empty after step 1).
    let dir = format!("{}/{}", state.images_dir, id);
    if let Err(e) = fs::remove_dir(&dir).await {
        tracing::debug!("delete_listing: could not remove image dir {}: {}", dir, e);
    }

    // 3. Remove images_cache rows (no CASCADE on this FK).
    db::delete_all_image_records(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // 4. Delete the listing row (listing_history cascades automatically).
    db::delete(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}
