/// Parsers for different listing sources.
///
/// Each parser lives in its own submodule and exposes:
///   - `parse(url: &str, html: &str) -> Option<ParsedListing>`
///
/// The top-level `parse()` function dispatches to the right parser based on the URL.

pub mod redfin;
pub mod realtor;
pub mod rew;

use crate::db;
use scraper::{Html, Selector};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

// ── Shared output types ───────────────────────────────────────────────────────

/// The normalised result of parsing a listing page: structured property data
/// and the ordered list of image source URLs.
pub struct ParsedListing {
    pub property: db::Property,
    pub image_urls: Vec<String>,
}

/// Raw debug output returned by `GET /api/parse`.
#[derive(Serialize)]
pub struct ParseResult {
    pub url: String,
    pub title: String,
    pub description: String,
    pub images: Vec<String>,
    pub raw_json_ld: Vec<JsonValue>,
    pub meta: BTreeMap<String, String>,
}

// ── Generic HTML utilities (shared across parsers) ────────────────────────────

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

// ── Parser dispatch ───────────────────────────────────────────────────────────

/// Merges two `Option<T>` values where redfin is primary.
/// - If both are `Some` and differ, logs a warning and keeps redfin's value.
/// - If only one is `Some`, uses that.
macro_rules! merge_field {
    ($field:expr, $redfin:expr, $rew:expr) => {
        match (&$redfin, &$rew) {
            (Some(r), Some(w)) if r != w => {
                tracing::warn!(
                    "merge conflict on {}: redfin={:?} rew={:?} — keeping redfin",
                    $field, r, w
                );
                $redfin
            }
            (None, w) => w.clone(),
            (r, _) => r.clone(),
        }
    };
}

/// Parses and merges data from multiple listing pages for the same property.
///
/// Strategy:
/// - Redfin is the primary source for all fields.
/// - rew.ca fills in any field that redfin left empty.
/// - When both sources have a value and they differ, keeps redfin's and logs a warning.
///
/// `sources` is a slice of `(url, html)` pairs. Unknown URLs are ignored.
pub fn parse_multi(sources: &[(&str, &str)]) -> Option<ParsedListing> {
    let redfin = sources
        .iter()
        .find(|(url, _)| url.contains("redfin"))
        .and_then(|(url, html)| redfin::parse(url, html));

    let rew = sources
        .iter()
        .find(|(url, _)| url.contains("rew.ca"))
        .and_then(|(url, html)| rew::parse(url, html));

    // At least one parser must succeed.
    let (r, w) = match (redfin, rew) {
        (None, None) => return None,
        (Some(r), None) => return Some(r),
        (None, Some(w)) => return Some(w),
        (Some(r), Some(w)) => (r, w),
    };

    let rp = r.property;
    let wp = w.property;

    let merged = db::Property {
        // Identity / URLs: redfin is canonical; carry rew_url from rew result.
        id: rp.id,
        redfin_url: rp.redfin_url.clone(),
        realtor_url: rp.realtor_url.clone(),
        rew_url: wp.rew_url.clone(),

        // Scalar string fields — prefer non-empty redfin, fall back to rew.
        title:       if rp.title.is_empty() { wp.title.clone() } else { rp.title.clone() },
        description: if rp.description.is_empty() { wp.description.clone() } else { rp.description.clone() },

        price:          merge_field!("price",          rp.price,          wp.price),
        price_currency: merge_field!("price_currency", rp.price_currency, wp.price_currency),
        offer_price:    None,

        street_address: merge_field!("street_address", rp.street_address, wp.street_address),
        city:           merge_field!("city",           rp.city,           wp.city),
        region:         merge_field!("region",         rp.region,         wp.region),
        postal_code:    merge_field!("postal_code",    rp.postal_code,    wp.postal_code),
        country:        merge_field!("country",        rp.country,        wp.country),

        bedrooms:  merge_field!("bedrooms",  rp.bedrooms,  wp.bedrooms),
        bathrooms: merge_field!("bathrooms", rp.bathrooms, wp.bathrooms),
        sqft:      merge_field!("sqft",      rp.sqft,      wp.sqft),

        year_built: merge_field!("year_built", rp.year_built, wp.year_built),

        lat: merge_field!("lat", rp.lat, wp.lat),
        lon: merge_field!("lon", rp.lon, wp.lon),

        // Images: prefer redfin's (higher quality); fall back to rew's.
        images: rp.images.clone(),

        created_at: rp.created_at.clone(),
        updated_at: rp.updated_at.clone(),
        notes:      rp.notes.clone(),

        parking_garage:  merge_field!("parking_garage",  rp.parking_garage,  wp.parking_garage),
        parking_covered: merge_field!("parking_covered", rp.parking_covered, wp.parking_covered),
        parking_open:    merge_field!("parking_open",    rp.parking_open,    wp.parking_open),
        land_sqft:       merge_field!("land_sqft",       rp.land_sqft,       wp.land_sqft),

        property_tax: merge_field!("property_tax", rp.property_tax, wp.property_tax),

        skytrain_station:  rp.skytrain_station.clone(),
        skytrain_walk_min: rp.skytrain_walk_min,

        radiant_floor_heating: merge_field!("radiant_floor_heating", rp.radiant_floor_heating, wp.radiant_floor_heating),
        ac:                    merge_field!("ac",                    rp.ac,                    wp.ac),

        down_payment_pct:       rp.down_payment_pct,
        mortgage_interest_rate: rp.mortgage_interest_rate,
        amortization_years:     rp.amortization_years,
        mortgage_monthly:       rp.mortgage_monthly,

        hoa_monthly:   merge_field!("hoa_monthly",   rp.hoa_monthly,   wp.hoa_monthly),
        monthly_total: rp.monthly_total,
        monthly_cost: rp.monthly_cost,

        has_rental_suite: merge_field!("has_rental_suite", rp.has_rental_suite, wp.has_rental_suite),
        rental_income:    merge_field!("rental_income",    rp.rental_income,    wp.rental_income),

        status:   rp.status.clone(),
        nickname: rp.nickname.clone(),

        school_elementary:        rp.school_elementary.clone(),
        school_elementary_rating: rp.school_elementary_rating,
        school_middle:            rp.school_middle.clone(),
        school_middle_rating:     rp.school_middle_rating,
        school_secondary:         rp.school_secondary.clone(),
        school_secondary_rating:  rp.school_secondary_rating,
    };

    // Image URLs: prefer redfin's; supplement with rew's if redfin had none.
    let image_urls = if r.image_urls.is_empty() { w.image_urls } else { r.image_urls };

    tracing::info!("parse_multi: merged property_tax={:?}", merged.property_tax);
    Some(ParsedListing { property: merged, image_urls })
}
