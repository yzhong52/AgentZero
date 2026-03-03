//! HTML fetching: direct HTTP client with Safari/AppleScript fallback for bot-protected hosts.

use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, REFERER, USER_AGENT,
};
use reqwest::Client;
use url::Url;

/// Fetch HTML using the reqwest HTTP client.
async fn fetch_html_direct(client: &Client, url: &Url) -> Result<String, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br"),
    );
    if let Ok(rv) = HeaderValue::from_str(url.as_str()) {
        headers.insert(REFERER, rv);
    }

    let resp = client.get(url.as_str()).headers(headers).send().await?;
    resp.error_for_status_ref()?;
    resp.text().await
}

/// Fetch HTML by opening the URL in Safari via AppleScript.
///
/// Safari passes bot-protection checks (Incapsula, PerimeterX) that block
/// plain HTTP clients because it runs the full JS challenge in a real browser
/// context.  The page is opened in a new tab, allowed to settle for ~20 s,
/// and the rendered DOM source is returned.
async fn fetch_html_safari(url: &Url) -> Result<String, String> {
    let script = format!(
        r#"
tell application "Safari"
    activate
    make new document with properties {{URL:"{url}"}}
    delay 20
    set pageSource to source of document 1
    close document 1
    return pageSource
end tell
"#,
        url = url.as_str()
    );
    let output = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .await
        .map_err(|e| format!("osascript failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript error: {stderr}"));
    }
    let html = String::from_utf8_lossy(&output.stdout).to_string();
    if html.len() < 2000 {
        return Err(format!("Safari returned only {} bytes (likely blocked)", html.len()));
    }
    Ok(html)
}

/// Returns `true` when the HTML looks like a bot-protection challenge page
/// rather than real listing content.
fn is_challenge_page(html: &str) -> bool {
    html.len() < 2000
        && (html.contains("Incapsula")
            || html.contains("_Incapsula_Resource")
            || html.contains("px-blocked")
            || html.contains("PerimeterX"))
}

/// Returns `true` for hosts known to use aggressive bot protection that
/// blocks plain HTTP clients (even with TLS impersonation).
fn is_bot_protected_host(url: &Url) -> bool {
    match url.host_str().unwrap_or("") {
        h if h.contains("zillow.com") => true,
        h if h.contains("realtor.ca") => true,
        _ => false,
    }
}

/// Fetch HTML for a listing URL.
///
/// Strategy:
/// 1. Try the reqwest HTTP client.
/// 2. If that fails with a 403 or returns a bot-challenge page for a known
///    protected host, fall back to Safari via AppleScript.
pub(crate) async fn fetch_html(client: &Client, url: &Url) -> Result<String, String> {
    // Fast path: direct HTTP fetch.
    match fetch_html_direct(client, url).await {
        Ok(html) if !is_challenge_page(&html) => return Ok(html),
        Ok(html) if !is_bot_protected_host(url) => return Ok(html),
        Ok(_challenge) => {
            tracing::info!(
                "fetch_html: direct fetch returned challenge page for {}, trying Safari",
                url
            );
        }
        Err(e) if is_bot_protected_host(url) => {
            tracing::info!(
                "fetch_html: direct fetch failed for {} ({}), trying Safari",
                url,
                e
            );
        }
        Err(e) => return Err(format!("Failed to fetch {url}: {e}")),
    }

    // Slow path: Safari via AppleScript (macOS only).
    fetch_html_safari(url).await
}
