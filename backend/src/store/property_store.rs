use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use crate::models::property::{Property, UserDetails};
use crate::store::image_store;

// Common column list — keep in sync with row_to_property().
const COLS: &str = "id, redfin_url, realtor_url, rew_url, title, description, price, price_currency,
                    street_address, city, region, postal_code, country,
                    bedrooms, bathrooms, sqft, year_built, lat, lon,
                    created_at, updated_at, notes,
                    parking_garage, parking_covered, parking_open, land_sqft, property_tax,
                    skytrain_station, skytrain_walk_min, radiant_floor_heating, ac,
                    down_payment_pct, mortgage_interest_rate, amortization_years, mortgage_monthly,
                    hoa_monthly, monthly_total, has_rental_suite, rental_income,
                    status, nickname,
                    school_elementary, school_elementary_rating,
                    school_middle, school_middle_rating,
                    school_secondary, school_secondary_rating";

/// Initialize the database connection pool and run migrations.
pub async fn init(database_url: &str) -> SqlitePool {
    let opts = SqliteConnectOptions::from_str(database_url)
        .expect("Invalid DATABASE_URL")
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .expect("Failed to connect to SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Save (insert or update) a Redfin listing, deduplicating by redfin_url.
pub async fn save_redfin(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    let url = p.redfin_url.as_deref().unwrap_or("");
    sqlx::query(
        r#"INSERT INTO listings
               (redfin_url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon,
                parking_garage, land_sqft, ac, radiant_floor_heating,
                down_payment_pct, mortgage_interest_rate, amortization_years, mortgage_monthly,
                school_elementary, school_elementary_rating,
                school_middle, school_middle_rating,
                school_secondary, school_secondary_rating,
                updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(redfin_url) DO UPDATE SET
               title                    = excluded.title,
               description              = excluded.description,
               price                    = excluded.price,
               price_currency           = excluded.price_currency,
               street_address           = excluded.street_address,
               city                     = excluded.city,
               region                   = excluded.region,
               postal_code              = excluded.postal_code,
               country                  = excluded.country,
               bedrooms                 = excluded.bedrooms,
               bathrooms                = excluded.bathrooms,
               sqft                     = excluded.sqft,
               year_built               = excluded.year_built,
               lat                      = excluded.lat,
               lon                      = excluded.lon,
               parking_garage           = excluded.parking_garage,
               land_sqft                = excluded.land_sqft,
               ac                       = excluded.ac,
               radiant_floor_heating    = excluded.radiant_floor_heating,
               down_payment_pct         = excluded.down_payment_pct,
               mortgage_interest_rate   = excluded.mortgage_interest_rate,
               amortization_years       = excluded.amortization_years,
               mortgage_monthly         = excluded.mortgage_monthly,
               school_elementary        = excluded.school_elementary,
               school_elementary_rating = excluded.school_elementary_rating,
               school_middle            = excluded.school_middle,
               school_middle_rating     = excluded.school_middle_rating,
               school_secondary         = excluded.school_secondary,
               school_secondary_rating  = excluded.school_secondary_rating,
               updated_at               = datetime('now')"#,
    )
    .bind(&p.redfin_url)
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
    .bind(p.down_payment_pct)
    .bind(p.mortgage_interest_rate)
    .bind(p.amortization_years)
    .bind(p.mortgage_monthly)
    .bind(&p.school_elementary)
    .bind(p.school_elementary_rating)
    .bind(&p.school_middle)
    .bind(p.school_middle_rating)
    .bind(&p.school_secondary)
    .bind(p.school_secondary_rating)
    .execute(pool)
    .await?;

    fetch_one_by_redfin_url(pool, url).await
}

/// Save (insert or update) a Realtor.ca listing, deduplicating by realtor_url.
pub async fn save_realtor(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    let url = p.realtor_url.as_deref().unwrap_or("");
    sqlx::query(
        r#"INSERT INTO listings
               (realtor_url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon,
                parking_garage, land_sqft, ac, radiant_floor_heating,
                down_payment_pct, mortgage_interest_rate, amortization_years, mortgage_monthly,
                school_elementary, school_elementary_rating,
                school_middle, school_middle_rating,
                school_secondary, school_secondary_rating,
                updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(realtor_url) DO UPDATE SET
               title                    = excluded.title,
               description              = excluded.description,
               price                    = excluded.price,
               price_currency           = excluded.price_currency,
               street_address           = excluded.street_address,
               city                     = excluded.city,
               region                   = excluded.region,
               postal_code              = excluded.postal_code,
               country                  = excluded.country,
               bedrooms                 = excluded.bedrooms,
               bathrooms                = excluded.bathrooms,
               sqft                     = excluded.sqft,
               year_built               = excluded.year_built,
               lat                      = excluded.lat,
               lon                      = excluded.lon,
               parking_garage           = excluded.parking_garage,
               land_sqft                = excluded.land_sqft,
               ac                       = excluded.ac,
               radiant_floor_heating    = excluded.radiant_floor_heating,
               down_payment_pct         = excluded.down_payment_pct,
               mortgage_interest_rate   = excluded.mortgage_interest_rate,
               amortization_years       = excluded.amortization_years,
               mortgage_monthly         = excluded.mortgage_monthly,
               school_elementary        = excluded.school_elementary,
               school_elementary_rating = excluded.school_elementary_rating,
               school_middle            = excluded.school_middle,
               school_middle_rating     = excluded.school_middle_rating,
               school_secondary         = excluded.school_secondary,
               school_secondary_rating  = excluded.school_secondary_rating,
               updated_at               = datetime('now')"#,
    )
    .bind(&p.realtor_url)
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
    .bind(p.down_payment_pct)
    .bind(p.mortgage_interest_rate)
    .bind(p.amortization_years)
    .bind(p.mortgage_monthly)
    .bind(&p.school_elementary)
    .bind(p.school_elementary_rating)
    .bind(&p.school_middle)
    .bind(p.school_middle_rating)
    .bind(&p.school_secondary)
    .bind(p.school_secondary_rating)
    .execute(pool)
    .await?;

    fetch_one_by_realtor_url(pool, url).await
}

/// Save (insert or update) a rew.ca listing, deduplicating by rew_url.
pub async fn save_rew(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    let url = p.rew_url.as_deref().unwrap_or("");
    sqlx::query(
        r#"INSERT INTO listings
               (rew_url, title, description, price, price_currency,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon,
                parking_garage, land_sqft, property_tax, hoa_monthly,
                updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(rew_url) DO UPDATE SET
               title                    = excluded.title,
               description              = excluded.description,
               price                    = excluded.price,
               price_currency           = excluded.price_currency,
               street_address           = excluded.street_address,
               city                     = excluded.city,
               region                   = excluded.region,
               postal_code              = excluded.postal_code,
               country                  = excluded.country,
               bedrooms                 = excluded.bedrooms,
               bathrooms                = excluded.bathrooms,
               sqft                     = excluded.sqft,
               year_built               = excluded.year_built,
               lat                      = excluded.lat,
               lon                      = excluded.lon,
               parking_garage           = excluded.parking_garage,
               land_sqft                = excluded.land_sqft,
               property_tax             = excluded.property_tax,
               hoa_monthly              = excluded.hoa_monthly,
               updated_at               = datetime('now')"#,
    )
    .bind(&p.rew_url)
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
    .bind(p.property_tax)
    .bind(p.hoa_monthly)
    .execute(pool)
    .await?;

    fetch_one_by_rew_url(pool, url).await
}

/// Update an existing property by ID (called on refresh — overwrites parsed fields).
pub async fn update_by_id(pool: &SqlitePool, id: i64, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
               title                    = ?,
               description              = ?,
               price                    = ?,
               price_currency           = ?,
               street_address           = ?,
               city                     = ?,
               region                   = ?,
               postal_code              = ?,
               country                  = ?,
               bedrooms                 = ?,
               bathrooms                = ?,
               sqft                     = ?,
               year_built               = ?,
               lat                      = ?,
               lon                      = ?,
               parking_garage           = ?,
               land_sqft                = ?,
               ac                       = ?,
               radiant_floor_heating    = ?,
               down_payment_pct         = ?,
               mortgage_interest_rate   = ?,
               amortization_years       = ?,
               mortgage_monthly         = ?,
               school_elementary        = ?,
               school_elementary_rating = ?,
               school_middle            = ?,
               school_middle_rating     = ?,
               school_secondary         = ?,
               school_secondary_rating  = ?,
               parking_covered          = ?,
               parking_open             = ?,
               property_tax             = ?,
               hoa_monthly              = ?,
               monthly_total            = ?,
               updated_at               = datetime('now')
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
    .bind(p.down_payment_pct)
    .bind(p.mortgage_interest_rate)
    .bind(p.amortization_years)
    .bind(p.mortgage_monthly)
    .bind(&p.school_elementary)
    .bind(p.school_elementary_rating)
    .bind(&p.school_middle)
    .bind(p.school_middle_rating)
    .bind(&p.school_secondary)
    .bind(p.school_secondary_rating)
    .bind(p.parking_covered)
    .bind(p.parking_open)
    .bind(p.property_tax)
    .bind(p.hoa_monthly)
    .bind(p.monthly_total)
    .bind(id)
    .execute(pool)
    .await?;

    fetch_one_by_id(pool, id).await
}

/// Retrieve all properties ordered by created_at (newest first).
pub async fn list(pool: &SqlitePool) -> Result<Vec<Property>, sqlx::Error> {
    let rows = sqlx::query(&format!("SELECT {COLS} FROM listings ORDER BY created_at DESC"))
        .fetch_all(pool)
        .await?;

    let mut properties: Vec<Property> = rows.iter().map(row_to_property).collect();

    for prop in &mut properties {
        prop.images = image_store::list_images_with_meta(pool, prop.id).await.unwrap_or_default();
    }

    Ok(properties)
}

/// Fetch a single property by ID (with images).
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Property, sqlx::Error> {
    let mut p = fetch_one_by_id(pool, id).await?;
    p.images = image_store::list_images_with_meta(pool, p.id).await.unwrap_or_default();
    Ok(p)
}

/// Update the nickname/alias for a property.
pub async fn update_nickname(pool: &SqlitePool, id: i64, nickname: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE listings SET nickname = ? WHERE id = ?")
        .bind(nickname)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Update all user-editable fields; returns the refreshed record.
pub async fn update_details(pool: &SqlitePool, id: i64, d: &UserDetails) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
               redfin_url = ?, realtor_url = ?, rew_url = ?,
               price = ?, price_currency = ?,
               street_address = ?, city = ?, region = ?, postal_code = ?,
               bedrooms = ?, bathrooms = ?, sqft = ?, year_built = ?,
               parking_garage = ?, parking_covered = ?, parking_open = ?,
               land_sqft = ?, property_tax = ?,
               skytrain_station = ?, skytrain_walk_min = ?,
               radiant_floor_heating = ?, ac = ?,
               down_payment_pct = ?, mortgage_interest_rate = ?, amortization_years = ?,
               mortgage_monthly = ?, hoa_monthly = ?, monthly_total = ?,
               has_rental_suite = ?, rental_income = ?,
               status = ?,
               school_elementary = ?, school_elementary_rating = ?,
               school_middle = ?, school_middle_rating = ?,
               school_secondary = ?, school_secondary_rating = ?
           WHERE id = ?"#,
    )
    .bind(&d.redfin_url)
    .bind(&d.realtor_url)
    .bind(&d.rew_url)
    .bind(d.price)
    .bind(&d.price_currency)
    .bind(&d.street_address)
    .bind(&d.city)
    .bind(&d.region)
    .bind(&d.postal_code)
    .bind(d.bedrooms)
    .bind(d.bathrooms)
    .bind(d.sqft)
    .bind(d.year_built)
    .bind(d.parking_garage)
    .bind(d.parking_covered)
    .bind(d.parking_open)
    .bind(d.land_sqft)
    .bind(d.property_tax)
    .bind(&d.skytrain_station)
    .bind(d.skytrain_walk_min)
    .bind(d.radiant_floor_heating)
    .bind(d.ac)
    .bind(d.down_payment_pct)
    .bind(d.mortgage_interest_rate)
    .bind(d.amortization_years)
    .bind(d.mortgage_monthly)
    .bind(d.hoa_monthly)
    .bind(d.monthly_total)
    .bind(d.has_rental_suite)
    .bind(d.rental_income)
    .bind(&d.status)
    .bind(&d.school_elementary)
    .bind(d.school_elementary_rating)
    .bind(&d.school_middle)
    .bind(d.school_middle_rating)
    .bind(&d.school_secondary)
    .bind(d.school_secondary_rating)
    .bind(id)
    .execute(pool)
    .await?;

    fetch_one_by_id(pool, id).await
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

/// Delete a listing and all associated records (images cascade via FK).
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM listings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn fetch_one_by_id(pool: &SqlitePool, id: i64) -> Result<Property, sqlx::Error> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM listings WHERE id = ?"))
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row_to_property(&row))
}

async fn fetch_one_by_redfin_url(pool: &SqlitePool, url: &str) -> Result<Property, sqlx::Error> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM listings WHERE redfin_url = ?"))
        .bind(url)
        .fetch_one(pool)
        .await?;
    Ok(row_to_property(&row))
}

async fn fetch_one_by_realtor_url(pool: &SqlitePool, url: &str) -> Result<Property, sqlx::Error> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM listings WHERE realtor_url = ?"))
        .bind(url)
        .fetch_one(pool)
        .await?;
    Ok(row_to_property(&row))
}

async fn fetch_one_by_rew_url(pool: &SqlitePool, url: &str) -> Result<Property, sqlx::Error> {
    let row = sqlx::query(&format!("SELECT {COLS} FROM listings WHERE rew_url = ?"))
        .bind(url)
        .fetch_one(pool)
        .await?;
    Ok(row_to_property(&row))
}

/// Convert a database row to a Property instance.
fn row_to_property(row: &sqlx::sqlite::SqliteRow) -> Property {
    Property {
        id: row.get("id"),
        redfin_url: row.get("redfin_url"),
        realtor_url: row.get("realtor_url"),
        rew_url: row.get("rew_url"),
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
        images: vec![],
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
        down_payment_pct: row.get("down_payment_pct"),
        mortgage_interest_rate: row.get("mortgage_interest_rate"),
        amortization_years: row.get("amortization_years"),
        mortgage_monthly: row.get("mortgage_monthly"),
        hoa_monthly: row.get("hoa_monthly"),
        monthly_total: row.get("monthly_total"),
        has_rental_suite: row.get("has_rental_suite"),
        rental_income: row.get("rental_income"),
        status: row.get("status"),
        nickname: row.get("nickname"),
        school_elementary: row.get("school_elementary"),
        school_elementary_rating: row.get("school_elementary_rating"),
        school_middle: row.get("school_middle"),
        school_middle_rating: row.get("school_middle_rating"),
        school_secondary: row.get("school_secondary"),
        school_secondary_rating: row.get("school_secondary_rating"),
    }
}
