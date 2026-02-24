use serde::{Deserialize, Serialize};
use crate::models::image::ImageEntry;

/// A real estate property with all parsed and user-tracked fields.
/// Images are populated separately from the images_cache table.
///
/// Field annotations:
///   editable            — user can update via PATCH /api/listings/:id/details
///   parsed; editable    — initially filled by the parser; user can override
///   parsed; display only — filled by the parser; no UI edit control
///   derived; read-only  — recomputed server-side on every save
///   system              — managed entirely by the server / DB
#[derive(Serialize, Clone)]
pub struct Property {
    // ── System ──────────────────────────────────────────────────────────────
    pub id: i64,                          // system

    // ── Header ──────────────────────────────────────────────────────────────
    pub title: String,                    // parsed; editable (inline header, via PATCH /details)
    pub description: String,              // parsed; display only

    // ── Price ────────────────────────────────────────────────────────────────
    pub price: Option<i64>,               // parsed; editable
    pub price_currency: Option<String>,   // parsed; editable

    // ── Location ─────────────────────────────────────────────────────────────
    pub street_address: Option<String>,   // parsed; editable
    pub city: Option<String>,             // parsed; editable
    pub region: Option<String>,           // parsed; editable
    pub postal_code: Option<String>,      // parsed; editable
    pub country: Option<String>,          // parsed; display only
    pub lat: Option<f64>,                 // parsed; used for map embed
    pub lon: Option<f64>,                 // parsed; used for map embed

    // ── Property facts ───────────────────────────────────────────────────────
    pub property_type: Option<String>,    // parsed; editable (e.g. "Townhouse", "Single Family Residential")
    pub bedrooms: Option<i64>,            // parsed; editable
    pub bathrooms: Option<i64>,           // parsed; editable
    pub sqft: Option<i64>,                // parsed; editable
    pub land_sqft: Option<i64>,           // parsed; editable
    pub year_built: Option<i64>,          // parsed; editable

    // ── Parking ──────────────────────────────────────────────────────────────
    pub parking_garage: Option<i64>,      // parsed; editable
    pub parking_covered: Option<i64>,     // parsed; editable
    pub parking_open: Option<i64>,        // parsed; editable

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
    pub offer_price: Option<i64>,           // editable (Finance panel)
    pub property_tax: Option<i64>,          // parsed; editable (Finance panel)
    pub hoa_monthly: Option<i64>,           // parsed; editable (Finance panel)
    pub down_payment_pct: Option<f64>,      // editable (Finance panel)
    pub mortgage_interest_rate: Option<f64>, // editable (Finance panel)
    pub amortization_years: Option<i64>,    // editable (Finance panel)
    pub mortgage_monthly: Option<i64>,      // editable (Finance panel, overrides computed value)
    pub monthly_total: Option<i64>,         // derived; read-only (mortgage + tax + HOA)
    pub monthly_cost: Option<i64>,          // derived; read-only (initial interest + tax + HOA)

    // ── Rental ───────────────────────────────────────────────────────────────
    pub has_rental_suite: Option<bool>,   // editable
    pub rental_income: Option<i64>,       // editable

    // ── Schools ──────────────────────────────────────────────────────────────
    pub school_elementary: Option<String>,       // editable
    pub school_elementary_rating: Option<f64>,   // editable
    pub school_middle: Option<String>,           // editable
    pub school_middle_rating: Option<f64>,       // editable
    pub school_secondary: Option<String>,        // editable
    pub school_secondary_rating: Option<f64>,    // editable

    // ── Source URLs ──────────────────────────────────────────────────────────
    pub redfin_url: Option<String>,   // editable
    pub realtor_url: Option<String>,  // editable
    pub rew_url: Option<String>,      // editable
    pub zillow_url: Option<String>,   // editable

    // ── Listing metadata ─────────────────────────────────────────────────────
    pub mls_number: Option<String>,   // parsed; editable
    pub listed_date: Option<String>,  // parsed; display only (ISO date, e.g. "2026-02-17")

    // ── User notes / status ──────────────────────────────────────────────────
    /// User-set status: "Interested" | "Pass" | "Buyable"
    pub status: Option<String>,  // editable (status widget)
    pub notes: Option<String>,   // editable (via PATCH /notes)

    // ── System metadata ──────────────────────────────────────────────────────
    /// Populated from images_cache, not stored directly in listings.
    pub images: Vec<ImageEntry>,  // system
    pub created_at: String,       // system
    pub updated_at: Option<String>, // system
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

    // ── Location ─────────────────────────────────────────────────────────────
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,

    // ── Property facts ───────────────────────────────────────────────────────
    pub property_type: Option<String>,
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    pub land_sqft: Option<i64>,
    pub year_built: Option<i64>,

    // ── Parking ──────────────────────────────────────────────────────────────
    pub parking_garage: Option<i64>,
    pub parking_covered: Option<i64>,
    pub parking_open: Option<i64>,

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
    // ── Status ───────────────────────────────────────────────────────────────
    pub status: Option<String>,
}
