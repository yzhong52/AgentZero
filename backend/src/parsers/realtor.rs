/// Realtor.ca listing parser.
///
/// Realtor.ca is a Next.js single-page app.  Data lives in two places:
///   1. A `<script id="__NEXT_DATA__" type="application/json">` block that
///      embeds the full server-side props, including a listing object that
///      mirrors the realtor.ca internal API format.
///   2. Standard JSON-LD `RealEstateListing` blocks (for SEO / fallback).
///
/// Field paths tried for the `__NEXT_DATA__` JSON:
///   props.pageProps.listing  (most common)
///   props.pageProps.property
///   props.pageProps.propertyDetails

use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value as JsonValue;
use std::sync::OnceLock;

use crate::db;
use super::{ParsedListing, extract_json_ld, extract_images, extract_title, extract_description};

// ── Static regexes ────────────────────────────────────────────────────────────

static SQFT_RE: OnceLock<Regex> = OnceLock::new();

fn sqft_re() -> &'static Regex {
    SQFT_RE.get_or_init(|| Regex::new(r"(?i)([\d,]+)\s*sq\.?\s*ft").unwrap())
}

// ── __NEXT_DATA__ extraction ──────────────────────────────────────────────────

/// Extract the Next.js page data embedded in `<script id="__NEXT_DATA__">`.
fn extract_next_data(document: &Html) -> Option<JsonValue> {
    let sel = Selector::parse("script#__NEXT_DATA__").unwrap();
    let el = document.select(&sel).next()?;
    let text = el.first_child()?.value().as_text()?;
    serde_json::from_str(text.trim()).ok()
}

/// Walk several candidate paths for the listing object inside __NEXT_DATA__.
fn find_listing_object(root: &JsonValue) -> Option<&JsonValue> {
    let page = root.get("props")?.get("pageProps")?;
    for key in &["listing", "property", "propertyDetails"] {
        if let Some(obj) = page.get(key) {
            if obj.is_object() {
                return Some(obj);
            }
        }
    }
    None
}

// ── Field helpers ─────────────────────────────────────────────────────────────

/// Parse a price string like "$2,298,000" → 2298000.
fn parse_price_str(s: &str) -> Option<i64> {
    let clean: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    clean.parse().ok()
}

/// Parse a sqft string like "2,718.00 sqft" → 2718.
fn parse_sqft_str(s: &str) -> Option<i64> {
    if let Some(cap) = sqft_re().captures(s) {
        let digits: String = cap[1].chars().filter(|c| c.is_ascii_digit()).collect();
        return digits.parse().ok();
    }
    None
}

/// Parse latitude/longitude that may be stored as a string or number.
fn parse_coord(v: &JsonValue) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

// ── Extraction from realtor.ca __NEXT_DATA__ listing object ──────────────────

/// The realtor.ca listing object has a shape like:
/// ```json
/// {
///   "MlsNumber": "R2875xxx",
///   "Property": {
///     "Price": "$2,298,000",
///     "PriceUnformattedValue": "2298000",
///     "Type": "Single Family",
///     "Address": {
///       "AddressText": "1278 E 13TH AV|Vancouver, British Columbia V5T 2M1",
///       "Longitude": "-123.xxx",
///       "Latitude": "49.xxx"
///     },
///     "Photo": [{"LargePhotoUrl": "..."}, ...],
///     "Parking": [{"Label": "Garage", "Spaces": "2"}]
///   },
///   "Building": {
///     "BathroomTotal": "3",
///     "BedroomsTotal": "5",
///     "SizeInterior": "2718.00 sqft",
///     "YearBuilt": "1921"
///   },
///   "Land": {
///     "SizeTotal": "5000 sqft"
///   }
/// }
/// ```
fn extract_from_next_listing(url: &str, title: &str, description: &str, listing: &JsonValue) -> Option<db::Property> {
    let prop    = listing.get("Property").unwrap_or(&JsonValue::Null);
    let build   = listing.get("Building").unwrap_or(&JsonValue::Null);
    let land    = listing.get("Land").unwrap_or(&JsonValue::Null);
    let address = prop.get("Address").unwrap_or(&JsonValue::Null);

    // Price
    let price: Option<i64> = prop["PriceUnformattedValue"].as_i64()
        .or_else(|| prop["PriceUnformattedValue"].as_str()?.parse().ok())
        .or_else(|| prop["Price"].as_str().and_then(parse_price_str));

    // Address: "1278 E 13TH AV|Vancouver, British Columbia V5T 2M1"
    let addr_text = address["AddressText"].as_str().unwrap_or("");
    let (street_address, city, region, postal_code) = parse_address_text(addr_text);

    // Coordinates
    let lat = parse_coord(&address["Latitude"]);
    let lon = parse_coord(&address["Longitude"]);

    // Bedrooms / bathrooms
    let bedrooms  = build["BedroomsTotal"].as_str().and_then(|s| s.parse().ok())
        .or_else(|| build["BedroomsTotal"].as_i64());
    let bathrooms = build["BathroomTotal"].as_str().and_then(|s| s.parse::<f64>().ok()).map(|v| v as i64)
        .or_else(|| build["BathroomTotal"].as_i64());

    // Interior sqft
    let sqft = build["SizeInterior"].as_str().and_then(parse_sqft_str)
        .or_else(|| build["SizeInterior"].as_i64());

    // Year built
    let year_built = build["YearBuilt"].as_str().and_then(|s| s.parse().ok())
        .or_else(|| build["YearBuilt"].as_i64());

    // Land sqft
    let land_sqft = land["SizeTotal"].as_str().and_then(parse_sqft_str);

    // Parking: array of {Label, Spaces}
    let parking_garage = prop["Parking"].as_array().and_then(|arr| {
        arr.iter().find(|p| {
            p["Label"].as_str().map(|l| l.to_lowercase().contains("garage")).unwrap_or(false)
        }).and_then(|p| {
            p["Spaces"].as_str().and_then(|s| s.parse().ok())
                .or_else(|| p["Spaces"].as_i64())
        })
    });

    // Images: Photo array
    let _image_urls: Vec<String> = prop["Photo"].as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    p["LargePhotoUrl"].as_str()
                        .or_else(|| p["HighResPhotoUrl"].as_str())
                        .or_else(|| p["MedResPhotoUrl"].as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    // Property tax
    let property_tax = listing["PropertyTaxYear"].as_i64()
        .and(listing["PropertyTax"].as_str().and_then(|s| s.replace(['$', ','], "").trim().parse().ok()));

    Some(db::Property {
        id: 0,
        redfin_url: None,
        realtor_url: Some(url.to_string()),
        title: title.to_string(),
        description: description.to_string(),
        price,
        price_currency: Some("CAD".to_string()),
        street_address,
        city,
        region,
        postal_code,
        country: Some("Canada".to_string()),
        bedrooms,
        bathrooms,
        sqft,
        year_built,
        lat,
        lon,
        images: Vec::new(),
        created_at: String::new(),
        updated_at: None,
        notes: None,
        parking_garage,
        parking_covered: None,
        parking_open: None,
        land_sqft,
        property_tax,
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
    })
}

/// Parse "1278 E 13TH AV|Vancouver, British Columbia V5T 2M1"
/// into (street_address, city, region, postal_code).
fn parse_address_text(text: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    if text.is_empty() {
        return (None, None, None, None);
    }
    let parts: Vec<&str> = text.splitn(2, '|').collect();
    let street_address = Some(parts[0].trim().to_string());
    if parts.len() < 2 {
        return (street_address, None, None, None);
    }
    // "Vancouver, British Columbia V5T 2M1"
    let rest = parts[1].trim();
    // Postal code: last token matching Canadian postal code pattern
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let (postal_code, rest_without_postal) = if tokens.len() >= 2 {
        let last2 = format!("{} {}", tokens[tokens.len()-2], tokens[tokens.len()-1]);
        // Canadian postal: A1A 1A1 format
        if is_canadian_postal(&last2) {
            let without = rest[..rest.len()-last2.len()].trim_end_matches([',', ' ']).to_string();
            (Some(last2), without)
        } else {
            (None, rest.to_string())
        }
    } else {
        (None, rest.to_string())
    };

    // "Vancouver, British Columbia"
    let city_region: Vec<&str> = rest_without_postal.splitn(2, ',').collect();
    let city   = Some(city_region[0].trim().to_string());
    let region = if city_region.len() > 1 { Some(city_region[1].trim().to_string()) } else { None };

    (street_address, city, region, postal_code)
}

fn is_canadian_postal(s: &str) -> bool {
    // Simple check: "A1A 1A1"
    let b = s.as_bytes();
    b.len() == 7
        && b[0].is_ascii_alphabetic()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_alphabetic()
        && b[3] == b' '
        && b[4].is_ascii_digit()
        && b[5].is_ascii_alphabetic()
        && b[6].is_ascii_digit()
}

// ── Image extraction from __NEXT_DATA__ ───────────────────────────────────────

fn extract_next_image_urls(listing: &JsonValue) -> Vec<String> {
    listing["Property"]["Photo"].as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    p["LargePhotoUrl"].as_str()
                        .or_else(|| p["HighResPhotoUrl"].as_str())
                        .or_else(|| p["MedResPhotoUrl"].as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ── JSON-LD fallback ──────────────────────────────────────────────────────────

/// Fallback: use the shared RealEstateListing JSON-LD parser and override the URL fields.
fn extract_from_json_ld(url: &str, title: &str, json_ld: &[JsonValue]) -> Option<db::Property> {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    })?;

    let entity = &listing["mainEntity"];
    let addr   = &entity["address"];

    Some(db::Property {
        id: 0,
        redfin_url: None,
        realtor_url: Some(url.to_string()),
        title: title.to_string(),
        description: listing["description"].as_str().unwrap_or("").to_string(),
        price: listing["offers"]["price"].as_i64(),
        price_currency: listing["offers"]["priceCurrency"].as_str().map(str::to_string)
            .or_else(|| Some("CAD".to_string())),
        street_address: addr["streetAddress"].as_str().map(str::to_string),
        city:        addr["addressLocality"].as_str().map(str::to_string),
        region:      addr["addressRegion"].as_str().map(str::to_string),
        postal_code: addr["postalCode"].as_str().map(str::to_string),
        country:     addr["addressCountry"].as_str().map(str::to_string),
        bedrooms:    entity["numberOfBedrooms"].as_i64(),
        bathrooms:   entity["numberOfBathroomsTotal"].as_i64(),
        sqft:        entity["floorSize"]["value"].as_i64(),
        year_built:  entity["yearBuilt"].as_i64(),
        lat: entity["geo"]["latitude"].as_f64(),
        lon: entity["geo"]["longitude"].as_f64(),
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
    })
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Parses a realtor.ca listing page.
/// Returns `None` if the URL is not realtor.ca or no recognisable data is found.
pub fn parse(url: &str, html: &str) -> Option<ParsedListing> {
    if !url.contains("realtor.ca") {
        return None;
    }

    let document = Html::parse_document(html);
    let title = extract_title(&document);
    let description = extract_description(&document);

    // ── Try __NEXT_DATA__ first (richer data) ─────────────────────────────────
    if let Some(next_data) = extract_next_data(&document) {
        if let Some(listing) = find_listing_object(&next_data) {
            if let Some(property) = extract_from_next_listing(url, &title, &description, listing) {
                let image_urls = extract_next_image_urls(listing);
                return Some(ParsedListing { property, image_urls });
            }
        }
    }

    // ── Fall back to JSON-LD ──────────────────────────────────────────────────
    let json_ld = extract_json_ld(&document);
    if let Some(property) = extract_from_json_ld(url, &title, &json_ld) {
        let image_urls = extract_images(&document);
        return Some(ParsedListing { property, image_urls });
    }

    None
}
