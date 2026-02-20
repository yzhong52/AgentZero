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

/// Parses a listing page by trying each registered parser in turn.
/// Returns `None` if no parser recognises the page structure.
pub fn parse(url: &str, html: &str) -> Option<ParsedListing> {
    realtor::parse(url, html)
        .or_else(|| redfin::parse(url, html))
        .or_else(|| rew::parse(url, html))
}
