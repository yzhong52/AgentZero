use serde::{Deserialize, Serialize};
use crate::models::image::ImageEntry;

/// A real estate property with all parsed and user-tracked fields.
/// Images are populated separately from the images_cache table.
#[derive(Serialize, Clone)]
pub struct Property {
    pub id: i64,

    // urls
    pub redfin_url: Option<String>,
    pub realtor_url: Option<String>,
    pub rew_url: Option<String>,

    pub title: String,
    pub description: String,
    // cost
    pub price: Option<i64>,
    pub price_currency: Option<String>,
    /// User's intended offer price — drives all mortgage calculations.
    /// When null the application falls back to `price` for calculations.
    pub offer_price: Option<i64>,
    // location
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    // interior
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    // building
    pub year_built: Option<i64>,
    // location
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    /// Populated from images_cache, not stored directly in listings.
    pub images: Vec<ImageEntry>,
    // misc
    pub created_at: String,
    pub updated_at: Option<String>,
    pub notes: Option<String>,
    // parking
    pub parking_garage: Option<i64>,
    pub parking_covered: Option<i64>,
    pub parking_open: Option<i64>,
    pub land_sqft: Option<i64>,
    // finance
    pub property_tax: Option<i64>,
    // location
    pub skytrain_station: Option<String>,
    pub skytrain_walk_min: Option<i64>,
    // amenities
    pub radiant_floor_heating: Option<bool>,
    pub ac: Option<bool>,
    // mortgage
    pub down_payment_pct: Option<f64>,
    pub mortgage_interest_rate: Option<f64>,
    pub amortization_years: Option<i64>,
    pub mortgage_monthly: Option<i64>,
    // cost
    pub hoa_monthly: Option<i64>,
    // Derived field: sum of mortgage, monthly property tax, and HOA fee
    pub monthly_total: Option<i64>,
    // Derived field: initial monthly mortgage interest + monthly property tax + HOA fee
    pub monthly_cost: Option<i64>,
    // rental
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,
    /// User-set status: "Interested" | "Pass" | "Buyable"
    pub status: Option<String>,
    /// User-assigned nickname / alias for this listing.
    pub nickname: Option<String>,
    // nearby schools (name + Fraser Institute rating 1-10)
    pub school_elementary: Option<String>,
    pub school_elementary_rating: Option<f64>,
    pub school_middle: Option<String>,
    pub school_middle_rating: Option<f64>,
    pub school_secondary: Option<String>,
    pub school_secondary_rating: Option<f64>,
}

/// All user-editable fields for a property.
/// Sent as the body of PATCH /api/listings/:id/details.
/// Every field is Option<T>; the frontend always sends the full current state
/// so that no field is unintentionally cleared.
#[derive(Deserialize)]
pub struct UserDetails {
    // source URLs (user can link or correct)
    pub redfin_url: Option<String>,
    pub realtor_url: Option<String>,
    pub rew_url: Option<String>,
    // core parsed fields (user can correct parser errors)
    pub price: Option<i64>,
    pub price_currency: Option<String>,
    /// User's intended offer price — drives mortgage calculations. Null means "use listing price".
    pub offer_price: Option<i64>,
    pub street_address: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub bedrooms: Option<i64>,
    pub bathrooms: Option<i64>,
    pub sqft: Option<i64>,
    pub year_built: Option<i64>,
    // parking
    pub parking_garage: Option<i64>,
    pub parking_covered: Option<i64>,
    pub parking_open: Option<i64>,
    pub land_sqft: Option<i64>,
    // finance
    pub property_tax: Option<i64>,
    // location
    pub skytrain_station: Option<String>,
    pub skytrain_walk_min: Option<i64>,
    // amenities
    pub radiant_floor_heating: Option<bool>,
    pub ac: Option<bool>,
    // mortgage
    pub down_payment_pct: Option<f64>,
    pub mortgage_interest_rate: Option<f64>,
    pub amortization_years: Option<i64>,
    pub mortgage_monthly: Option<i64>,
    pub hoa_monthly: Option<i64>,
    pub monthly_total: Option<i64>,
    pub monthly_cost: Option<i64>,
    // rental
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,
    // status / nickname
    pub status: Option<String>,
    // nearby schools
    pub school_elementary: Option<String>,
    pub school_elementary_rating: Option<f64>,
    pub school_middle: Option<String>,
    pub school_middle_rating: Option<f64>,
    pub school_secondary: Option<String>,
    pub school_secondary_rating: Option<f64>,
}
