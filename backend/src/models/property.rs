use crate::models::image::ImageEntry;
use crate::models::open_house::OpenHouse;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
#[cfg(test)]
use ts_rs::TS;

/// The user-facing status of a listing.
/// Stored in SQLite as its display name ("Interested", "Buyable", "Pass").
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "../../frontend/src/bindings/"))]
pub enum ListingStatus {
    /// Auto-added by Agent Zero; awaiting user review.
    Pending,
    #[default]
    Interested,
    Buyable,
    Pass,
}

impl std::fmt::Display for ListingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "Pending",
            Self::Interested => "Interested",
            Self::Buyable => "Buyable",
            Self::Pass => "Pass",
        })
    }
}

impl FromStr for ListingStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(Self::Pending),
            "Interested" => Ok(Self::Interested),
            "Buyable" => Ok(Self::Buyable),
            "Pass" => Ok(Self::Pass),
            _ => Err(format!("unknown listing status: {s}")),
        }
    }
}

// ── sqlx: store/retrieve as TEXT in SQLite ───────────────────────────────────

impl sqlx::Type<sqlx::Sqlite> for ListingStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for ListingStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(Into::into)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for ListingStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        use std::borrow::Cow;
        buf.push(sqlx::sqlite::SqliteArgumentValue::Text(Cow::Owned(
            self.to_string(),
        )));
        Ok(sqlx::encode::IsNull::No)
    }
}

/// The raw database row for a property listing — scalar fields only.
///
/// Images and open houses live in separate tables and are joined when building
/// the full [`Property`] API response via [`Property::from_stored`]. Internal
/// code (parsers, merge logic, store write-path) works exclusively with this
/// type so that the absence of joined fields is enforced by the compiler rather
/// than by convention.
#[derive(Serialize, Deserialize, Clone)]
pub struct StoredProperty {
    // ── System ──────────────────────────────────────────────────────────────
    pub id: i64,
    pub search_profile_id: i64,

    // ── Header ──────────────────────────────────────────────────────────────
    pub title: String,
    pub description: String,

    // ── Price ────────────────────────────────────────────────────────────────
    pub price: Option<i64>,
    pub price_currency: Option<String>,

    // ── Location ─────────────────────────────────────────────────────────────
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,

    // ── Property facts ───────────────────────────────────────────────────────
    pub property_type: Option<String>,
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    pub land_sqft: Option<i64>,
    pub year_built: Option<i64>,

    // ── Parking ──────────────────────────────────────────────────────────────
    pub parking_total: Option<i64>,
    pub parking_garage: Option<i64>,
    pub parking_carport: Option<i64>,
    pub parking_pad: Option<i64>,

    // ── Features ─────────────────────────────────────────────────────────────
    pub radiant_floor_heating: Option<bool>,
    pub ac: Option<bool>,
    pub laundry_in_unit: Option<bool>,

    // ── Transit ──────────────────────────────────────────────────────────────
    pub skytrain_station: Option<String>,
    pub skytrain_walk_min: Option<i64>,

    // ── Finance ──────────────────────────────────────────────────────────────
    pub offer_price: Option<i64>,
    pub property_tax: Option<i64>,
    pub hoa_monthly: Option<i64>,
    pub down_payment_pct: Option<f64>,
    pub mortgage_interest_rate: Option<f64>,
    pub amortization_years: Option<i64>,
    pub mortgage_monthly: Option<i64>,
    pub monthly_total: Option<i64>,
    pub monthly_cost: Option<i64>,

    // ── Rental ───────────────────────────────────────────────────────────────
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,

    // ── Schools ──────────────────────────────────────────────────────────────
    pub school_elementary: Option<String>,
    pub school_elementary_rating: Option<f64>,
    pub school_middle: Option<String>,
    pub school_middle_rating: Option<f64>,
    pub school_secondary: Option<String>,
    pub school_secondary_rating: Option<f64>,

    // ── Source URLs ──────────────────────────────────────────────────────────
    pub redfin_url: Option<String>,
    pub realtor_url: Option<String>,
    pub rew_url: Option<String>,
    pub zillow_url: Option<String>,

    // ── Listing metadata ─────────────────────────────────────────────────────
    pub mls_number: Option<String>,
    pub listed_date: Option<String>,
    pub source_status: Option<String>,

    // ── User notes / status ──────────────────────────────────────────────────
    pub status: ListingStatus,
    pub notes: Option<String>,

    // ── System metadata ──────────────────────────────────────────────────────
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// A real estate property with all parsed and user-tracked fields.
/// Images are populated separately from the images_cache table.
///
/// Field annotations:
///   editable            — user can update via PATCH /api/listings/:id/details
///   parsed; editable    — initially filled by the parser; user can override
///   parsed; display only — filled by the parser; no UI edit control
///   derived; read-only  — recomputed server-side on every save
///   system              — managed entirely by the server / DB
#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "../../frontend/src/bindings/"))]
pub struct Property {
    // ── System ──────────────────────────────────────────────────────────────
    pub id: i64,                // system
    pub search_profile_id: i64, // system — FK to search_profiles.id

    // ── Header ──────────────────────────────────────────────────────────────
    pub title: String,       // parsed; editable (inline header, via PATCH /details)
    pub description: String, // parsed; display only

    // ── Price ────────────────────────────────────────────────────────────────
    pub price: Option<i64>,             // parsed; editable
    pub price_currency: Option<String>, // parsed; editable

    // ── Location ─────────────────────────────────────────────────────────────
    pub street_address: Option<String>, // parsed; editable
    pub city: Option<String>,           // parsed; editable
    pub region: Option<String>,         // parsed; editable
    pub postal_code: Option<String>,    // parsed; editable
    pub country: Option<String>,        // parsed; display only
    pub lat: Option<f64>,               // parsed; used for map embed
    pub lon: Option<f64>,               // parsed; used for map embed

    // ── Property facts ───────────────────────────────────────────────────────
    pub property_type: Option<String>, // parsed; editable (e.g. "Townhouse", "Single Family Residential")
    pub bedrooms: Option<i64>,         // parsed; editable
    pub bathrooms: Option<i64>,        // parsed; editable
    pub sqft: Option<i64>,             // parsed; editable
    pub land_sqft: Option<i64>,        // parsed; editable
    pub year_built: Option<i64>,       // parsed; editable

    // ── Parking ──────────────────────────────────────────────────────────────
    pub parking_total: Option<i64>,   // parsed; editable
    pub parking_garage: Option<i64>,  // parsed; editable
    pub parking_carport: Option<i64>, // parsed; editable
    pub parking_pad: Option<i64>,     // parsed; editable

    // ── Features ─────────────────────────────────────────────────────────────
    pub radiant_floor_heating: Option<bool>, // parsed; editable
    pub ac: Option<bool>,                    // parsed; editable
    pub laundry_in_unit: Option<bool>,       // parsed; editable

    // ── Transit ──────────────────────────────────────────────────────────────
    pub skytrain_station: Option<String>, // editable
    pub skytrain_walk_min: Option<i64>,   // editable

    // ── Finance ──────────────────────────────────────────────────────────────
    /// User's intended offer price — drives all mortgage calculations.
    /// When null the application falls back to `price` for calculations.
    pub offer_price: Option<i64>, // editable (Finance panel)
    pub property_tax: Option<i64>, // parsed; editable (Finance panel)
    pub hoa_monthly: Option<i64>,  // parsed; editable (Finance panel)
    pub down_payment_pct: Option<f64>, // editable (Finance panel)
    pub mortgage_interest_rate: Option<f64>, // editable (Finance panel)
    pub amortization_years: Option<i64>, // editable (Finance panel)
    pub mortgage_monthly: Option<i64>, // derived; read-only (amortised monthly payment)
    pub monthly_total: Option<i64>, // derived; read-only (mortgage + tax + HOA)
    pub monthly_cost: Option<i64>, // derived; read-only (initial interest + tax + HOA)

    // ── Rental ───────────────────────────────────────────────────────────────
    pub has_rental_suite: Option<bool>, // editable
    pub rental_income: Option<i64>,     // editable

    // ── Schools ──────────────────────────────────────────────────────────────
    pub school_elementary: Option<String>,     // editable
    pub school_elementary_rating: Option<f64>, // editable
    pub school_middle: Option<String>,         // editable
    pub school_middle_rating: Option<f64>,     // editable
    pub school_secondary: Option<String>,      // editable
    pub school_secondary_rating: Option<f64>,  // editable

    // ── Source URLs ──────────────────────────────────────────────────────────
    pub redfin_url: Option<String>,  // editable
    pub realtor_url: Option<String>, // editable
    pub rew_url: Option<String>,     // editable
    pub zillow_url: Option<String>,  // editable

    // ── Listing metadata ─────────────────────────────────────────────────────
    pub mls_number: Option<String>,      // parsed; editable
    pub listed_date: Option<String>,     // parsed; display only (ISO date, e.g. "2026-02-17")
    pub source_status: Option<String>,   // parsed; display only (e.g. "OffMarket" when listing is gone)

    // ── User notes / status ──────────────────────────────────────────────────
    pub status: ListingStatus, // editable (status widget); never null, defaults to Interested
    pub notes: Option<String>, // editable (via PATCH /notes)

    // ── System metadata ──────────────────────────────────────────────────────
    /// Populated from images_cache, not stored directly in listings.
    pub images: Vec<ImageEntry>, // system
    /// Populated from open_houses table, not stored directly in listings.
    pub open_houses: Vec<OpenHouse>, // system
    pub created_at: String,         // system
    pub updated_at: Option<String>, // system
}

impl Property {
    /// Construct a full API `Property` from a DB row (`StoredProperty`) plus
    /// the joined data that lives in separate tables.  The compiler enforces
    /// that both `images` and `open_houses` are always explicitly provided.
    pub fn from_stored(
        s: StoredProperty,
        images: Vec<ImageEntry>,
        open_houses: Vec<OpenHouse>,
    ) -> Self {
        Property {
            id: s.id,
            search_profile_id: s.search_profile_id,
            title: s.title,
            description: s.description,
            price: s.price,
            price_currency: s.price_currency,
            street_address: s.street_address,
            city: s.city,
            region: s.region,
            postal_code: s.postal_code,
            country: s.country,
            lat: s.lat,
            lon: s.lon,
            property_type: s.property_type,
            bedrooms: s.bedrooms,
            bathrooms: s.bathrooms,
            sqft: s.sqft,
            land_sqft: s.land_sqft,
            year_built: s.year_built,
            parking_total: s.parking_total,
            parking_garage: s.parking_garage,
            parking_carport: s.parking_carport,
            parking_pad: s.parking_pad,
            radiant_floor_heating: s.radiant_floor_heating,
            ac: s.ac,
            laundry_in_unit: s.laundry_in_unit,
            skytrain_station: s.skytrain_station,
            skytrain_walk_min: s.skytrain_walk_min,
            offer_price: s.offer_price,
            property_tax: s.property_tax,
            hoa_monthly: s.hoa_monthly,
            down_payment_pct: s.down_payment_pct,
            mortgage_interest_rate: s.mortgage_interest_rate,
            amortization_years: s.amortization_years,
            mortgage_monthly: s.mortgage_monthly,
            monthly_total: s.monthly_total,
            monthly_cost: s.monthly_cost,
            has_rental_suite: s.has_rental_suite,
            rental_income: s.rental_income,
            school_elementary: s.school_elementary,
            school_elementary_rating: s.school_elementary_rating,
            school_middle: s.school_middle,
            school_middle_rating: s.school_middle_rating,
            school_secondary: s.school_secondary,
            school_secondary_rating: s.school_secondary_rating,
            redfin_url: s.redfin_url,
            realtor_url: s.realtor_url,
            rew_url: s.rew_url,
            zillow_url: s.zillow_url,
            mls_number: s.mls_number,
            listed_date: s.listed_date,
            source_status: s.source_status,
            status: s.status,
            notes: s.notes,
            created_at: s.created_at,
            updated_at: s.updated_at,
            images,
            open_houses,
        }
    }
}

impl From<Property> for StoredProperty {
    fn from(p: Property) -> Self {
        StoredProperty {
            id: p.id,
            search_profile_id: p.search_profile_id,
            title: p.title,
            description: p.description,
            price: p.price,
            price_currency: p.price_currency,
            street_address: p.street_address,
            city: p.city,
            region: p.region,
            postal_code: p.postal_code,
            country: p.country,
            lat: p.lat,
            lon: p.lon,
            property_type: p.property_type,
            bedrooms: p.bedrooms,
            bathrooms: p.bathrooms,
            sqft: p.sqft,
            land_sqft: p.land_sqft,
            year_built: p.year_built,
            parking_total: p.parking_total,
            parking_garage: p.parking_garage,
            parking_carport: p.parking_carport,
            parking_pad: p.parking_pad,
            radiant_floor_heating: p.radiant_floor_heating,
            ac: p.ac,
            laundry_in_unit: p.laundry_in_unit,
            skytrain_station: p.skytrain_station,
            skytrain_walk_min: p.skytrain_walk_min,
            offer_price: p.offer_price,
            property_tax: p.property_tax,
            hoa_monthly: p.hoa_monthly,
            down_payment_pct: p.down_payment_pct,
            mortgage_interest_rate: p.mortgage_interest_rate,
            amortization_years: p.amortization_years,
            mortgage_monthly: p.mortgage_monthly,
            monthly_total: p.monthly_total,
            monthly_cost: p.monthly_cost,
            has_rental_suite: p.has_rental_suite,
            rental_income: p.rental_income,
            school_elementary: p.school_elementary,
            school_elementary_rating: p.school_elementary_rating,
            school_middle: p.school_middle,
            school_middle_rating: p.school_middle_rating,
            school_secondary: p.school_secondary,
            school_secondary_rating: p.school_secondary_rating,
            redfin_url: p.redfin_url,
            realtor_url: p.realtor_url,
            rew_url: p.rew_url,
            zillow_url: p.zillow_url,
            mls_number: p.mls_number,
            listed_date: p.listed_date,
            source_status: p.source_status,
            status: p.status,
            notes: p.notes,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

/// All user-editable fields for a property.
/// Sent as the body of PATCH /api/listings/:id/details.
/// Every field is Option<T>; the frontend always sends the full current state
/// so that no field is unintentionally cleared.
#[derive(Deserialize)]
pub struct UserDetails {
    // ── Header ──────────────────────────────────────────────────────────────
    pub title: Option<String>,

    // ── Price ────────────────────────────────────────────────────────────────
    pub price: Option<i64>,
    pub price_currency: Option<String>,

    // ── Property facts ───────────────────────────────────────────────────────
    pub property_type: Option<String>,
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    pub land_sqft: Option<i64>,
    pub year_built: Option<i64>,

    // ── Parking ──────────────────────────────────────────────────────────────
    pub parking_total: Option<i64>,
    pub parking_garage: Option<i64>,
    pub parking_carport: Option<i64>,
    pub parking_pad: Option<i64>,

    // ── Features ─────────────────────────────────────────────────────────────
    pub radiant_floor_heating: Option<bool>,
    pub ac: Option<bool>,
    pub laundry_in_unit: Option<bool>,

    // ── Transit ──────────────────────────────────────────────────────────────
    pub skytrain_station: Option<String>,
    pub skytrain_walk_min: Option<i64>,

    // ── Finance ──────────────────────────────────────────────────────────────
    /// User's intended offer price — drives mortgage calculations. Null means "use listing price".
    pub offer_price: Option<i64>,
    pub property_tax: Option<i64>,
    pub hoa_monthly: Option<i64>,
    pub down_payment_pct: Option<f64>,
    pub mortgage_interest_rate: Option<f64>,
    pub amortization_years: Option<i64>,
    pub monthly_total: Option<i64>,
    pub monthly_cost: Option<i64>,

    // ── Rental ───────────────────────────────────────────────────────────────
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,

    // ── Schools ──────────────────────────────────────────────────────────────
    pub school_elementary: Option<String>,
    pub school_elementary_rating: Option<f64>,
    pub school_middle: Option<String>,
    pub school_middle_rating: Option<f64>,
    pub school_secondary: Option<String>,
    pub school_secondary_rating: Option<f64>,

    // ── Source URLs ──────────────────────────────────────────────────────────
    pub redfin_url: Option<String>,
    pub realtor_url: Option<String>,
    pub rew_url: Option<String>,
    pub zillow_url: Option<String>,
    // ── Listing metadata ─────────────────────────────────────────────────────
    pub mls_number: Option<String>,
    // ── Status ───────────────────────────────────────────────────────────────
    pub status: Option<ListingStatus>,
}
