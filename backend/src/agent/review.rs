//! Background agent review — triages a newly added listing using the Claude API.
//!
//! After a listing is saved via `POST /api/listings/suggest`, this task is
//! spawned to:
//! 1. Fetch all search profiles from the DB.
//! 2. Build a compact prompt from the parsed property fields.
//! 3. Call the Claude API (`claude-haiku-4-5`) to decide:
//!    - `HumanPending` + `search_profile_id` — listing matches a profile.
//!    - `AgentSkip` — no profile fits this listing.
//! 4. Call `POST /api/listings/:id/agent-review` on localhost to apply the decision.
//!
//! On any error the listing stays as `AgentPending` — no crash, silent degradation.

use reqwest::Client;
use sqlx::SqlitePool;

use crate::models::property::StoredProperty;

/// Entry point called from `suggest_listing` via `tokio::spawn`.
pub async fn run_agent_review(
    listing_id: i64,
    _property: StoredProperty,
    _pool: SqlitePool,
    _client: Client,
) {
    // TODO (Step 6): implement Claude API triage.
    tracing::info!(
        "agent_review: listing_id={} queued for review (not yet implemented)",
        listing_id
    );
}
