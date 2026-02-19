use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use crate::models::property::{Property, UserDetails};
use crate::store::image_store;

/// Initialize the database connection pool and run migrations.
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

/// Save a new property or update existing if URL already exists.
pub async fn save(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO listings
               (url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon,
                parking_garage, land_sqft, ac, radiant_floor_heating,
                updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(url) DO UPDATE SET
               title                = excluded.title,
               description          = excluded.description,
               price                = excluded.price,
               price_currency       = excluded.price_currency,
               street_address       = excluded.street_address,
               city                 = excluded.city,
               region               = excluded.region,
               postal_code          = excluded.postal_code,
               country              = excluded.country,
               bedrooms             = excluded.bedrooms,
               bathrooms            = excluded.bathrooms,
               sqft                 = excluded.sqft,
               year_built           = excluded.year_built,
               lat                  = excluded.lat,
               lon                  = excluded.lon,
               parking_garage       = excluded.parking_garage,
               land_sqft            = excluded.land_sqft,
               ac                   = excluded.ac,
               radiant_floor_heating = excluded.radiant_floor_heating,
               updated_at           = datetime('now')"#,
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
    .bind(p.parking_garage)
    .bind(p.land_sqft)
    .bind(p.ac)
    .bind(p.radiant_floor_heating)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at, notes,
                parking_garage, parking_covered, parking_open, land_sqft, property_tax,
                skytrain_station, skytrain_walk_min, radiant_floor_heating, ac,
                mortgage_monthly, hoa_monthly, monthly_total, has_rental_suite, rental_income
         FROM listings WHERE url = ?",
    )
    .bind(&p.url)
    .fetch_one(pool)
    .await?;

    Ok(row_to_property(&row))
}

/// Update an existing property by ID.
pub async fn update_by_id(pool: &SqlitePool, id: i64, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
               title                 = ?,
               description           = ?,
               price                 = ?,
               price_currency        = ?,
               street_address        = ?,
               city                  = ?,
               region                = ?,
               postal_code           = ?,
               country               = ?,
               bedrooms              = ?,
               bathrooms             = ?,
               sqft                  = ?,
               year_built            = ?,
               lat                   = ?,
               lon                   = ?,
               parking_garage        = ?,
               land_sqft             = ?,
               ac                    = ?,
               radiant_floor_heating = ?,
               updated_at            = datetime('now')
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
    .bind(p.parking_garage)
    .bind(p.land_sqft)
    .bind(p.ac)
    .bind(p.radiant_floor_heating)
    .bind(id)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at, notes,
                parking_garage, parking_covered, parking_open, land_sqft, property_tax,
                skytrain_station, skytrain_walk_min, radiant_floor_heating, ac,
                mortgage_monthly, hoa_monthly, monthly_total, has_rental_suite, rental_income
         FROM listings WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row_to_property(&row))
}

/// Retrieve all properties ordered by created_at (newest first).
pub async fn list(pool: &SqlitePool) -> Result<Vec<Property>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon, created_at, updated_at, notes,
                parking_garage, parking_covered, parking_open, land_sqft, property_tax,
                skytrain_station, skytrain_walk_min, radiant_floor_heating, ac,
                mortgage_monthly, hoa_monthly, monthly_total, has_rental_suite, rental_income
         FROM listings ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut properties: Vec<Property> = rows.iter().map(row_to_property).collect();

    for prop in &mut properties {
        prop.images = image_store::list_images_with_meta(pool, prop.id).await.unwrap_or_default();
    }

    Ok(properties)
}

/// Convert a database row to a Property instance.
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
        notes: row.get("notes"),
        parking_garage: row.get("parking_garage"),
        parking_covered: row.get("parking_covered"),
        parking_open: row.get("parking_open"),
        land_sqft: row.get("land_sqft"),
        property_tax: row.get("property_tax"),
        skytrain_station: row.get("skytrain_station"),
        skytrain_walk_min: row.get("skytrain_walk_min"),
        radiant_floor_heating: row.get("radiant_floor_heating"),
        ac: row.get("ac"),
        mortgage_monthly: row.get("mortgage_monthly"),
        hoa_monthly: row.get("hoa_monthly"),
        monthly_total: row.get("monthly_total"),
        has_rental_suite: row.get("has_rental_suite"),
        rental_income: row.get("rental_income"),
    }
}

/// Update user-tracked fields for a property.
pub async fn update_details(pool: &SqlitePool, id: i64, d: &UserDetails) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
               parking_garage = ?, parking_covered = ?, parking_open = ?,
               land_sqft = ?, property_tax = ?,
               skytrain_station = ?, skytrain_walk_min = ?,
               radiant_floor_heating = ?, ac = ?,
               mortgage_monthly = ?, hoa_monthly = ?, monthly_total = ?,
               has_rental_suite = ?, rental_income = ?
           WHERE id = ?"#,
    )
    .bind(d.parking_garage)
    .bind(d.parking_covered)
    .bind(d.parking_open)
    .bind(d.land_sqft)
    .bind(d.property_tax)
    .bind(&d.skytrain_station)
    .bind(d.skytrain_walk_min)
    .bind(d.radiant_floor_heating)
    .bind(d.ac)
    .bind(d.mortgage_monthly)
    .bind(d.hoa_monthly)
    .bind(d.monthly_total)
    .bind(d.has_rental_suite)
    .bind(d.rental_income)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update the notes field for a property.
pub async fn update_notes(pool: &SqlitePool, id: i64, notes: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE listings SET notes = ? WHERE id = ?")
        .bind(notes)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
