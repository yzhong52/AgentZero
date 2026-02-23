/// Zillow-specific listing parser.
///
/// Zillow includes structured data in:
///   - JSON-LD (`RealEstateListing` and `PropertyValue` objects) for core fields.
///   - Image URLs in og:image meta tags or within the JSON-LD.
///
/// Note: Zillow uses PerimeterX bot protection. To scrape Zillow, you must:
///   - Use a headless browser (Playwright, Selenium, Puppeteer)
///   - Pass the properly rendered HTML to this parser

use serde_json::Value as JsonValue;
use scraper::Html;

use crate::db;
use super::{ParsedListing, extract_json_ld, extract_title, extract_description, extract_images};

/// Attempts to extract a property from a Zillow listing page.
/// Looks for RealEstateListing or similar structured data in JSON-LD.
pub fn parse(url: &str, html: &str) -> Option<ParsedListing> {
    let document = Html::parse_document(html);

    // Extract JSON-LD structured data
    let json_lds = extract_json_ld(&document);
    let mut property = None;

    for json in json_lds {
        if let Some(extracted) = parse_real_estate_listing(&json, url) {
            property = Some(extracted);
            break;
        }
    }

    let property = property.or_else(|| parse_fallback(&document, url))?;

    // Extract images from meta tags and JSON-LD
    let image_urls = extract_images(&document);

    Some(ParsedListing {
        property,
        image_urls,
    })
}

/// Helper to construct a minimal Property struct.
fn new_property(url: &str) -> db::Property {
    db::Property {
        id: 0,
        redfin_url: None,
        realtor_url: None,
        rew_url: None,
        zillow_url: Some(url.to_string()),
        title: String::new(),
        description: String::new(),
        price: None,
        price_currency: None,
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
        images: Vec::new(),
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
    }
}

/// Parses a RealEstateListing JSON-LD object.
fn parse_real_estate_listing(json: &JsonValue, url: &str) -> Option<db::Property> {
    let obj = json.as_object()?;

    // Check if this is a RealEstateListing
    let type_str = obj.get("@type")?.as_str()?;
    if !type_str.contains("RealEstateListing") {
        return None;
    }

    let mut prop = new_property(url);

    // Title
    if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
        prop.title = name.to_string();
    }

    // Description
    if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
        prop.description = desc.to_string();
    }

    // Price
    if let Some(price_obj) = obj.get("price").and_then(|v| v.as_object()) {
        if let Some(price_str) = price_obj.get("price").and_then(|v| v.as_str()) {
            if let Ok(price) = price_str.replace("$", "").replace(",", "").parse::<i64>() {
                prop.price = Some(price);
            }
        }
    } else if let Some(price_str) = obj.get("price").and_then(|v| v.as_str()) {
        if let Ok(price) = price_str.replace("$", "").replace(",", "").parse::<i64>() {
            prop.price = Some(price);
        }
    }

    // Currency (default to USD for Zillow.com)
    prop.price_currency = Some("USD".to_string());

    // Address parsing
    if let Some(addr_obj) = obj.get("address").and_then(|v| v.as_object()) {
        if let Some(street) = addr_obj.get("streetAddress").and_then(|v| v.as_str()) {
            prop.street_address = Some(street.to_string());
        }
        if let Some(city) = addr_obj.get("addressLocality").and_then(|v| v.as_str()) {
            prop.city = Some(city.to_string());
        }
        if let Some(region) = addr_obj.get("addressRegion").and_then(|v| v.as_str()) {
            prop.region = Some(region.to_string());
        }
        if let Some(postal) = addr_obj.get("postalCode").and_then(|v| v.as_str()) {
            prop.postal_code = Some(postal.to_string());
        }
        if let Some(country) = addr_obj.get("addressCountry").and_then(|v| v.as_str()) {
            prop.country = Some(country.to_string());
        }
    }

    // Bedrooms
    if let Some(beds) = obj.get("numberOfRooms").and_then(|v| v.as_i64()) {
        prop.bedrooms = Some(beds);
    }

    // Bathrooms
    if let Some(baths) = obj.get("numberOfBathroomsTotal").and_then(|v| v.as_i64()) {
        prop.bathrooms = Some(baths);
    }

    // Square footage
    if let Some(floor_size) = obj.get("floorSize").and_then(|v| v.as_object()) {
        if let Some(sqft_str) = floor_size.get("value").and_then(|v| v.as_str()) {
            if let Ok(sqft) = sqft_str.replace(",", "").parse::<i64>() {
                prop.sqft = Some(sqft);
            }
        }
    }

    // Year built
    if let Some(year) = obj.get("yearBuilt").and_then(|v| v.as_i64()) {
        prop.year_built = Some(year);
    }

    // Geo coordinates
    if let Some(geo) = obj.get("geo").and_then(|v| v.as_object()) {
        if let Some(lat) = geo.get("latitude").and_then(|v| v.as_f64()) {
            prop.lat = Some(lat);
        }
        if let Some(lon) = geo.get("longitude").and_then(|v| v.as_f64()) {
            prop.lon = Some(lon);
        }
    }

    // Property type
    if let Some(prop_type) = obj.get("propertyType").and_then(|v| v.as_str()) {
        prop.status = Some(prop_type.to_string());
    }

    Some(prop)
}

/// Fallback parser for when JSON-LD is not available.
/// Uses meta tags and basic HTML scraping.
fn parse_fallback(document: &Html, url: &str) -> Option<db::Property> {
    let mut prop = new_property(url);

    // Use generic extractors
    prop.title = extract_title(document);
    prop.description = extract_description(document);
    prop.price_currency = Some("USD".to_string());

    // Try to extract price from the page title or description
    // (Zillow usually includes price in the title like "2223 Graveley St, Vancouver, BC V5L 3C1")
    if let Some(price) = extract_price_from_text(&prop.title) {
        prop.price = Some(price);
    }

    Some(prop)
}

/// Attempts to extract a price from text (e.g., "$999,000" -> 999000).
fn extract_price_from_text(text: &str) -> Option<i64> {
    let price_str: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .collect();

    if price_str.is_empty() {
        return None;
    }

    price_str.replace(",", "").parse().ok()
}
