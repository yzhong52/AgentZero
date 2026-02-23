/// Zillow listing parser — **not supported; always returns `None`**.
///
/// # Why Zillow cannot be scraped
///
/// Zillow uses **PerimeterX** bot protection (served via CloudFront).
/// Every direct HTTP request — regardless of headers — receives:
///
/// ```text
/// HTTP/2 403
/// x-px-blocked: 1
/// ```
///
/// This happens because PerimeterX detects bots through several signals that
/// cannot be faked by a plain HTTP client:
///
/// 1. **TLS fingerprint (JA3 hash)** — the TLS handshake produced by
///    reqwest/rustls has a distinct fingerprint.  PerimeterX identifies it as
///    non-browser before reading a single request header.
///
/// 2. **JavaScript challenge** — PerimeterX injects JS into a real browser
///    that runs a challenge and writes a `_pxhd` cookie.  A plain HTTP client
///    never executes JS and therefore never passes the challenge.
///
/// 3. **IP reputation** — data-centre and residential IPs without a clean
///    browsing history are auto-blocked regardless of headers or cookies.
///
/// # Workarounds (not implemented)
///
/// - **Zillow Bridge API** — official data feed; requires applying for access
///   at <https://www.zillow.com/howzillow-works/apis/>.
/// - **Headless browser** — Playwright/Puppeteer with a real Chrome instance
///   can pass the JS challenge, but is heavy, slow, and against Zillow ToS.
/// - **Residential proxy** — expensive and against Zillow ToS.
///
/// # Current behaviour
///
/// `parse()` returns `None`.  The `add_listing` handler in `api/add.rs`
/// detects that all URLs are Zillow, logs a warning, and saves a stub so
/// the user can fill in details manually via the edit panel.

use super::ParsedListing;

/// Always returns `None` — see module-level documentation for details.
pub fn parse(_url: &str, _html: &str) -> Option<ParsedListing> {
    None
}
