use crate::db;
use image::imageops::FilterType;
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
/// - Skips URLs already in images_cache (fast path: source_url lookup).
/// - Skips exact byte-duplicates via SHA-256.
/// - Skips perceptual duplicates via dHash with Hamming distance ≤ PHASH_THRESHOLD.
/// Returns resolved image URLs: local `/images/<id>/<sha256>.<ext>` when cached,
/// or the original remote URL as fallback.
pub async fn cache_images(
    pool: &SqlitePool,
    client: &Client,
    listing_id: i64,
    image_urls: &[String],
    images_dir: &str,
) -> Vec<String> {
    let dir = format!("{}/{}", images_dir, listing_id);
    if let Err(e) = fs::create_dir_all(&dir).await {
        tracing::warn!("Could not create image dir {}: {}", dir, e);
        return image_urls.to_vec();
    }

    // Load existing cached images for this listing (for pHash comparison).
    let mut cached = db::list_cached_images(pool, listing_id)
        .await
        .unwrap_or_default();

    let mut resolved: Vec<String> = Vec::with_capacity(image_urls.len());

    for url in image_urls {
        // Fast path: URL already cached.
        match db::find_cached_by_url(pool, url).await {
            Ok(Some(local_path)) => {
                resolved.push(local_path);
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

        // SHA-256.
        let sha256 = hex::encode(Sha256::digest(&bytes));

        // Check SHA-256 dupe within this listing.
        if cached.iter().any(|c| c.sha256 == sha256) {
            // Exact duplicate — reuse existing local_path.
            if let Some(c) = cached.iter().find(|c| c.sha256 == sha256) {
                resolved.push(c.local_path.clone());
            } else {
                resolved.push(url.clone());
            }
            continue;
        }

        // Decode image for pHash.
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("Could not decode image {}: {}", url, e);
                resolved.push(url.clone());
                continue;
            }
        };

        let ph = dhash(&img);

        // pHash dedup: compare against all cached hashes for this listing.
        if let Some(existing) = cached.iter().find(|c| hamming(c.phash, ph) <= PHASH_THRESHOLD) {
            tracing::debug!(
                "pHash duplicate detected for {} (distance={})",
                url,
                hamming(existing.phash, ph)
            );
            resolved.push(existing.local_path.clone());
            continue;
        }

        // Write file.
        let ext = image_ext(&bytes);
        let filename = format!("{}.{}", sha256, ext);
        let local_path = format!("{}/{}", dir, filename);
        let serve_path = format!("/images/{}/{}", listing_id, filename);

        if let Err(e) = fs::write(&local_path, &bytes).await {
            tracing::warn!("Failed to write image {}: {}", local_path, e);
            resolved.push(url.clone());
            continue;
        }

        // Persist to DB.
        if let Err(e) =
            db::insert_cached_image(pool, listing_id, url, &sha256, ph, &serve_path).await
        {
            tracing::warn!("Failed to insert image cache row: {}", e);
        }

        // Update in-memory list for subsequent pHash checks.
        cached.push(db::CachedImage {
            sha256,
            phash: ph,
            local_path: serve_path.clone(),
        });

        resolved.push(serve_path);
    }

    resolved
}

/// Resolve a listing's stored image URLs: return local path if cached, else remote URL.
pub async fn resolve_images(pool: &SqlitePool, image_urls: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(image_urls.len());
    for url in image_urls {
        match db::find_cached_by_url(pool, url).await {
            Ok(Some(local)) => out.push(local),
            _ => out.push(url.clone()),
        }
    }
    out
}

/// Ensure the images directory exists at startup.
pub async fn ensure_images_dir(path: &str) {
    if let Err(e) = fs::create_dir_all(path).await {
        tracing::warn!("Could not create images dir {}: {}", path, e);
    }
}

