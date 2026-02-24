use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use crate::models::property::{Property, ListingStatus};
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
                    status,
                    school_elementary, school_elementary_rating,
                    school_middle, school_middle_rating,
                    school_secondary, school_secondary_rating,
                    property_type, listed_date, mls_number, laundry_in_unit";

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

/// Insert a new property and return it with the assigned id.
/// The caller is responsible for computing mortgage/monthly fields before calling this.
pub async fn add_listing(pool: &SqlitePool, p: &Property) -> Result<Property, sqlx::Error> {
    let row = sqlx::query(
        r#"INSERT INTO listings
               (redfin_url, realtor_url, rew_url, zillow_url,
                title, description, price, price_currency, offer_price,
                street_address, city, region, postal_code, country,
                bedrooms, bathrooms, sqft, year_built, lat, lon,
                parking_garage, parking_covered, parking_open, land_sqft,
                property_tax, skytrain_station, skytrain_walk_min,
                ac, radiant_floor_heating,
                down_payment_pct, mortgage_interest_rate, amortization_years, mortgage_monthly,
                hoa_monthly, monthly_total, monthly_cost,
                has_rental_suite, rental_income, status,
                school_elementary, school_elementary_rating,
                school_middle, school_middle_rating,
                school_secondary, school_secondary_rating,
                property_type, listed_date, mls_number, laundry_in_unit,
                created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                   ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
               ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
           RETURNING id"#,
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
    .bind(p.parking_covered)
    .bind(p.parking_open)
    .bind(p.land_sqft)
    .bind(p.property_tax)
    .bind(&p.skytrain_station)
    .bind(p.skytrain_walk_min)
    .bind(p.ac)
    .bind(p.radiant_floor_heating)
    .bind(p.down_payment_pct)
    .bind(p.mortgage_interest_rate)
    .bind(p.amortization_years)
    .bind(p.mortgage_monthly)
    .bind(p.hoa_monthly)
    .bind(p.monthly_total)
    .bind(p.monthly_cost)
    .bind(p.has_rental_suite)
    .bind(p.rental_income)
    .bind(&p.status)
    .bind(&p.school_elementary)
    .bind(p.school_elementary_rating)
    .bind(&p.school_middle)
    .bind(p.school_middle_rating)
    .bind(&p.school_secondary)
    .bind(p.school_secondary_rating)
    .bind(&p.property_type)
    .bind(&p.listed_date)
    .bind(&p.mls_number)
    .bind(p.laundry_in_unit)
    .fetch_one(pool)
    .await?;

    let new_id: i64 = row.get("id");
    fetch_one_by_id(pool, new_id).await
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
               property_type            = ?,
               listed_date              = ?,
               mls_number               = ?,
               laundry_in_unit          = ?,
               parking_covered          = ?,
               parking_open             = ?,
               property_tax             = ?,
               hoa_monthly              = ?,
               monthly_total            = ?,
               monthly_cost             = ?,
               skytrain_station         = ?,
               skytrain_walk_min        = ?,
               has_rental_suite         = ?,
               rental_income            = ?,
               status                   = ?,
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
    .bind(&p.property_type)
    .bind(&p.listed_date)
    .bind(&p.mls_number)
    .bind(p.laundry_in_unit)
    .bind(p.parking_covered)
    .bind(p.parking_open)
    .bind(p.property_tax)
    .bind(p.hoa_monthly)
    .bind(p.monthly_total)
    .bind(p.monthly_cost)
    .bind(&p.skytrain_station)
    .bind(p.skytrain_walk_min)
    .bind(p.has_rental_suite)
    .bind(p.rental_income)
    .bind(&p.status)
    .bind(id)
    .execute(pool)
    .await?;

    fetch_one_by_id(pool, id).await
}

/// Retrieve all properties ordered by created_at (newest first).
/// List properties, optionally filtered by status values.
///
/// - `statuses`: if empty, returns all properties.
/// - `statuses`: if non-empty, returns only rows whose `status` is in the given list.
pub async fn list(pool: &SqlitePool, statuses: &[ListingStatus]) -> Result<Vec<Property>, sqlx::Error> {
    let sql = if statuses.is_empty() {
        format!("SELECT {COLS} FROM listings ORDER BY created_at DESC")
    } else {
        let placeholders: Vec<String> = statuses.iter().map(|s| format!("'{s}'")).collect();
        format!("SELECT {COLS} FROM listings WHERE status IN ({}) ORDER BY created_at DESC", placeholders.join(","))
    };

    let rows = sqlx::query(&sql).fetch_all(pool).await?;

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
        school_elementary: row.get("school_elementary"),
        school_elementary_rating: row.get("school_elementary_rating"),
        school_middle: row.get("school_middle"),
        school_middle_rating: row.get("school_middle_rating"),
        school_secondary: row.get("school_secondary"),
        school_secondary_rating: row.get("school_secondary_rating"),
        property_type: row.get("property_type"),
        listed_date: row.get("listed_date"),
        mls_number: row.get("mls_number"),
        laundry_in_unit: row.get("laundry_in_unit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::property::UserDetails;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_add_listing_roundtrip() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let db_path = std::env::temp_dir().join(format!("agentzero_test_add_{}.db", now));
        let database_url = format!("sqlite://{}", db_path.display());
        let pool = init(&database_url).await;

        let p = Property {
            id: 0,
            redfin_url: Some("https://example.com/add".to_string()),
            realtor_url: Some("https://realtor.example/add".to_string()),
            rew_url: Some("https://rew.example/add".to_string()),
            zillow_url: None,
            title: "Add Listing Test".to_string(),
            description: "desc".to_string(),
            price: Some(700_000),
            price_currency: Some("CAD".to_string()),
            offer_price: Some(690_000),
            street_address: Some("999 Test Ave".to_string()),
            city: Some("Vancouver".to_string()),
            region: Some("BC".to_string()),
            postal_code: Some("V6B1A1".to_string()),
            country: Some("Canada".to_string()),
            bedrooms: Some(2),
            bathrooms: Some(2),
            sqft: Some(900),
            year_built: Some(2001),
            lat: Some(49.28),
            lon: Some(-123.12),
            images: vec![],
            created_at: String::new(),
            updated_at: None,
            notes: None,
            parking_garage: Some(1),
            parking_covered: Some(0),
            parking_open: Some(0),
            land_sqft: Some(1200),
            property_tax: Some(3200),
            skytrain_station: Some("Test Station".to_string()),
            skytrain_walk_min: Some(8),
            radiant_floor_heating: Some(false),
            ac: Some(true),
            down_payment_pct: Some(0.2),
            mortgage_interest_rate: Some(0.05),
            amortization_years: Some(25),
            mortgage_monthly: Some(2800),
            hoa_monthly: Some(400),
            monthly_total: Some(3466),
            monthly_cost: Some(3333),
            has_rental_suite: Some(false),
            rental_income: Some(0),
            status: ListingStatus::Interested,
            school_elementary: Some("Elm".to_string()),
            school_elementary_rating: Some(7.1),
            school_middle: Some("Oak".to_string()),
            school_middle_rating: Some(7.8),
            school_secondary: Some("Pine".to_string()),
            school_secondary_rating: Some(8.2),
            property_type: Some("Condo".to_string()),
            listed_date: Some("2026-02-24".to_string()),
            mls_number: Some("R9999999".to_string()),
            laundry_in_unit: Some(true),
        };

        let saved = add_listing(&pool, &p).await.expect("add_listing failed");
        assert!(saved.id > 0);
        assert_eq!(saved.title, "Add Listing Test");
        assert_eq!(saved.price, Some(700_000));
        assert_eq!(saved.property_type.as_deref(), Some("Condo"));
        assert_eq!(saved.listed_date.as_deref(), Some("2026-02-24"));
        assert_eq!(saved.mls_number.as_deref(), Some("R9999999"));
        assert_eq!(saved.laundry_in_unit, Some(true));

        let _ = std::fs::remove_file(db_path);
    }

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
            zillow_url: None,
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
            status: ListingStatus::Interested,
            school_elementary: None,
            school_elementary_rating: None,
            school_middle: None,
            school_middle_rating: None,
            school_secondary: None,
            school_secondary_rating: None,
            property_type: None,
            listed_date: None,
            mls_number: None,
            laundry_in_unit: None,
        };

        // Insert initial listing directly (avoid add_listing upsert complexity in tests)
        let _ = sqlx::query("INSERT INTO listings (redfin_url, title, description, price, price_currency, status, created_at) VALUES (?, ?, ?, ?, ?, 'Interested', datetime('now'))")
            .bind(&p.redfin_url)
            .bind(&p.title)
            .bind(&p.description)
            .bind(p.price)
            .bind(&p.price_currency)
            .execute(&pool)
            .await
            .expect("insert failed");

        let saved = list(&pool, &[]).await.expect("list failed").into_iter().next().expect("no listing"); // &[] = all
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

        // Insert initial listing with a title and known price
        let _ = sqlx::query("INSERT INTO listings (redfin_url, title, description, price, price_currency, status, created_at) VALUES (?, ?, ?, ?, ?, 'Interested', datetime('now'))")
            .bind(&Some("https://example.com/2".to_string()))
            .bind("Seed Title")
            .bind("")
            .bind(Some(100_000_i64))
            .bind(&Some("CAD".to_string()))
            .execute(&pool)
            .await
            .expect("insert failed");

        let saved = list(&pool, &[]).await.expect("list failed").into_iter().next().expect("no listing"); // &[] = all

        // Build UserDetails with every editable field set to a non-null value.
        let details = UserDetails {
            // Header
            title: Some("Updated Title".to_string()),
            // Price
            price: Some(110_000),
            price_currency: Some("CAD".to_string()),
            offer_price: Some(105_000),
            // Location
            street_address: Some("123 Test St".to_string()),
            city: Some("Vancouver".to_string()),
            region: Some("BC".to_string()),
            postal_code: Some("V1V1V1".to_string()),
            // Property facts
            bedrooms: Some(3),
            bathrooms: Some(2),
            sqft: Some(1200),
            year_built: Some(1990),
            // Parking / land
            parking_garage: Some(1),
            parking_covered: Some(1),
            parking_open: Some(0),
            land_sqft: Some(2000),
            // Features
            radiant_floor_heating: Some(true),
            ac: Some(false),
            // Transit
            skytrain_station: Some("Test Station".to_string()),
            skytrain_walk_min: Some(10),
            // Finance
            property_tax: Some(3000),
            hoa_monthly: Some(50),
            down_payment_pct: Some(0.2),
            mortgage_interest_rate: Some(0.035),
            amortization_years: Some(25),
            mortgage_monthly: Some(1500),
            monthly_total: Some(1750),
            monthly_cost: Some(1370),
            // Rental
            has_rental_suite: Some(true),
            rental_income: Some(800),
            // Schools
            school_elementary: Some("Elm Elementary".to_string()),
            school_elementary_rating: Some(7.5),
            school_middle: Some("Oak Middle".to_string()),
            school_middle_rating: Some(8.0),
            school_secondary: Some("Pine Secondary".to_string()),
            school_secondary_rating: Some(8.5),
            // Source URLs
            redfin_url: Some("https://redfin.example/2-updated".to_string()),
            realtor_url: Some("https://realtor.example/2".to_string()),
            rew_url: Some("https://rew.example/2".to_string()),
            zillow_url: Some("https://zillow.example/2".to_string()),
            // Listing metadata
            mls_number: Some("R3086230".to_string()),
            // Status
            status: Some(ListingStatus::Interested),
            // Property type and features
            property_type: Some("Townhouse".to_string()),
            laundry_in_unit: Some(true),
        };

        // Mirror the exact merge logic from patch_details in main.rs
        let mut merged = saved.clone();
        merged.title = details.title.clone().unwrap_or(merged.title.clone());
        if details.redfin_url.is_some() { merged.redfin_url = details.redfin_url.clone(); }
        if details.realtor_url.is_some() { merged.realtor_url = details.realtor_url.clone(); }
        if details.rew_url.is_some() { merged.rew_url = details.rew_url.clone(); }
        if details.zillow_url.is_some() { merged.zillow_url = details.zillow_url.clone(); }
        merged.price = details.price.or(merged.price);
        merged.price_currency = details.price_currency.clone().or(merged.price_currency.clone());
        merged.offer_price = details.offer_price.or(merged.offer_price);
        merged.street_address = details.street_address.clone().or(merged.street_address.clone());
        merged.city = details.city.clone().or(merged.city.clone());
        merged.region = details.region.clone().or(merged.region.clone());
        merged.postal_code = details.postal_code.clone().or(merged.postal_code.clone());
        merged.bedrooms = details.bedrooms.or(merged.bedrooms);
        merged.bathrooms = details.bathrooms.or(merged.bathrooms);
        merged.sqft = details.sqft.or(merged.sqft);
        merged.year_built = details.year_built.or(merged.year_built);
        merged.parking_garage = details.parking_garage.or(merged.parking_garage);
        merged.parking_covered = details.parking_covered.or(merged.parking_covered);
        merged.parking_open = details.parking_open.or(merged.parking_open);
        merged.land_sqft = details.land_sqft.or(merged.land_sqft);
        merged.radiant_floor_heating = details.radiant_floor_heating.or(merged.radiant_floor_heating);
        merged.ac = details.ac.or(merged.ac);
        merged.skytrain_station = details.skytrain_station.clone().or(merged.skytrain_station.clone());
        merged.skytrain_walk_min = details.skytrain_walk_min.or(merged.skytrain_walk_min);
        merged.property_tax = details.property_tax.or(merged.property_tax);
        merged.hoa_monthly = details.hoa_monthly.or(merged.hoa_monthly);
        merged.down_payment_pct = details.down_payment_pct.or(merged.down_payment_pct);
        merged.mortgage_interest_rate = details.mortgage_interest_rate.or(merged.mortgage_interest_rate);
        merged.amortization_years = details.amortization_years.or(merged.amortization_years);
        merged.mortgage_monthly = details.mortgage_monthly.or(merged.mortgage_monthly);
        merged.monthly_total = details.monthly_total.or(merged.monthly_total);
        merged.monthly_cost = details.monthly_cost.or(merged.monthly_cost);
        merged.has_rental_suite = details.has_rental_suite.or(merged.has_rental_suite);
        merged.rental_income = details.rental_income.or(merged.rental_income);
        merged.school_elementary = details.school_elementary.clone().or(merged.school_elementary.clone());
        merged.school_elementary_rating = details.school_elementary_rating.or(merged.school_elementary_rating);
        merged.school_middle = details.school_middle.clone().or(merged.school_middle.clone());
        merged.school_middle_rating = details.school_middle_rating.or(merged.school_middle_rating);
        merged.school_secondary = details.school_secondary.clone().or(merged.school_secondary.clone());
        merged.school_secondary_rating = details.school_secondary_rating.or(merged.school_secondary_rating);
        if let Some(s) = details.status.clone() { merged.status = s; }
        merged.property_type = details.property_type.clone().or(merged.property_type.clone());
        merged.laundry_in_unit = details.laundry_in_unit.or(merged.laundry_in_unit);
        merged.mls_number = details.mls_number.clone().or(merged.mls_number.clone());

        let updated = update_by_id(&pool, saved.id, &merged).await.expect("update_by_id failed");

        // Assert every field was persisted and round-tripped correctly.
        assert_eq!(updated.title, "Updated Title");
        // Price
        assert_eq!(updated.price, Some(110_000));
        assert_eq!(updated.price_currency.as_deref(), Some("CAD"));
        assert_eq!(updated.offer_price, Some(105_000));
        // Location
        assert_eq!(updated.street_address.as_deref(), Some("123 Test St"));
        assert_eq!(updated.city.as_deref(), Some("Vancouver"));
        assert_eq!(updated.region.as_deref(), Some("BC"));
        assert_eq!(updated.postal_code.as_deref(), Some("V1V1V1"));
        // Property facts
        assert_eq!(updated.bedrooms, Some(3));
        assert_eq!(updated.bathrooms, Some(2));
        assert_eq!(updated.sqft, Some(1200));
        assert_eq!(updated.year_built, Some(1990));
        // Parking / land
        assert_eq!(updated.parking_garage, Some(1));
        assert_eq!(updated.parking_covered, Some(1));
        assert_eq!(updated.parking_open, Some(0));
        assert_eq!(updated.land_sqft, Some(2000));
        // Features
        assert_eq!(updated.radiant_floor_heating, Some(true));
        assert_eq!(updated.ac, Some(false));
        // Transit
        assert_eq!(updated.skytrain_station.as_deref(), Some("Test Station"));
        assert_eq!(updated.skytrain_walk_min, Some(10));
        // Finance
        assert_eq!(updated.property_tax, Some(3000));
        assert_eq!(updated.hoa_monthly, Some(50));
        assert!((updated.down_payment_pct.unwrap() - 0.2).abs() < 1e-9);
        assert!((updated.mortgage_interest_rate.unwrap() - 0.035).abs() < 1e-9);
        assert_eq!(updated.amortization_years, Some(25));
        assert_eq!(updated.mortgage_monthly, Some(1500));
        assert_eq!(updated.monthly_total, Some(1750));
        assert_eq!(updated.monthly_cost, Some(1370));
        // Rental
        assert_eq!(updated.has_rental_suite, Some(true));
        assert_eq!(updated.rental_income, Some(800));
        // Schools
        assert_eq!(updated.school_elementary.as_deref(), Some("Elm Elementary"));
        assert!((updated.school_elementary_rating.unwrap() - 7.5).abs() < 1e-9);
        assert_eq!(updated.school_middle.as_deref(), Some("Oak Middle"));
        assert!((updated.school_middle_rating.unwrap() - 8.0).abs() < 1e-9);
        assert_eq!(updated.school_secondary.as_deref(), Some("Pine Secondary"));
        assert!((updated.school_secondary_rating.unwrap() - 8.5).abs() < 1e-9);
        // Source URLs
        assert_eq!(updated.redfin_url.as_deref(), Some("https://redfin.example/2-updated"));
        assert_eq!(updated.realtor_url.as_deref(), Some("https://realtor.example/2"));
        assert_eq!(updated.rew_url.as_deref(), Some("https://rew.example/2"));
        assert_eq!(updated.zillow_url.as_deref(), Some("https://zillow.example/2"));
        // Status
        assert_eq!(updated.status, ListingStatus::Interested);
        // Property type and new features
        assert_eq!(updated.property_type.as_deref(), Some("Townhouse"));
        assert_eq!(updated.laundry_in_unit, Some(true));
        assert_eq!(updated.mls_number.as_deref(), Some("R3086230"));

        let _ = std::fs::remove_file(db_path);
    }
}
