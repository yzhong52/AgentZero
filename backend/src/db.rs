use serde::Serialize;
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

#[derive(Serialize, Clone)]
pub struct ImageEntry {
    pub id: i64,
    pub url: String,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct Property {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub description: String,
    pub price: Option<i64>,
    pub price_currency: Option<String>,
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    pub year_built: Option<i64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    /// Populated from images_cache, not stored directly in listings.
    pub images: Vec<ImageEntry>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

pub async fn init(database_url: &str) -> SqlitePool {
    let opts = SqliteConnectOptions::from_str(database_url)
        .expect("Invalid DATABASE_URL")
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .expect("Failed to connect to SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

pub async fn save(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO listings
               (url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(url) DO UPDATE SET
               title          = excluded.title,
               description    = excluded.description,
               price          = excluded.price,
               price_currency = excluded.price_currency,
               street_address = excluded.street_address,
               city           = excluded.city,
               region         = excluded.region,
               postal_code    = excluded.postal_code,
               country        = excluded.country,
               bedrooms       = excluded.bedrooms,
               bathrooms      = excluded.bathrooms,
               sqft           = excluded.sqft,
               year_built     = excluded.year_built,
               lat            = excluded.lat,
               lon            = excluded.lon,
               updated_at     = datetime('now')"#,
    )
    .bind(&p.url)
    .bind(&p.title)
    .bind(&p.description)
    .bind(p.price)
    .bind(&p.price_currency)
    .bind(&p.street_address)
    .bind(&p.city)
    .bind(&p.region)
    .bind(&p.postal_code)
    .bind(&p.country)
    .bind(p.bedrooms)
    .bind(p.bathrooms)
    .bind(p.sqft)
    .bind(p.year_built)
    .bind(p.lat)
    .bind(p.lon)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at
         FROM listings WHERE url = ?",
    )
    .bind(&p.url)
    .fetch_one(pool)
    .await?;

    Ok(row_to_property(&row))
}

pub async fn update_by_id(pool: &SqlitePool, id: i64, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
               title          = ?,
               description    = ?,
               price          = ?,
               price_currency = ?,
               street_address = ?,
               city           = ?,
               region         = ?,
               postal_code    = ?,
               country        = ?,
               bedrooms       = ?,
               bathrooms      = ?,
               sqft           = ?,
               year_built     = ?,
               lat            = ?,
               lon            = ?,
               updated_at     = datetime('now')
           WHERE id = ?"#,
    )
    .bind(&p.title)
    .bind(&p.description)
    .bind(p.price)
    .bind(&p.price_currency)
    .bind(&p.street_address)
    .bind(&p.city)
    .bind(&p.region)
    .bind(&p.postal_code)
    .bind(&p.country)
    .bind(p.bedrooms)
    .bind(p.bathrooms)
    .bind(p.sqft)
    .bind(p.year_built)
    .bind(p.lat)
    .bind(p.lon)
    .bind(id)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at
         FROM listings WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row_to_property(&row))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Property>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at
         FROM listings ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut properties: Vec<Property> = rows.iter().map(row_to_property).collect();

    for prop in &mut properties {
        prop.images = list_images_with_meta(pool, prop.id).await.unwrap_or_default();
    }

    Ok(properties)
}

fn row_to_property(row: &sqlx::sqlite::SqliteRow) -> Property {
    Property {
        id: row.get("id"),
        url: row.get("url"),
        title: row.get("title"),
        description: row.get("description"),
        price: row.get("price"),
        price_currency: row.get("price_currency"),
        street_address: row.get("street_address"),
        city: row.get("city"),
        region: row.get("region"),
        postal_code: row.get("postal_code"),
        country: row.get("country"),
        bedrooms: row.get("bedrooms"),
        bathrooms: row.get("bathrooms"),
        sqft: row.get("sqft"),
        year_built: row.get("year_built"),
        lat: row.get("lat"),
        lon: row.get("lon"),
        images: vec![], // populated separately from images_cache
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ---------------------------------------------------------------------------
// images_cache
// ---------------------------------------------------------------------------

/// An image that has been successfully downloaded and cached locally.
/// Only rows with non-null local_path are returned here (used for dedup).
pub struct CachedImage {
    pub sha256: String,
    pub phash: i64,
    pub local_path: String,
}

/// All successfully cached images for a listing (used for SHA-256 / dHash dedup).
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
