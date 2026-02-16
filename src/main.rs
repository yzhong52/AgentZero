use axum::{
    extract::Query,
    http::StatusCode,
    response::{Html, Json},
    routing::get,
    Router,
};
use serde::Deserialize;
use tower_http::services::ServeDir;
use url::Url;

mod parser;

use parser::PropertyData;

#[derive(Deserialize)]
struct ParseQuery {
    url: String,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    detail: String,
}

fn is_safe_url(url_str: &str) -> bool {
    match Url::parse(url_str) {
        Ok(url) => matches!(url.scheme(), "http" | "https") && url.has_host(),
        Err(_) => false,
    }
}

async fn index() -> Html<String> {
    Html(std::fs::read_to_string("static/index.html").unwrap_or_else(|_| {
        r#"<!DOCTYPE html>
<html>
<head><title>Property Parser</title></head>
<body><h1>Property Parser</h1><p>Frontend not found. Run build first.</p></body>
</html>"#
        .to_string()
    }))
}

async fn parse_url(Query(params): Query<ParseQuery>) -> Result<Json<PropertyData>, (StatusCode, Json<ErrorResponse>)> {
    let url_str = params.url.trim();
    if url_str.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                detail: "Missing 'url' query parameter".to_string(),
            }),
        ));
    }

    if !is_safe_url(url_str) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                detail: "Invalid or disallowed URL".to_string(),
            }),
        ));
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    detail: format!("Failed to create HTTP client: {}", e),
                }),
            )
        })?;

    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};

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

    let resp = client
        .get(url_str)
        .headers(headers)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    detail: format!("Failed to fetch URL: {}", e),
                }),
            )
        })?;

    let status = resp.status();
    if status.as_u16() == 429 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                detail: "This site is rate-limiting requests. Try again in a few minutes, or paste the page HTML if you have it.".to_string(),
            }),
        ));
    }

    if !status.is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                detail: format!("HTTP error {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")),
            }),
        ));
    }

    let html = resp.text().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                detail: format!("Failed to read response: {}", e),
            }),
        )
    })?;

    let parsed = parser::parse_properties_from_html(&html, url_str);
    Ok(Json(parsed))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/parse", get(parse_url))
        .nest_service("/static", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8001").await.unwrap();
    println!("Server running on http://127.0.0.1:8001");
    axum::serve(listener, app).await.unwrap();
}
