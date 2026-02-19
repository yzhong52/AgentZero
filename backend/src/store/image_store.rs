use sqlx::{Row, SqlitePool};
use crate::models::image::{ImageEntry, CachedImage};

/// All successfully cached images for a listing (used for SHA-256 / phash dedup).
pub async fn list_cached_images(
    pool: &SqlitePool,
    listing_id: i64,
) -> Result<Vec<CachedImage>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT sha256, phash, local_path FROM images_cache
         WHERE listing_id = ? AND local_path IS NOT NULL",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| CachedImage {
            sha256: r.get("sha256"),
            phash: r.get("phash"),
            local_path: r.get("local_path"),
        })
        .collect())
}

/// Register an image URL for a listing at a given position.
/// If the URL already exists, its position is updated to reflect the latest
/// parser ordering. sha256/phash/local_path are left unchanged on conflict.
pub async fn insert_image_url(
    pool: &SqlitePool,
    listing_id: i64,
    source_url: &str,
    position: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO images_cache (listing_id, source_url, position, created_at)
         VALUES (?, ?, ?, datetime('now'))
         ON CONFLICT(listing_id, source_url) DO UPDATE SET position = excluded.position",
    )
    .bind(listing_id)
    .bind(source_url)
    .bind(position)
    .execute(pool)
    .await?;
    Ok(())
}

/// URLs that have been registered but not yet downloaded (local_path IS NULL).
pub async fn list_pending_image_urls(
    pool: &SqlitePool,
    listing_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT source_url FROM images_cache
         WHERE listing_id = ? AND local_path IS NULL ORDER BY position",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get("source_url")).collect())
}

/// Mark an image as cached by filling in sha256, phash, and local_path.
pub async fn update_cached_image(
    pool: &SqlitePool,
    listing_id: i64,
    source_url: &str,
    sha256: &str,
    phash: i64,
    local_path: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE images_cache SET sha256 = ?, phash = ?, local_path = ?
         WHERE listing_id = ? AND source_url = ?",
    )
    .bind(sha256)
    .bind(phash)
    .bind(local_path)
    .bind(listing_id)
    .bind(source_url)
    .execute(pool)
    .await?;
    Ok(())
}

/// Resolved images for a listing with metadata: local_path if cached, source_url as fallback.
pub async fn list_images_with_meta(
    pool: &SqlitePool,
    listing_id: i64,
) -> Result<Vec<ImageEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, COALESCE(local_path, source_url) AS url, created_at
         FROM images_cache WHERE listing_id = ? ORDER BY position",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| ImageEntry {
            id: r.get("id"),
            url: r.get("url"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// Returns the local_path for an image (None if not downloaded, or row not found).
/// The outer Option is None when the row doesn't exist for this listing.
pub async fn get_image_local_path(
    pool: &SqlitePool,
    image_id: i64,
    listing_id: i64,
) -> Result<Option<Option<String>>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT local_path FROM images_cache WHERE id = ? AND listing_id = ?",
    )
    .bind(image_id)
    .bind(listing_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.get::<Option<String>, _>("local_path")))
}

/// Delete an image_cache row. Call after removing any file from the object store.
pub async fn delete_image_record(
    pool: &SqlitePool,
    image_id: i64,
    listing_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM images_cache WHERE id = ? AND listing_id = ?")
        .bind(image_id)
        .bind(listing_id)
        .execute(pool)
        .await?;
    Ok(())
}
