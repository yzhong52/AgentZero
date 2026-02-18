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

    let mut total_cached = 0usize;
    let mut total_images = 0usize;

    for (i, listing) in listings.iter().enumerate() {
        println!("[{}/{}] {}", i + 1, total, listing.url);

        if listing.images.is_empty() {
            println!("  no images");
            continue;
        }

        let resolved = images::cache_images(
            &pool,
            &client,
            store.as_ref(),
            listing.id,
            &listing.images,
            &images_url_prefix,
        )
        .await;

        let cached = resolved
            .iter()
            .filter(|u| u.starts_with(&images_url_prefix))
            .count();

        println!("  {}/{} images cached locally", cached, resolved.len());
        total_cached += cached;
        total_images += resolved.len();
    }

    println!("\nDone. {}/{} images cached across {} listing(s).", total_cached, total_images, total);
}
