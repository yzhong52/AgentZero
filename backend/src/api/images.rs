//! DELETE /api/listings/:id/images/:image_id — remove a single cached image.

use axum::{extract::{State, Path}, http::StatusCode};
use object_store::{ObjectStoreExt, path::Path as ObjectPath};
use tokio::fs;

use crate::{AppState, IMAGES_URL_PREFIX, db};

/// DELETE /api/listings/:id/images/:image_id
///
/// Removes a single cached image: deletes the file from the object store and
/// the row from `images_cache`. Silently removes the per-listing directory if
/// it becomes empty.
pub async fn delete_image(
    State(state): State<AppState>,
    Path((listing_id, image_id)): Path<(i64, i64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Verify the image exists and belongs to this listing; get its local_path.
    let local_path = db::get_image_local_path(&state.db, image_id, listing_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?
        .ok_or((StatusCode::NOT_FOUND, "Image not found".to_string()))?;

    // Delete the file from the object store when it was successfully downloaded.
    if let Some(path) = local_path {
        // path looks like "/images/1/abc123.jpg"; strip prefix to get object key.
        let object_key = path
            .strip_prefix(&format!("{}/", IMAGES_URL_PREFIX))
            .unwrap_or(&path);
        if let Err(e) = state.store.delete(&ObjectPath::from(object_key)).await {
            tracing::warn!("Failed to delete image file {}: {}", object_key, e);
            // Proceed to remove the DB record even if file deletion fails.
        }
    }

    db::delete_image_record(&state.db, image_id, listing_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    // If no images remain for this listing, remove the empty per-listing directory.
    let dir = format!("{}/{}", state.images_dir, listing_id);
    if let Err(e) = fs::remove_dir(&dir).await {
        // Not empty (other images remain) or already gone — both are fine.
        tracing::debug!("Could not remove image dir {}: {}", dir, e);
    }

    Ok(StatusCode::NO_CONTENT)
}
