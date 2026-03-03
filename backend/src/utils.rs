//! Shared helper utilities.

use url::Url;

/// Parse and validate `input` as an HTTP/HTTPS URL.
/// Returns `None` for malformed strings or non-HTTP schemes.
pub fn safe_url(input: &str) -> Option<Url> {
    if let Ok(u) = Url::parse(input) {
        match u.scheme() {
            "http" | "https" => Some(u),
            _ => None,
        }
    } else {
        None
    }
}
