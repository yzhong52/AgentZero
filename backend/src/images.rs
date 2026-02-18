use crate::db;
use image::imageops::FilterType;
use object_store::{ObjectStore, ObjectStoreExt, path::Path as ObjectPath};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::fs;

/// Maximum Hamming distance between two dHashes to be considered duplicates.
const PHASH_THRESHOLD: u32 = 8;

/// Compute a 64-bit difference hash (dHash) for perceptual deduplication.
/// Resize to 9×8 grayscale, compare adjacent pixels in each row.
fn dhash(img: &image::DynamicImage) -> i64 {
    let small = img
        .resize_exact(9, 8, FilterType::Lanczos3)
        .grayscale();
    let pixels = small.to_luma8().into_raw();
    let mut hash: u64 = 0;
    for row in 0..8u32 {
        for col in 0..8u32 {
            let left = pixels[(row * 9 + col) as usize];
            let right = pixels[(row * 9 + col + 1) as usize];
            hash = (hash << 1) | if left > right { 1 } else { 0 };
        }
    }
    hash as i64
}

fn hamming(a: i64, b: i64) -> u32 {
    (a ^ b).count_ones()
}

/// Detect image format from bytes and return file extension.
fn image_ext(bytes: &[u8]) -> &'static str {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Jpeg) => "jpg",
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::WebP) => "webp",
        _ => "jpg",
    }
}

/// Download and cache all images for a listing.
///
/// Uses `store` (any `ObjectStore` impl) for writes — swap `LocalFileSystem`
/// for `AmazonS3`, `GoogleCloudStorage`, etc. with no other code changes.
///
/// `url_prefix` is prepended to object keys to build the public serve URL:
///   - Local:  "/images"               → "/images/<id>/<sha256>.jpg"
///   - S3:     "https://bucket.s3…"    → "https://bucket.s3…/<id>/<sha256>.jpg"
///
/// Dedup per listing:
///   1. Fast path: source URL already in images_cache → skip download.
///   2. SHA-256: exact byte-duplicate → reuse existing path.
///   3. dHash: perceptual duplicate (Hamming ≤ 8) → reuse existing path.
pub async fn cache_images(
    pool: &SqlitePool,
    client: &Client,
    store: &dyn ObjectStore,
    listing_id: i64,
    image_urls: &[String],
    url_prefix: &str,
) -> Vec<String> {
    // Load existing cached images for this listing (for SHA-256 / pHash comparison).
    let mut cached = db::list_cached_images(pool, listing_id)
        .await
        .unwrap_or_default();

    let mut resolved: Vec<String> = Vec::with_capacity(image_urls.len());

    for url in image_urls {
        // Fast path: URL already cached.
        match db::find_cached_by_url(pool, url).await {
            Ok(Some(serve_url)) => {
                resolved.push(serve_url);
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("DB lookup failed for {}: {}", url, e);
                resolved.push(url.clone());
                continue;
            }
        }

        // Download image bytes.
        let bytes = match client.get(url).send().await {
            Ok(resp) => match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("Failed to read image bytes {}: {}", url, e);
                    resolved.push(url.clone());
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("Failed to download image {}: {}", url, e);
                resolved.push(url.clone());
                continue;
            }
        };

        // SHA-256 — exact duplicate within this listing.
        let sha256 = hex::encode(Sha256::digest(&bytes));
        if let Some(c) = cached.iter().find(|c| c.sha256 == sha256) {
            resolved.push(c.local_path.clone());
            continue;
        }

        // Decode image for dHash.
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("Could not decode image {}: {}", url, e);
                resolved.push(url.clone());
                continue;
            }
        };

        let ph = dhash(&img);

        // dHash dedup — perceptual duplicate within this listing.
        if let Some(existing) = cached.iter().find(|c| hamming(c.phash, ph) <= PHASH_THRESHOLD) {
            tracing::debug!(
                "dHash duplicate for {} (distance={})",
                url,
                hamming(existing.phash, ph)
            );
            resolved.push(existing.local_path.clone());
            continue;
        }

        // Write to object store.
        let ext = image_ext(&bytes);
        let object_key = ObjectPath::from(format!("{}/{}.{}", listing_id, sha256, ext));
        let serve_url = format!("{}/{}", url_prefix, object_key);

        if let Err(e) = store.put(&object_key, bytes.clone().into()).await {
            tracing::warn!("Failed to write image to store {}: {}", object_key, e);
            resolved.push(url.clone());
            continue;
        }

        // Persist to DB.
        if let Err(e) =
            db::insert_cached_image(pool, listing_id, url, &sha256, ph, &serve_url).await
        {
            tracing::warn!("Failed to insert image cache row: {}", e);
        }

        // Update in-memory list for subsequent dedup checks.
        cached.push(db::CachedImage {
            sha256,
            phash: ph,
            local_path: serve_url.clone(),
        });

        resolved.push(serve_url);
    }

    resolved
}

/// Resolve a listing's stored image URLs: return cached serve URL if available,
/// else fall back to the original remote URL.
pub async fn resolve_images(pool: &SqlitePool, image_urls: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(image_urls.len());
    for url in image_urls {
        match db::find_cached_by_url(pool, url).await {
            Ok(Some(serve_url)) => out.push(serve_url),
            _ => out.push(url.clone()),
        }
    }
    out
}

/// Ensure the local images directory exists at startup.
/// Only needed for LocalFileSystem — cloud stores (S3, GCS) don't require this.
pub async fn ensure_images_dir(path: &str) {
    if let Err(e) = fs::create_dir_all(path).await {
        tracing::warn!("Could not create images dir {}: {}", path, e);
    }
}
