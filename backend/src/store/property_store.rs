use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use crate::models::property::Property;
use crate::store::image_store;

// Common column list — keep in sync with row_to_property().
const COLS: &str = "id, redfin_url, realtor_url, rew_url, zillow_url, title, description, price, price_currency, offer_price,
                    street_address, city, region, postal_code, country,
                    bedrooms, bathrooms, sqft, year_built, lat, lon,
                    created_at, updated_at, notes,
                    parking_garage, parking_covered, parking_open, land_sqft, property_tax,
                    skytrain_station, skytrain_walk_min, radiant_floor_heating, ac,
                    down_payment_pct, mortgage_interest_rate, amortization_years, mortgage_monthly,
                    hoa_monthly, monthly_total, monthly_cost, has_rental_suite, rental_income,
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

/// Update an existing property by ID (called on refresh — overwrites parsed fields).
pub async fn update_by_id(pool: &SqlitePool, id: i64, p: &Property) -> Result<Property, sqlx::Error> {
    sqlx::query(
        r#"UPDATE listings SET
              redfin_url              = ?,
              realtor_url             = ?,
              rew_url                 = ?,
              zillow_url              = ?,
               title                    = ?,
               description              = ?,
               price                    = ?,
               price_currency           = ?,
               offer_price              = ?,
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
               monthly_cost             = ?,
               updated_at               = datetime('now')
           WHERE id = ?"#,
    )
    .bind(&p.redfin_url)
    .bind(&p.realtor_url)
    .bind(&p.rew_url)
    .bind(&p.zillow_url)
    .bind(&p.title)
    .bind(&p.description)
    .bind(p.price)
    .bind(&p.price_currency)
    .bind(p.offer_price)
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
    .bind(p.monthly_cost)
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
// `update_details` removed — use `update_by_id` after merging `UserDetails` in the caller.

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

/// Convert a database row to a Property instance.
fn row_to_property(row: &sqlx::sqlite::SqliteRow) -> Property {
    Property {
        id: row.get("id"),
        redfin_url: row.get("redfin_url"),
        realtor_url: row.get("realtor_url"),
        rew_url: row.get("rew_url"),
        zillow_url: row.get("zillow_url"),
        title: row.get("title"),
        description: row.get("description"),
        price: row.get("price"),
        price_currency: row.get("price_currency"),
        offer_price: row.get("offer_price"),
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
        monthly_cost: row.get("monthly_cost"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::property::UserDetails;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_update_by_id_roundtrip() {
        // Create a unique temporary database file in the system temp dir.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let db_path = std::env::temp_dir().join(format!("agentzero_test_{}.db", now));
        let database_url = format!("sqlite://{}", db_path.display());

        let pool = init(&database_url).await;

        // Construct a minimal property to save.
        let p = Property {
            id: 0,
            redfin_url: Some("https://example.com/1".to_string()),
            realtor_url: None,
            rew_url: None,
            title: "Original Title".to_string(),
            description: "".to_string(),
            price: Some(500_000),
            price_currency: Some("CAD".to_string()),
            offer_price: None,
            street_address: None,
            city: None,
            region: None,
            postal_code: None,
            country: None,
            bedrooms: None,
            bathrooms: None,
            sqft: None,
            year_built: None,
            lat: None,
            lon: None,
            images: vec![],
            created_at: String::new(),
            updated_at: None,
            notes: None,
            parking_garage: None,
            parking_covered: None,
            parking_open: None,
            land_sqft: None,
            property_tax: None,
            skytrain_station: None,
            skytrain_walk_min: None,
            radiant_floor_heating: None,
            ac: None,
            down_payment_pct: None,
            mortgage_interest_rate: None,
            amortization_years: None,
            mortgage_monthly: None,
            hoa_monthly: None,
            monthly_total: None,
            monthly_cost: None,
            has_rental_suite: None,
            rental_income: None,
            status: None,
            nickname: None,
            school_elementary: None,
            school_elementary_rating: None,
            school_middle: None,
            school_middle_rating: None,
            school_secondary: None,
            school_secondary_rating: None,
        };

        // Insert initial listing directly (avoid save_listing upsert complexity in tests)
        let _ = sqlx::query("INSERT INTO listings (redfin_url, title, description, price, price_currency, created_at) VALUES (?, ?, ?, ?, ?, datetime('now'))")
            .bind(&p.redfin_url)
            .bind(&p.title)
            .bind(&p.description)
            .bind(p.price)
            .bind(&p.price_currency)
            .execute(&pool)
            .await
            .expect("insert failed");

        let saved = list(&pool).await.expect("list failed").into_iter().next().expect("no listing");
        assert!(saved.id > 0, "expected saved id > 0");
        assert_eq!(saved.title, "Original Title");
        assert_eq!(saved.price, Some(500_000));

        // Update some fields and call update_by_id
        let mut updated = saved.clone();
        updated.title = "Updated Title".to_string();
        updated.price = Some(510_000);

        let after = update_by_id(&pool, saved.id, &updated).await.expect("update_by_id failed");
        assert_eq!(after.title, "Updated Title");
        assert_eq!(after.price, Some(510_000));

        // Cleanup the temporary DB file (best-effort)
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_update_details_roundtrip() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let db_path = std::env::temp_dir().join(format!("agentzero_test2_{}.db", now));
        let database_url = format!("sqlite://{}", db_path.display());
        let pool = init(&database_url).await;

        // Insert initial listing
        let _ = sqlx::query("INSERT INTO listings (redfin_url, title, description, price, price_currency, created_at) VALUES (?, ?, ?, ?, ?, datetime('now'))")
            .bind(&Some("https://example.com/2".to_string()))
            .bind("Seed Title")
            .bind("")
            .bind(Some(100_000_i64))
            .bind(&Some("CAD".to_string()))
            .execute(&pool)
            .await
            .expect("insert failed");

        let saved = list(&pool).await.expect("list failed").into_iter().next().expect("no listing");

        // Prepare user details to update many fields
        let details = UserDetails {
            redfin_url: Some("https://example.com/2-updated".to_string()),
            realtor_url: Some("https://realtor.example/2".to_string()),
            rew_url: Some("https://rew.example/2".to_string()),
            price: Some(110_000),
            price_currency: Some("CAD".to_string()),
            offer_price: None,
            street_address: Some("123 Test St".to_string()),
            city: Some("Vancouver".to_string()),
            region: Some("BC".to_string()),
            postal_code: Some("V1V1V1".to_string()),
            bedrooms: Some(3),
            bathrooms: Some(2),
            sqft: Some(1200),
            year_built: Some(1990),
            parking_garage: Some(1),
            parking_covered: Some(1),
            parking_open: Some(0),
            land_sqft: Some(2000),
            property_tax: Some(3000),
            skytrain_station: Some("Test Station".to_string()),
            skytrain_walk_min: Some(10),
            radiant_floor_heating: Some(true),
            ac: Some(false),
            down_payment_pct: Some(0.2),
            mortgage_interest_rate: Some(0.035),
            amortization_years: Some(25),
            mortgage_monthly: Some(1500),
            hoa_monthly: Some(50),
            monthly_total: Some(1750),
            monthly_cost: Some(1370),
            has_rental_suite: Some(false),
            rental_income: Some(0),
            status: Some("Interested".to_string()),
            school_elementary: Some("Elem".to_string()),
            school_elementary_rating: Some(7.5),
            school_middle: Some("Middle".to_string()),
            school_middle_rating: Some(8.0),
            school_secondary: Some("Secondary".to_string()),
            school_secondary_rating: Some(8.5),
        };

        // Merge details into the saved property and call update_by_id
        let mut merged = saved.clone();
        if details.redfin_url.is_some() { merged.redfin_url = details.redfin_url.clone(); }
        if details.realtor_url.is_some() { merged.realtor_url = details.realtor_url.clone(); }
        if details.rew_url.is_some() { merged.rew_url = details.rew_url.clone(); }
        merged.price = details.price.or(merged.price);
        merged.price_currency = details.price_currency.clone().or(merged.price_currency.clone());
        merged.city = details.city.clone().or(merged.city.clone());
        merged.bedrooms = details.bedrooms.or(merged.bedrooms);
        merged.bathrooms = details.bathrooms.or(merged.bathrooms);
        merged.sqft = details.sqft.or(merged.sqft);
        merged.radiant_floor_heating = details.radiant_floor_heating.or(merged.radiant_floor_heating);
        merged.mortgage_monthly = details.mortgage_monthly.or(merged.mortgage_monthly);

        let updated = update_by_id(&pool, saved.id, &merged).await.expect("update_by_id failed");

        assert_eq!(updated.redfin_url.as_deref(), Some("https://example.com/2-updated"));
        assert_eq!(updated.realtor_url.as_deref(), Some("https://realtor.example/2"));
        assert_eq!(updated.rew_url.as_deref(), Some("https://rew.example/2"));
        assert_eq!(updated.price, Some(110_000));
        assert_eq!(updated.price_currency.as_deref(), Some("CAD"));
        assert_eq!(updated.city.as_deref(), Some("Vancouver"));
        assert_eq!(updated.bedrooms, Some(3));
        assert_eq!(updated.bathrooms, Some(2));
        assert_eq!(updated.sqft, Some(1200));
        assert_eq!(updated.radiant_floor_heating, Some(true));
        assert_eq!(updated.mortgage_monthly, Some(1500));
        assert_eq!(updated.monthly_cost, Some(1370));

        let _ = std::fs::remove_file(db_path);
    }
}
