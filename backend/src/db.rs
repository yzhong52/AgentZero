use serde::Serialize;
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

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
    pub images: Vec<String>,
    pub created_at: String,
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
    let images = serde_json::to_string(&p.images).unwrap_or_else(|_| "[]".to_string());

    sqlx::query(
        r#"INSERT INTO listings
               (url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, images)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
               images         = excluded.images"#,
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
    .bind(&images)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, images, created_at
         FROM listings WHERE url = ?",
    )
    .bind(&p.url)
    .fetch_one(pool)
    .await?;

    Ok(row_to_property(&row))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Property>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, images, created_at
         FROM listings ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(row_to_property).collect())
}

fn row_to_property(row: &sqlx::sqlite::SqliteRow) -> Property {
    let images_str: &str = row.get("images");
    let images: Vec<String> = serde_json::from_str(images_str).unwrap_or_default();

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
        images,
        created_at: row.get("created_at"),
    }
}
