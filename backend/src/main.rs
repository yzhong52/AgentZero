mod db;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use reqwest::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

#[derive(Clone)]
struct AppState {
    db: sqlx::SqlitePool,
    client: Client,
}

#[derive(Serialize)]
struct ParseResult {
    url: String,
    title: String,
    description: String,
    images: Vec<String>,
    raw_json_ld: Vec<JsonValue>,
    meta: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SaveRequest {
    url: String,
}

fn safe_url(input: &str) -> Option<Url> {
    if let Ok(u) = Url::parse(input) {
        match u.scheme() {
            "http" | "https" => Some(u),
            _ => None,
        }
    } else {
        None
    }
}

async fn fetch_html(client: &Client, url: &Url) -> Result<String, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br"));
    if let Ok(rv) = HeaderValue::from_str(url.as_str()) {
        headers.insert(REFERER, rv);
    }

    let resp = client.get(url.as_str()).headers(headers).send().await?;
    resp.error_for_status_ref()?;
    resp.text().await
}

fn extract_json_ld(document: &Html) -> Vec<JsonValue> {
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

fn meta_map(document: &Html) -> HashMap<String, String> {
    let mut m = HashMap::new();
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

fn extract_title(document: &Html) -> String {
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

fn extract_description(document: &Html) -> String {
    let sel =
        Selector::parse("meta[property=\"og:description\"], meta[name=\"description\"]").unwrap();
    if let Some(el) = document.select(&sel).next() {
        if let Some(content) = el.value().attr("content") {
            return content.to_string();
        }
    }
    String::new()
}

fn extract_images(document: &Html) -> Vec<String> {
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
fn extract_property(url: &str, title: &str, json_ld: &[JsonValue]) -> db::Property {
    let listing = json_ld.iter().find(|v| {
        let t = &v["@type"];
        t == "RealEstateListing"
            || t.as_array()
                .map(|a| a.iter().any(|x| x == "RealEstateListing"))
                .unwrap_or(false)
    });

    let mut p = db::Property {
        id: 0,
        url: url.to_string(),
        title: title.to_string(),
        description: String::new(),
        price: None,
        price_currency: None,
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
    };

    let Some(listing) = listing else {
        return p;
    };

    // description
    if let Some(d) = listing["description"].as_str() {
        p.description = d.to_string();
    }

    // offers
    p.price = listing["offers"]["price"].as_i64();
    p.price_currency = listing["offers"]["priceCurrency"].as_str().map(str::to_string);

    let entity = &listing["mainEntity"];

    // address
    let addr = &entity["address"];
    p.street_address = addr["streetAddress"].as_str().map(str::to_string);
    p.city = addr["addressLocality"].as_str().map(str::to_string);
    p.region = addr["addressRegion"].as_str().map(str::to_string);
    p.postal_code = addr["postalCode"].as_str().map(str::to_string);
    p.country = addr["addressCountry"].as_str().map(str::to_string);

    // rooms
    p.bedrooms = entity["numberOfBedrooms"].as_i64();
    p.bathrooms = entity["numberOfBathroomsTotal"].as_i64();
    p.sqft = entity["floorSize"]["value"].as_i64();
    p.year_built = entity["yearBuilt"].as_i64();

    // geo
    p.lat = entity["geo"]["latitude"].as_f64();
    p.lon = entity["geo"]["longitude"].as_f64();

    // images from mainEntity.image array
    if let Some(imgs) = entity["image"].as_array() {
        p.images = imgs
            .iter()
            .filter_map(|img| img["url"].as_str().map(str::to_string))
            .collect();
    }

    p
}

async fn parse(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ParseResult>, (StatusCode, String)> {
    let url = params
        .get("url")
        .ok_or((StatusCode::BAD_REQUEST, "Missing 'url' query parameter".to_string()))?;
    let url = url.trim();
    let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    let document = Html::parse_document(&html);
    let json_ld = extract_json_ld(&document);
    let meta = meta_map(&document);
    let title = extract_title(&document);
    let description = extract_description(&document);
    let images = extract_images(&document);

    Ok(Json(ParseResult {
        url: parsed.to_string(),
        title,
        description,
        images,
        raw_json_ld: json_ld,
        meta,
    }))
}

async fn save_listing(
    State(state): State<AppState>,
    Json(body): Json<SaveRequest>,
) -> Result<Json<db::Property>, (StatusCode, String)> {
    let url = body.url.trim();
    let parsed = safe_url(url).ok_or((StatusCode::BAD_REQUEST, "Invalid URL".to_string()))?;

    let html = fetch_html(&state.client, &parsed)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

    // `Html` is !Send, so extract everything in a block and drop it before the next await.
    let (json_ld, title) = {
        let document = Html::parse_document(&html);
        (extract_json_ld(&document), extract_title(&document))
    };

    let property = extract_property(parsed.as_str(), &title, &json_ld);

    let saved = db::save(&state.db, &property)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(saved))
}

async fn list_listings(
    State(state): State<AppState>,
) -> Result<Json<Vec<db::Property>>, (StatusCode, String)> {
    let listings = db::list(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)))?;

    Ok(Json(listings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn snapshot_redfin_3662_oak_st() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);

        let result = ParseResult {
            url: "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332"
                .to_string(),
            title: extract_title(&document),
            description: extract_description(&document),
            images: extract_images(&document),
            raw_json_ld: extract_json_ld(&document),
            meta: meta_map(&document),
        };

        insta::assert_json_snapshot!(result);
    }

    #[test]
    fn snapshot_extract_property() {
        let html = std::fs::read_to_string("fixtures/redfin_3662_oak_st.html")
            .expect("fixture not found — run from backend/");
        let document = Html::parse_document(&html);
        let json_ld = extract_json_ld(&document);
        let title = extract_title(&document);
        let url = "https://www.redfin.ca/bc/vancouver/3662-Oak-St-V6H-2M2/home/155902332";

        let property = extract_property(url, &title, &json_ld);

        insta::assert_json_snapshot!(property);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://listings.db".to_string());

    let db = db::init(&database_url).await;

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let state = AppState { db, client };

    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<axum::http::HeaderValue>()
                .unwrap(),
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/parse", get(parse))
        .route("/api/listings", post(save_listing).get(list_listings))
        .with_state(state)
        .layer(cors);

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
