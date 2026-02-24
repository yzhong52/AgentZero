//! Redfin-specific listing parser.
//!
//! Redfin embeds structured data in two places:
//!   - JSON-LD (`RealEstateListing`) for core fields and image URLs.
//!   - An escaped JSON blob in a `<script>` tag for lot size and nearby schools.

use regex::Regex;
use scraper::Html;
use serde_json::Value as JsonValue;
use std::sync::OnceLock;

use crate::db;
use super::{ParsedListing, extract_json_ld, extract_title};

// ── Static regexes ────────────────────────────────────────────────────────────

static GARAGE_RE: OnceLock<Regex> = OnceLock::new();
static LOT_SIZE_RE: OnceLock<Regex> = OnceLock::new();
static NEARBY_SCHOOLS_RE: OnceLock<Regex> = OnceLock::new();
static TAX_ANNUAL_RE: OnceLock<Regex> = OnceLock::new();
static HOA_FEE_RE: OnceLock<Regex> = OnceLock::new();
static PARKING_COUNT_RE: OnceLock<Regex> = OnceLock::new();

fn garage_re() -> &'static Regex {
    GARAGE_RE.get_or_init(|| Regex::new(r"(?i)(\d+)\s+garage").unwrap())
}

fn parking_count_re() -> &'static Regex {
    PARKING_COUNT_RE.get_or_init(|| Regex::new(r"(\d+)").unwrap())
}

fn lot_size_re() -> &'static Regex {
    LOT_SIZE_RE.get_or_init(|| Regex::new(r#"lotSize\\?\":(\d+)"#).unwrap())
}

fn nearby_schools_re() -> &'static Regex {
    NEARBY_SCHOOLS_RE.get_or_init(|| {
        Regex::new(r#""nearbySchools":\s*(\[[^\]]*\])"#).unwrap()
    })
}

fn tax_annual_re() -> &'static Regex {
    TAX_ANNUAL_RE.get_or_init(|| {
        Regex::new(r"Tax Annual Amount:\s*\$?([\d,]+)").unwrap()
    })
}

/// Matches HOA / strata / maintenance fee text as it appears in Redfin's
/// property-details section.  Handles all of:
///   "HOA Dues: $543/month"
///   "HOA Fee: $200 per month"
///   "Maintenance Fee: $650"
fn hoa_fee_re() -> &'static Regex {
    HOA_FEE_RE.get_or_init(|| {
        Regex::new(r"(?i)(?:hoa\s*dues|hoa\s*fee|maintenance\s*fee)[s]?\s*:?\s*\$?([\d,]+)").unwrap()
    })
}

// ── School extraction ─────────────────────────────────────────────────────────

pub struct SchoolInfo {
    pub elementary: Option<(String, Option<f64>)>,
    pub middle: Option<(String, Option<f64>)>,
    pub secondary: Option<(String, Option<f64>)>,
}

/// Extracts nearby school names and GreatSchools ratings from Redfin's embedded JSON.
/// Redfin categorises schools as "e" (elementary), "m" (middle), "h" (high/secondary).
/// Returns `None` if no school data is found in the page.
pub fn extract_schools(html: &str) -> Option<SchoolInfo> {
    let caps = nearby_schools_re().captures(html)?;
    let json_str = caps.get(1)?.as_str();
    let schools: JsonValue = serde_json::from_str(json_str).ok()?;
    let arr = schools.as_array()?;

    let mut elementary: Option<(String, Option<f64>)> = None;
    let mut middle: Option<(String, Option<f64>)> = None;
    let mut secondary: Option<(String, Option<f64>)> = None;

    for s in arr {
        let name = match s["name"].as_str().or_else(|| s["schoolName"].as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let rating = s["rating"].as_f64()
            .or_else(|| s["greatSchoolsRating"].as_f64())
            .or_else(|| s["score"].as_f64());
        let level = s["levelCode"].as_str()
            .or_else(|| s["type"].as_str())
            .or_else(|| s["gradeRange"].as_str())
            .unwrap_or("");

        let lower = level.to_lowercase();
        if (lower.contains('e') || lower.starts_with('k') || lower.contains("elementary") || lower.contains("primary")) && elementary.is_none() {
            elementary = Some((name, rating));
        } else if (lower.contains('m') || lower.contains("middle") || lower.contains("junior")) && middle.is_none() {
            middle = Some((name, rating));
        } else if (lower.contains('h') || lower.contains("high") || lower.contains("secondary")) && secondary.is_none() {
            secondary = Some((name, rating));
        }
    }

    if elementary.is_none() && middle.is_none() && secondary.is_none() {
        return None;
    }

    Some(SchoolInfo { elementary, middle, secondary })
}

// ── Property tax ──────────────────────────────────────────────────────────────

/// Extracts annual property tax from Redfin's property details section.
/// Matches "Tax Annual Amount: $9,082.04" in the rendered HTML.
pub fn extract_property_tax(html: &str) -> Option<i64> {
    let caps = tax_annual_re().captures(html)?;
    let digits: String = caps.get(1)?.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

// ── HOA / strata fee ──────────────────────────────────────────────────────────

/// Extracts monthly HOA / strata / maintenance fee from Redfin's property
/// details section.
///
/// Redfin surfaces the fee as plain text in two ways:
///
/// 1. In the property-details text block, e.g.:
///    `"HOA Dues: $543/month"` or `"Maintenance Fee: $650"`
///
/// 2. In the escaped JSON blob embedded in a `<script>` tag, e.g.:
///    `"hoaFee":543` or `"maintenanceFee":543`
///
/// Both are tried; the text-block match takes precedence.
pub fn extract_hoa_monthly(html: &str) -> Option<i64> {
    // 1. Text-block match ("HOA Dues: $543/month", "Maintenance Fee: $650", …)
    if let Some(caps) = hoa_fee_re().captures(html) {
        let digits: String = caps.get(1)?.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(v) = digits.parse::<i64>() {
            if v > 0 { return Some(v); }
        }
    }
    // 2. Embedded JSON blob — Redfin uses several key names.
    //    The blob is an escaped JSON string, so quotes appear as \" and
    //    the pattern looks like: \"monthlyHoaDues\":1137
    //    Regex: optional-\ + " + key + optional-\ + " + :digits
    let json_re = Regex::new(r#"\\?"(?:hoaFee|maintenanceFee|monthlyHoaDues)\\?":\s*(\d+)"#).unwrap();
    if let Some(caps) = json_re.captures(html) {
        if let Ok(v) = caps[1].parse::<i64>() {
            if v > 0 { return Some(v); }
        }
    }
    None
}

// ── Lot size ──────────────────────────────────────────────────────────────────

/// Extracts lot size (sqft) from the raw HTML source.
/// Redfin embeds `"lotSize":3480` as escaped JSON in a script block.
pub fn extract_lot_size(html: &str) -> Option<i64> {
    lot_size_re()
        .captures(html)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

// ── Amenity features ──────────────────────────────────────────────────────────

/// Parses `amenityFeature` array entries for parking count, AC, radiant floor heating,
/// and in-unit laundry.
fn parse_amenity_features(features: &[JsonValue]) -> (Option<i64>, Option<bool>, Option<bool>, Option<bool>) {
    let mut parking_garage: Option<i64> = None;
    let mut ac: Option<bool> = None;
    let mut radiant: Option<bool> = None;
    let mut laundry_in_unit: Option<bool> = None;

    for f in features {
        let name = match f["name"].as_str() {
            Some(n) => n,
            None => continue,
        };
        let active = f["value"].as_bool().unwrap_or(false);
        let lower = name.to_lowercase();

        if lower.contains("parking") {
            // Handles both "Parking: 2 spaces" and "2 garage spaces" formats.
            let caps = garage_re().captures(name)
                .or_else(|| parking_count_re().captures(name));
            if let Some(n) = caps.and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<i64>().ok()) {
                parking_garage = Some(n);
            }
        } else if active && lower.contains("laundry") {
            laundry_in_unit = Some(true);
        } else if active && (lower.contains("air conditioning") || lower.contains(" a/c")) {
            ac = Some(true);
        } else if active && lower.contains("radiant") {
            radiant = Some(true);
        }
    }

    (parking_garage, ac, radiant, laundry_in_unit)
}

// ── JSON-LD extraction ────────────────────────────────────────────────────────

/// Extracts structured property fields from JSON-LD blocks.
/// Looks for the item whose `@type` includes `"RealEstateListing"`.
/// `images` is always left empty — `extract_image_urls` handles that.
pub fn extract_property(url: &str, title: &str, json_ld: &[JsonValue]) -> Option<db::Property> {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    })?;

    let entity = &listing["mainEntity"];
    let addr = &entity["address"];

    let description = listing["description"].as_str().unwrap_or("").to_string();
    let price = listing["offers"]["price"].as_i64();
    let price_currency = listing["offers"]["priceCurrency"].as_str().map(str::to_string);
    let street_address = addr["streetAddress"].as_str().map(str::to_string);
    let city = addr["addressLocality"].as_str().map(str::to_string);
    let region = addr["addressRegion"].as_str().map(str::to_string);
    let postal_code = addr["postalCode"].as_str().map(str::to_string);
    let country = addr["addressCountry"].as_str().map(str::to_string);
    let bedrooms = entity["numberOfBedrooms"].as_i64();
    let bathrooms = entity["numberOfBathroomsTotal"].as_i64();
    let sqft = entity["floorSize"]["value"].as_i64();
    let year_built = entity["yearBuilt"].as_i64();
    let lat = entity["geo"]["latitude"].as_f64();
    let lon = entity["geo"]["longitude"].as_f64();

    let amenities = entity["amenityFeature"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let (parking_garage, ac, radiant_floor_heating, laundry_in_unit) = parse_amenity_features(amenities);

    let property_type = entity["accommodationCategory"].as_str().map(str::to_string);
    let listed_date = listing["datePosted"].as_str().map(|s| s[..s.len().min(10)].to_string());

    Some(db::Property {
        id: 0,
        redfin_url: Some(url.to_string()),
        realtor_url: None,
        rew_url: None,
        zillow_url: None,
        title: title.to_string(),
        description,
        price,
        price_currency,
        offer_price: None,
        street_address,
        city,
        region,
        postal_code,
        country,
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
        land_sqft: None,
        property_tax: None,
        skytrain_station: None,
        skytrain_walk_min: None,
        radiant_floor_heating,
        ac,
        laundry_in_unit,
        // Mortgage params are set by main.rs after parsing (save/refresh handlers).
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
        school_elementary: None,
        school_elementary_rating: None,
        school_middle: None,
        school_middle_rating: None,
        school_secondary: None,
        school_secondary_rating: None,
        property_type,
        listed_date,
        mls_number: None,
    })
}

/// Extracts image source URLs from `mainEntity.image[]` in the JSON-LD block.
pub fn extract_image_urls(json_ld: &[JsonValue]) -> Vec<String> {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    });
    listing
        .and_then(|l| l["mainEntity"]["image"].as_array())
        .map(|imgs| {
            imgs.iter()
                .filter_map(|img| img["url"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Parses a Redfin listing page into a `ParsedListing`.
/// Returns `None` if the page does not contain a `RealEstateListing` JSON-LD block.
pub fn parse(url: &str, html: &str) -> Option<ParsedListing> {
    let document = Html::parse_document(html);
    let json_ld = extract_json_ld(&document);
    let title = extract_title(&document);

    let mut property = extract_property(url, &title, &json_ld)?;

    property.land_sqft = extract_lot_size(html);
    property.property_tax = extract_property_tax(html);
    property.hoa_monthly = extract_hoa_monthly(html);
    if let Some(schools) = extract_schools(html) {
        if let Some((name, rating)) = schools.elementary {
            property.school_elementary = Some(name);
            property.school_elementary_rating = rating;
        }
        if let Some((name, rating)) = schools.middle {
            property.school_middle = Some(name);
            property.school_middle_rating = rating;
        }
        if let Some((name, rating)) = schools.secondary {
            property.school_secondary = Some(name);
            property.school_secondary_rating = rating;
        }
    }

    let image_urls = extract_image_urls(&json_ld);
    Some(ParsedListing { property, image_urls })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::test_support::{fixture, listing_to_property};

    #[test]
    fn redfin_829_e14th() {
        let html = std::fs::read_to_string(fixture("redfin_829_e14th.html")).expect("fixture not found");
        let listing = parse(
            "https://www.redfin.ca/bc/vancouver/829-E-14th-Ave-V5T-2N5/home/155809679",
            &html,
        )
        .expect("parse failed");
        insta::assert_json_snapshot!("redfin_829_e14th", listing_to_property(listing));
    }

    #[test]
    fn redfin_788_w8th() {
        let html = std::fs::read_to_string(fixture("redfin_788_w8th.html")).expect("fixture not found");
        let listing = parse(
            "https://www.redfin.ca/bc/vancouver/788-W-8th-Ave-V5Z-1E1/home/",
            &html,
        )
        .expect("parse failed");
        let property = listing_to_property(listing);
        assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
        insta::assert_json_snapshot!("redfin_788_w8th", property);
    }
}
