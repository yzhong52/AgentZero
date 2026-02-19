use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::db;

static GARAGE_RE: OnceLock<Regex> = OnceLock::new();

fn garage_re() -> &'static Regex {
    GARAGE_RE.get_or_init(|| Regex::new(r"(?i)(\d+)\s+garage").unwrap())
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
    let land_sqft = entity["lotSize"]["value"].as_i64();

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
        land_sqft,
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
