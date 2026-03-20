//! Handler for the refresh diff preview endpoint.
//!
//! - GET /api/listings/:id/preview — fetch and parse without saving; returns the merged
//!   result so the caller can diff it against the stored record.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::models::open_house::OpenHouse;
use crate::models::property::Property;
use crate::store::{open_house_store, property_store};
use crate::{parsers, AppState};

use super::refresh::{fetch_sources, resolve_parsed_listing, stored_source_urls, ResolvedListing};

/// GET /api/listings/:id/preview
///
/// Fetches and parses a listing without saving — used to build the refresh diff preview.
/// Applies the same field-preservation rules as `refresh_listing` so the diff accurately
/// reflects what would change if the user confirmed the refresh.
pub(crate) async fn preview_refresh(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Property>, (StatusCode, String)> {
    // ── 1. Load the stored record ──────────────────────────────────────────────
    let stored = property_store::get_by_id(&state.db, id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Listing not found: {}", e)))?;

    // ── 2. Fetch HTML for each stored source URL ───────────────────────────────
    let stored_urls = stored_source_urls(&stored);
    let sources = fetch_sources(&state, &stored_urls, "preview_refresh", false).await?;

    // ── 3. Parse and resolve ───────────────────────────────────────────────────
    let listing = parsers::parse_multi(&sources).ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "No recognized listing format found in page".to_string(),
    ))?;

    match resolve_parsed_listing(listing, &stored) {
        // ── 3a. Off-market / data-stripped page ───────────────────────────────
        // Mirror the same guard used in refresh_listing: only show a source_status
        // change in the diff so the user isn't presented with spurious field resets.
        ResolvedListing::StatusOnly(new_status) => {
            let open_houses = open_house_store::list_open_houses(&state.db, stored.id)
                .await
                .unwrap_or_default();
            Ok(Json(Property {
                source_status: new_status,
                open_houses,
                ..stored
            }))
        }

        // ── 4. Full listing — merge using the same rules as refresh ───────────
        ResolvedListing::Full { property: preview, open_houses: parsed_oh, .. } => {
            // Populate open_houses with the freshly parsed events so the diff can
            // compare them against the stored ones. Use negative fake IDs (never in DB).
            let parsed_open_houses: Vec<OpenHouse> = parsed_oh
                .into_iter()
                .enumerate()
                .map(|(i, oh)| OpenHouse {
                    id: -(i as i64 + 1),
                    listing_id: stored.id,
                    start_time: oh.start_time,
                    end_time: oh.end_time,
                    visited: false,
                    created_at: String::new(),
                })
                .collect();

            Ok(Json(Property { open_houses: parsed_open_houses, ..preview }))
        }
    }
}
