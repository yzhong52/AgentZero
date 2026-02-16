use actix_cors::Cors;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder, middleware::Logger};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

#[derive(Serialize)]
struct ParseResult {
    url: String,
    title: String,
    description: String,
    images: Vec<String>,
    raw_json_ld: Vec<JsonValue>,
    meta: HashMap<String, String>,
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
    let resp = client
        .get(url.as_str())
        .header("User-Agent", "property-parser/1.0")
        .send()
        .await?;
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
                        for item in arr { out.push(item.clone()); }
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
        let name = el.value().attr("property").or_else(|| el.value().attr("name")).unwrap_or("");
        if name.is_empty() { continue; }
        if let Some(content) = el.value().attr("content") {
            m.insert(name.to_string(), content.to_string());
        }
    }
    m
}

fn extract_title(document: &Html) -> String {
    let og = Selector::parse("meta[property=\"og:title\"]").unwrap();
    if let Some(el) = document.select(&og).next() {
        if let Some(content) = el.value().attr("content") { return content.to_string(); }
    }
    let title = Selector::parse("title").unwrap();
    if let Some(el) = document.select(&title).next() {
        return el.text().collect::<Vec<_>>().join("").trim().to_string();
    }
    String::new()
}

fn extract_description(document: &Html) -> String {
    let sel = Selector::parse("meta[property=\"og:description\"], meta[name=\"description\"]").unwrap();
    if let Some(el) = document.select(&sel).next() {
        if let Some(content) = el.value().attr("content") { return content.to_string(); }
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

#[get("/api/parse")]
async fn parse(web::Query(params): web::Query<HashMap<String, String>>) -> impl Responder {
    let url = if let Some(u) = params.get("url") { u } else {
        return HttpResponse::BadRequest().body("Missing 'url' query parameter");
    };
    let url = url.trim();
    let parsed = match safe_url(url) {
        Some(u) => u,
        None => return HttpResponse::BadRequest().body("Invalid URL"),
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let html = match fetch_html(&client, &parsed).await {
        Ok(h) => h,
        Err(e) => return HttpResponse::BadGateway().body(format!("Failed to fetch URL: {}", e)),
    };

    let document = Html::parse_document(&html);
    let json_ld = extract_json_ld(&document);
    let meta = meta_map(&document);
    let title = extract_title(&document);
    let description = extract_description(&document);
    let images = extract_images(&document);

    let out = ParseResult {
        url: parsed.to_string(),
        title,
        description,
        images,
        raw_json_ld: json_ld,
        meta,
    };

    HttpResponse::Ok().json(out)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let bind = "127.0.0.1:8000";
    println!("Starting backend at http://{}", bind);

    HttpServer::new(|| {
        let cors = Cors::default()
            .allow_any_method()
            .allow_any_header()
            .allowed_origin("http://localhost:5173")
            .supports_credentials();

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .service(parse)
    })
    .bind(bind)?
    .run()
    .await
}
fn main() {
    println!("Hello, world!");
}
