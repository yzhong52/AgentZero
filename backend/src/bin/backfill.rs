/// Backfill locally cached images for all existing listings.
///
/// Usage:
///   cd backend && cargo run --bin backfill
///
/// Env vars (same defaults as the main server):
///   DATABASE_URL      — sqlite://listings.db
///   IMAGES_DIR        — listings_images
///   IMAGES_URL_PREFIX — /images
use object_store::local::LocalFileSystem;
use property_parser::{db, images};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://listings.db".to_string());
    let images_dir =
        std::env::var("IMAGES_DIR").unwrap_or_else(|_| "listings_images".to_string());
    let images_url_prefix =
        std::env::var("IMAGES_URL_PREFIX").unwrap_or_else(|_| "/images".to_string());

    let pool = db::init(&database_url).await;

    images::ensure_images_dir(&images_dir).await;
    let store: Arc<dyn object_store::ObjectStore> = Arc::new(
        LocalFileSystem::new_with_prefix(std::path::Path::new(&images_dir))
            .expect("Failed to initialize local image store"),
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let listings = match db::list(&pool).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to list listings: {}", e);
            std::process::exit(1);
        }
    };

    let total = listings.len();
    println!("Found {} listing(s)", total);

    let mut total_newly_cached = 0usize;
    let mut total_pending = 0usize;

    for (i, listing) in listings.iter().enumerate() {
        let pending = match db::list_pending_image_urls(&pool, listing.id).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  Failed to list pending for listing {}: {}", listing.id, e);
                continue;
            }
        };

        let listing_url = listing.redfin_url.as_deref().or(listing.realtor_url.as_deref()).unwrap_or("?");
        println!("[{}/{}] {} — {} pending", i + 1, total, listing_url, pending.len());

        if pending.is_empty() {
            continue;
        }

        let newly_cached = images::cache_images(
            &pool,
            &client,
            store.as_ref(),
            listing.id,
            &images_url_prefix,
        )
        .await;

        println!("  {}/{} newly cached", newly_cached, pending.len());
        total_newly_cached += newly_cached;
        total_pending += pending.len();
    }

    println!(
        "\nDone. {}/{} images cached across {} listing(s).",
        total_newly_cached, total_pending, total
    );
}
