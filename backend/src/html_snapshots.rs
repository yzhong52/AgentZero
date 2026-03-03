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

/// Create the snapshots directory if it doesn't exist.
pub async fn ensure_dir() {
    if let Err(e) = fs::create_dir_all(DIR).await {
        tracing::warn!("html_snapshots: could not create directory '{}': {}", DIR, e);
    }
}

/// Write `html` to `{DIR}/{listing_id}_{source}.html`.
/// Silently skips empty HTML (e.g. blocked-host stubs).
pub(crate) async fn save_listing_html(listing_id: i64, site: crate::parsers::ListingSite, html: &str) {
    if html.is_empty() {
        return;
    }
    let label = site.name();
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
