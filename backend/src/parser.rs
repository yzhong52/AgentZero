use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::db;

static NEARBY_SCHOOLS_RE: OnceLock<Regex> = OnceLock::new();

fn nearby_schools_re() -> &'static Regex {
    NEARBY_SCHOOLS_RE.get_or_init(|| {
        Regex::new(r#""nearbySchools":\s*(\[[^\]]*\])"#).unwrap()
    })
}

pub struct SchoolInfo {
    pub elementary: Option<(String, Option<f64>)>,
    pub middle: Option<(String, Option<f64>)>,
    pub secondary: Option<(String, Option<f64>)>,
}

/// Extracts nearby school names and GreatSchools ratings from Redfin's embedded JSON.
/// Redfin categorises schools as "E" (elementary), "M" (middle), "H" (high/secondary).
/// Returns None if no school data is found in the page.
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
        if (lower.contains('e') || lower.starts_with("k") || lower.contains("elementary") || lower.contains("primary")) && elementary.is_none() {
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

static GARAGE_RE: OnceLock<Regex> = OnceLock::new();
static LOT_SIZE_RE: OnceLock<Regex> = OnceLock::new();

fn garage_re() -> &'static Regex {
    GARAGE_RE.get_or_init(|| Regex::new(r"(?i)(\d+)\s+garage").unwrap())
}

/// Extracts lot size (sqft) from the raw HTML source.
/// Redfin embeds `"lotSize":3480` as escaped JSON in a script block — not in JSON-LD.
pub fn extract_lot_size(html: &str) -> Option<i64> {
    LOT_SIZE_RE
        .get_or_init(|| Regex::new(r#"lotSize\\?\":(\d+)"#).unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

/// Parses `amenityFeature` array entries for parking count, AC, and radiant floor heating.
/// Only sets a field to `Some(true)` for booleans when the feature has `value: true`.
fn parse_amenity_features(features: &[JsonValue]) -> (Option<i64>, Option<bool>, Option<bool>) {
    let mut parking_garage: Option<i64> = None;
    let mut ac: Option<bool> = None;
    let mut radiant: Option<bool> = None;

    for f in features {
        let name = match f["name"].as_str() {
            Some(n) => n,
            None => continue,
        };
        let active = f["value"].as_bool().unwrap_or(false);
        let lower = name.to_lowercase();

        if lower.contains("parking") {
            if let Some(caps) = garage_re().captures(name) {
                if let Some(n) = caps.get(1).and_then(|m| m.as_str().parse::<i64>().ok()) {
                    parking_garage = Some(n);
                }
            }
        } else if active && (lower.contains("air conditioning") || lower.contains(" a/c")) {
            ac = Some(true);
        } else if active && lower.contains("radiant") {
            radiant = Some(true);
        }
    }

    (parking_garage, ac, radiant)
}

#[derive(Serialize)]
pub struct ParseResult {
    pub url: String,
    pub title: String,
    pub description: String,
    pub images: Vec<String>,
    pub raw_json_ld: Vec<JsonValue>,
    pub meta: BTreeMap<String, String>,
}

pub fn extract_json_ld(document: &Html) -> Vec<JsonValue> {
    let selector = Selector::parse("script[type=\"application/ld+json\"]").unwrap();
    let mut out = Vec::new();
    for el in document.select(&selector) {
        if let Some(text) = el.first_child().and_then(|n| n.value().as_text()) {
            let s = text.trim();
            if s.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<JsonValue>(s) {
                if v.is_array() {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            out.push(item.clone());
                        }
                    }
                } else {
                    out.push(v);
                }
            }
        }
    }
    out
}

pub fn meta_map(document: &Html) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let selector = Selector::parse("meta").unwrap();
    for el in document.select(&selector) {
        let name = el
            .value()
            .attr("property")
            .or_else(|| el.value().attr("name"))
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if let Some(content) = el.value().attr("content") {
            m.insert(name.to_string(), content.to_string());
        }
    }
    m
}

pub fn extract_title(document: &Html) -> String {
    let og = Selector::parse("meta[property=\"og:title\"]").unwrap();
    if let Some(el) = document.select(&og).next() {
        if let Some(content) = el.value().attr("content") {
            return content.to_string();
        }
    }
    let title = Selector::parse("title").unwrap();
    if let Some(el) = document.select(&title).next() {
        return el.text().collect::<Vec<_>>().join("").trim().to_string();
    }
    String::new()
}

pub fn extract_description(document: &Html) -> String {
    let sel =
        Selector::parse("meta[property=\"og:description\"], meta[name=\"description\"]").unwrap();
    if let Some(el) = document.select(&sel).next() {
        if let Some(content) = el.value().attr("content") {
            return content.to_string();
        }
    }
    String::new()
}

pub fn extract_images(document: &Html) -> Vec<String> {
    let sel = Selector::parse("meta[property=\"og:image\"]").unwrap();
    let mut out = Vec::new();
    for el in document.select(&sel) {
        if let Some(content) = el.value().attr("content") {
            out.push(content.to_string());
        }
    }
    out
}

/// Extracts structured property fields from JSON-LD blocks.
/// Looks for the item whose @type includes "RealEstateListing".
/// Returns None if no matching block is found.
/// `images` is always left empty here — call `extract_image_urls` separately.
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
    let (parking_garage, ac, radiant_floor_heating) = parse_amenity_features(amenities);

    Some(db::Property {
        id: 0,
        url: url.to_string(),
        title: title.to_string(),
        description,
        price,
        price_currency,
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
        land_sqft: None, // set from raw HTML by caller via extract_lot_size()
        property_tax: None,
        skytrain_station: None,
        skytrain_walk_min: None,
        radiant_floor_heating,
        ac,
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

/// Extracts image source URLs from mainEntity.image[] in the RealEstateListing JSON-LD block.
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
