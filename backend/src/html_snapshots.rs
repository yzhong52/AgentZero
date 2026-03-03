//! Persist raw listing HTML to disk for offline inspection and parser backfills.
//!
//! Files are written to `html_snapshots/` (sibling of `listings_images/`) as:
//!   `{listing_id}_{source}.html`
//! e.g. `42_redfin.html`, `42_rew.html`
//!
//! On refresh the file is overwritten with the latest fetch so it always
//! reflects the most recent page seen by the backend.

use tokio::fs;

pub const DIR: &str = "html_snapshots";

/// Derive a short source label from the URL hostname.
fn source_label(url: &str) -> &'static str {
    if url.contains("redfin.") {
        "redfin"
    } else if url.contains("rew.ca") {
        "rew"
    } else if url.contains("realtor.ca") {
        "realtor"
    } else if url.contains("zillow.com") {
        "zillow"
    } else {
        "unknown"
    }
}

/// Create the snapshots directory if it doesn't exist.
pub async fn ensure_dir() {
    if let Err(e) = fs::create_dir_all(DIR).await {
        tracing::warn!("html_snapshots: could not create directory '{}': {}", DIR, e);
    }
}

/// Write `html` to `{DIR}/{listing_id}_{source}.html`.
/// Silently skips empty HTML (e.g. blocked-host stubs).
pub async fn save(listing_id: i64, url: &str, html: &str) {
    if html.is_empty() {
        return;
    }
    let label = source_label(url);
    let path = format!("{}/{listing_id}_{label}.html", DIR);
    match fs::write(&path, html.as_bytes()).await {
        Ok(_) => tracing::debug!(
            "html_snapshots: saved {} bytes → {}",
            html.len(),
            path
        ),
        Err(e) => tracing::warn!("html_snapshots: failed to write {}: {}", path, e),
    }
}
