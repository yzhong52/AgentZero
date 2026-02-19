use serde::{Deserialize, Serialize};
use crate::models::image::ImageEntry;

/// A real estate property with all parsed and user-tracked fields.
/// Images are populated separately from the images_cache table.
#[derive(Serialize, Clone)]
pub struct Property {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub description: String,
    // cost
    pub price: Option<i64>,
    pub price_currency: Option<String>,
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
    // bulding
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
    // User-tracked fields (not populated by the parser)
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
    // finance
    pub mortgage_monthly: Option<i64>,
    // cost
    pub hoa_monthly: Option<i64>,
    // cost
    pub monthly_total: Option<i64>,
    // rental
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,
    /// User-set status: "Interested" | "Pass" | "Buyable"
    pub status: Option<String>,
    /// User-assigned nickname / alias for this listing.
    pub nickname: Option<String>,
}

/// User-provided details for a property (subset of Property fields).
/// Used for PATCH requests to update tracked information.
#[derive(Deserialize)]
pub struct UserDetails {
    pub parking_garage: Option<i64>,
    pub parking_covered: Option<i64>,
    pub parking_open: Option<i64>,
    pub land_sqft: Option<i64>,
    pub property_tax: Option<i64>,
    pub skytrain_station: Option<String>,
    pub skytrain_walk_min: Option<i64>,
    pub radiant_floor_heating: Option<bool>,
    pub ac: Option<bool>,
    pub mortgage_monthly: Option<i64>,
    pub hoa_monthly: Option<i64>,
    pub monthly_total: Option<i64>,
    pub has_rental_suite: Option<bool>,
    pub rental_income: Option<i64>,
    pub status: Option<String>,
}
