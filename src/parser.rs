use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PropertyData {
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub address: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub price: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub beds: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub baths: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sqft: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, String>,
}

fn text_from_element(doc: &Html, selector: &Selector) -> String {
    doc.select(selector)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default()
}

fn first_group(text: &str, pattern: &str) -> String {
    Regex::new(pattern)
        .ok()
        .and_then(|re| re.captures(text))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default()
}

fn extract_json_ld(doc: &Html) -> Vec<serde_json::Value> {
    let script_selector = Selector::parse("script[type='application/ld+json']").unwrap();
    let mut result = Vec::new();

    for script in doc.select(&script_selector) {
        if let Some(text) = script.text().next() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                match json {
                    serde_json::Value::Array(arr) => result.extend(arr),
                    val => result.push(val),
                }
            }
        }
    }
    result
}

pub fn parse_properties_from_html(html: &str, url: &str) -> PropertyData {
    let doc = Html::parse_document(html);
    let mut out = PropertyData {
        url: url.to_string(),
        address: String::new(),
        price: String::new(),
        beds: String::new(),
        baths: String::new(),
        sqft: String::new(),
        description: String::new(),
        title: String::new(),
        images: Vec::new(),
        meta: HashMap::new(),
    };

    // Extract JSON-LD
    for ld in extract_json_ld(&doc) {
        if let Some(type_val) = ld.get("@type") {
            let type_str = match type_val {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(arr) => {
                    arr.first().and_then(|v| v.as_str()).unwrap_or("").to_string()
                }
                _ => String::new(),
            };

            if type_str.contains("Place") || type_str.contains("RealEstate") || type_str.contains("Product") {
                // Address
                if let Some(addr) = ld.get("address") {
                    if let Some(street) = addr.get("streetAddress").and_then(|v| v.as_str()) {
                        if out.address.is_empty() {
                            out.address = street.to_string();
                        }
                    }
                    if let Some(region) = addr.get("addressRegion").and_then(|v| v.as_str()) {
                        out.meta.insert("address_region".to_string(), region.to_string());
                    }
                    if let Some(locality) = addr.get("addressLocality").and_then(|v| v.as_str()) {
                        out.meta.insert("address_locality".to_string(), locality.to_string());
                    }
                }

                // Price from offers
                if let Some(offers) = ld.get("offers") {
                    if let Some(price_val) = offers.get("price") {
                        if out.price.is_empty() {
                            out.price = price_val.to_string().trim_matches('"').to_string();
                        }
                    }
                }

                // Description
                if let Some(desc) = ld.get("description").and_then(|v| v.as_str()) {
                    if out.description.is_empty() {
                        out.description = desc.to_string();
                    }
                }

                // Title/Name
                if let Some(name) = ld.get("name").and_then(|v| v.as_str()) {
                    if out.title.is_empty() {
                        out.title = name.to_string();
                    }
                }
            }
        }
    }

    // Meta tags (og:, twitter:, description)
    let meta_selector = Selector::parse("meta").unwrap();
    for meta in doc.select(&meta_selector) {
        let prop = meta
            .value()
            .attr("property")
            .or_else(|| meta.value().attr("name"))
            .unwrap_or("");
        let content = meta.value().attr("content").unwrap_or("");

        if prop.to_lowercase().contains("title") && out.title.is_empty() {
            out.title = content.to_string();
        } else if prop.to_lowercase().contains("description") && out.description.is_empty() {
            out.description = content.to_string();
        } else if prop.to_lowercase().contains("image") && !content.is_empty() && !out.images.contains(&content.to_string()) {
            out.images.push(content.to_string());
        }
    }

    // Title from page
    if out.title.is_empty() {
        let title_selector = Selector::parse("title").unwrap();
        out.title = text_from_element(&doc, &title_selector);
    }

    // Body text for pattern matching
    let body_selector = Selector::parse("body").unwrap();
    let body_text = text_from_element(&doc, &body_selector);

    // Extract price
    if out.price.is_empty() {
        out.price = first_group(&body_text, r"(\$[\d,]+(?:\.\d{2})?)");
    }

    // Extract beds
    if out.beds.is_empty() {
        out.beds = first_group(&body_text, r"(\d+)\s*(?:bed|bd|br)\b");
    }

    // Extract baths
    if out.baths.is_empty() {
        out.baths = first_group(&body_text, r"(\d+(?:\.\d+)?)\s*(?:bath|ba)\b");
    }

    // Extract sqft
    if out.sqft.is_empty() {
        let sqft_re = Regex::new(r"([\d,]+)\s*sq\.?\s*ft\.?|([\d,]+)\s*sf\b").unwrap();
        if let Some(caps) = sqft_re.captures(&body_text) {
            out.sqft = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
        }
    }
    if out.sqft.is_empty() {
        out.sqft = first_group(&body_text, r"(\d[\d,]*)\s*sqft");
    }

    // Extract images
    let img_selector = Selector::parse("img[src]").unwrap();
    for img in doc.select(&img_selector).take(20) {
        if let Some(src) = img.value().attr("src") {
            let src_lower = src.to_lowercase();
            if src_lower.contains("photo")
                || src_lower.contains("image")
                || src_lower.contains("listing")
                || src_lower.contains("property")
            {
                if !out.images.contains(&src.to_string()) {
                    out.images.push(src.to_string());
                }
            }
        }
    }

    if out.images.is_empty() {
        for img in doc.select(&img_selector).take(5) {
            if let Some(src) = img.value().attr("src") {
                let src_lower = src.to_lowercase();
                if !src_lower.contains("logo")
                    && !src_lower.contains("icon")
                    && !src_lower.contains("pixel")
                    && !src_lower.contains("track")
                {
                    out.images.push(src.to_string());
                }
            }
        }
    }

    out
}
