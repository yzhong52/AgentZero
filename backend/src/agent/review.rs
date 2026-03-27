//! Background agent review — triages a newly added listing using the Claude API.
//!
//! After a listing is saved via `POST /api/listings/agent-suggest`, this task is
//! spawned to:
//! 1. Fetch all search profiles from the DB.
//! 2. Build a compact prompt from the parsed property fields.
//! 3. Call the Claude API (`claude-haiku-4-5`) to decide:
//!    - `HumanReview` + `search_profile_id` — listing matches a profile.
//!    - `AgentSkip` — no profile fits this listing.
//! 4. Call `POST /api/listings/:id/agent-review` on localhost to apply the decision.
//!
//! Failures are persisted on the listing so the client can distinguish a true
//! timeout from a terminal error such as invalid credentials or insufficient
//! upstream billing.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::env;
use std::sync::Arc;

use crate::models::property::StoredProperty;
use crate::store::{property_store, search_profile_store};

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const CLAUDE_MODEL: &str = "claude-haiku-4-5-20251001";

/// Entry point called from `suggest_listing` via `tokio::spawn`.
pub async fn run_agent_review(
    listing_id: i64,
    property: StoredProperty,
    pool: SqlitePool,
    client: Client,
    anthropic_api_key: Arc<str>,
) {
    if let Err(e) = try_agent_review(listing_id, property, &pool, &client, &anthropic_api_key).await {
        if let Err(db_err) = property_store::mark_agent_review_failed(
            &pool,
            listing_id,
            e.code,
            &e.user_message,
        )
        .await
        {
            tracing::warn!(
                "agent_review: listing_id={} failed to persist failure state after {}: {}",
                listing_id,
                e,
                db_err,
            );
        }
        tracing::warn!(
            "agent_review: listing_id={} failed: {}",
            listing_id,
            e
        );
    }
}

async fn try_agent_review(
    listing_id: i64,
    property: StoredProperty,
    pool: &SqlitePool,
    client: &Client,
    anthropic_api_key: &str,
) -> Result<(), AgentReviewFailure> {
    tracing::info!(
        "agent_review: listing_id={} Anthropic key loaded ({})",
        listing_id,
        describe_api_key(anthropic_api_key)
    );

    // Fetch all search profiles.
    let profiles = search_profile_store::list_all(pool)
        .await
        .map_err(|e| AgentReviewFailure::internal(
            "search_profiles_unavailable",
            "Agent review could not load search profiles.",
            format!("DB error fetching profiles: {e}"),
        ))?;

    if profiles.is_empty() {
        return Err(AgentReviewFailure::internal(
            "no_search_profiles",
            "Agent review cannot run because no search profiles are defined.",
            "no search profiles defined — cannot triage".to_string(),
        ));
    }

    let prompt = build_prompt(&property, &profiles);
    tracing::debug!("agent_review: listing_id={} prompt built ({} chars)", listing_id, prompt.len());

    // Call Claude API.
    let request_body = json!({
        "model": CLAUDE_MODEL,
        "max_tokens": 300,
        "messages": [{ "role": "user", "content": prompt }]
    });

    let request = client
        .post(CLAUDE_API_URL)
        .header("x-api-key", anthropic_api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .build()
        .map_err(|e| AgentReviewFailure::internal(
            "request_build_failed",
            "Agent review could not build the Claude request.",
            format!("Failed to build Claude API request: {e}"),
        ))?;

    let header_value = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(mask_api_key)
        .unwrap_or_else(|| "<missing-or-non-utf8>".to_string());
    let anthropic_version = request
        .headers()
        .get("anthropic-version")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing-or-non-utf8>");
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing-or-non-utf8>");

    tracing::info!(
        "agent_review: listing_id={} sending Claude request url={} x_api_key_present={} x_api_key={} anthropic_version={} content_type={} prompt_chars={}",
        listing_id,
        CLAUDE_API_URL,
        request.headers().contains_key("x-api-key"),
        header_value,
        anthropic_version,
        content_type,
        prompt.len(),
    );

    let response = client
        .execute(request)
        .await
        .map_err(|e| AgentReviewFailure::upstream_network(format!("Claude API request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(classify_claude_api_failure(status, &body));
    }

    let response_json: Value = response
        .json()
        .await
        .map_err(|e| AgentReviewFailure::internal(
            "response_parse_failed",
            "Agent review received an unreadable response from Claude.",
            format!("Failed to parse Claude API response: {e}"),
        ))?;

    let text = response_json["content"][0]["text"]
        .as_str()
        .ok_or_else(|| AgentReviewFailure::internal(
            "response_shape_invalid",
            "Agent review received an unexpected response shape from Claude.",
            format!("Unexpected Claude API response shape: {response_json}"),
        ))?;

    tracing::debug!("agent_review: listing_id={} raw response: {}", listing_id, text);

    let decision: AgentDecision = parse_decision(text).map_err(|e| {
        AgentReviewFailure::internal(
            "decision_parse_failed",
            "Agent review returned a decision we could not understand.",
            format!("Failed to parse agent decision from '{text}': {e}"),
        )
    })?;

    tracing::info!(
        "agent_review: listing_id={} decision={:?} profile={:?} comment={:?}",
        listing_id,
        decision.status,
        decision.search_profile_id,
        decision.comment,
    );

    // Apply the decision via the agent-review endpoint.
    let backend_port = env::var("BACKEND_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000);

    let endpoint = format!("http://127.0.0.1:{backend_port}/api/listings/{listing_id}/agent-review");

    let review_body = json!({
        "status": decision.status,
        "search_profile_id": decision.search_profile_id,
        "comment": decision.comment,
    });

    client
        .post(&endpoint)
        .json(&review_body)
        .send()
        .await
        .map_err(|e| AgentReviewFailure::internal(
            "local_apply_request_failed",
            "Agent review could not apply its decision locally.",
            format!("Failed to call agent-review endpoint: {e}"),
        ))?
        .error_for_status()
        .map_err(|e| AgentReviewFailure::internal(
            "local_apply_failed",
            "Agent review decision was produced but could not be saved.",
            format!("agent-review endpoint returned error: {e}"),
        ))?;

    Ok(())
}

/// Build the compact triage prompt sent to Claude.
fn build_prompt(p: &StoredProperty, profiles: &[crate::models::search_profile::SearchProfile]) -> String {
    let price = p.price
        .map(|v| format!("${}", format_number(v)))
        .unwrap_or_else(|| "—".to_string());
    let currency = p.price_currency.as_deref().unwrap_or("CAD");
    let address = match (&p.street_address, &p.city) {
        (Some(s), Some(c)) => format!("{s}, {c}"),
        (Some(s), None) => s.clone(),
        (None, Some(c)) => c.clone(),
        _ => "—".to_string(),
    };

    let mut lines = vec![
        "You are triaging a real estate listing against a set of search profiles.".to_string(),
        "Decide which profile best matches, or skip if none fit.".to_string(),
        String::new(),
        "Listing:".to_string(),
        format!("  Title:    {}", p.title),
        format!("  Price:    {price} {currency}"),
        format!("  Address:  {address}"),
        format!("  Beds/Baths: {} / {}",
            p.bedrooms.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
            p.bathrooms.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())),
        format!("  Sqft:     {}   Land: {}",
            p.sqft.map(|v| format_number(v)).unwrap_or_else(|| "—".to_string()),
            p.land_sqft.map(|v| format_number(v)).unwrap_or_else(|| "—".to_string())),
        format!("  Type:     {}   Built: {}",
            p.property_type.as_deref().unwrap_or("—"),
            p.year_built.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())),
        format!("  Tax/yr:   {}   HOA/mo: {}",
            p.property_tax.map(|v| format!("${}", format_number(v))).unwrap_or_else(|| "—".to_string()),
            p.hoa_monthly.map(|v| format!("${}", format_number(v))).unwrap_or_else(|| "—".to_string())),
        format!("  Monthly:  {}",
            p.monthly_total.map(|v| format!("${}", format_number(v))).unwrap_or_else(|| "—".to_string())),
        format!("  Schools:  {} / {}",
            p.school_elementary.as_deref().unwrap_or("—"),
            p.school_secondary.as_deref().unwrap_or("—")),
        format!("  Features: garage={} AC={} laundry={} radiant={}",
            p.parking_garage.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
            fmt_bool(p.ac),
            fmt_bool(p.laundry_in_unit),
            fmt_bool(p.radiant_floor_heating)),
        String::new(),
        "Search profiles:".to_string(),
    ];

    for profile in profiles {
        lines.push(format!("  [{}] {}: {}", profile.id, profile.title, profile.description));
    }

    lines.push(String::new());
    lines.push("Pick the best matching profile ID, or skip if none fit.".to_string());
    lines.push("Reply with JSON only (no markdown, no explanation):".to_string());
    lines.push(String::new());
    lines.push("If it matches a profile (HumanReview), the comment should say WHY it is a good fit —".to_string());
    lines.push("highlight the specific strengths (price, size, location, features) relative to that profile.".to_string());
    lines.push(r#"{"status":"HumanReview","search_profile_id":N,"comment":"why this property is a strong match"}"#.to_string());
    lines.push(String::new());
    lines.push("If no profile fits (AgentSkip), the comment should say WHY it was skipped —".to_string());
    lines.push("what key criteria it fails (price too high, wrong area, too few beds, etc.).".to_string());
    lines.push(r#"{"status":"AgentSkip","comment":"why this property does not match any profile"}"#.to_string());

    lines.join("\n")
}

fn fmt_bool(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "—",
    }
}

fn format_number(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    let formatted: String = out.chars().rev().collect();
    if n < 0 { format!("-{formatted}") } else { formatted }
}

fn describe_api_key(api_key: &str) -> String {
    format!(
        "masked={} len={} trimmed_len={} leading_ws={} trailing_ws={} contains_newline={}",
        mask_api_key(api_key),
        api_key.len(),
        api_key.trim().len(),
        api_key.chars().next().is_some_and(char::is_whitespace),
        api_key.chars().last().is_some_and(char::is_whitespace),
        api_key.contains(['\n', '\r'])
    )
}

fn mask_api_key(api_key: &str) -> String {
    let prefix_len = 7.min(api_key.len());
    let suffix_start = api_key.len().saturating_sub(4);
    format!("{}****{}", &api_key[..prefix_len], &api_key[suffix_start..])
}

/// The decision extracted from the Claude response.
#[derive(Debug, Deserialize, Serialize)]
struct AgentDecision {
    status: String,
    search_profile_id: Option<i64>,
    comment: String,
}

#[derive(Debug)]
struct AgentReviewFailure {
    code: &'static str,
    user_message: String,
    detail: String,
}

impl AgentReviewFailure {
    fn internal(code: &'static str, user_message: &'static str, detail: String) -> Self {
        Self {
            code,
            user_message: user_message.to_string(),
            detail,
        }
    }

    fn upstream_network(detail: String) -> Self {
        Self {
            code: "upstream_network_error",
            user_message: "Agent review could not reach Claude. Try again shortly.".to_string(),
            detail,
        }
    }
}

impl std::fmt::Display for AgentReviewFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

#[derive(Deserialize)]
struct ClaudeErrorEnvelope {
    error: ClaudeErrorBody,
}

#[derive(Deserialize)]
struct ClaudeErrorBody {
    #[serde(rename = "type")]
    kind: String,
    message: String,
}

fn classify_claude_api_failure(status: reqwest::StatusCode, body: &str) -> AgentReviewFailure {
    let parsed = serde_json::from_str::<ClaudeErrorEnvelope>(body).ok();
    let upstream_message = parsed
        .as_ref()
        .map(|err| err.error.message.as_str())
        .unwrap_or(body)
        .trim();
    let upstream_kind = parsed
        .as_ref()
        .map(|err| err.error.kind.as_str())
        .unwrap_or("unknown_error");
    let detail = format!("Claude API returned {status} ({upstream_kind}): {upstream_message}");

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return AgentReviewFailure {
            code: "authentication_error",
            user_message: "Anthropic authentication failed. Check the configured API key.".to_string(),
            detail,
        };
    }

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return AgentReviewFailure {
            code: "rate_limited",
            user_message: "Anthropic rate-limited the agent review. Try again shortly.".to_string(),
            detail,
        };
    }

    let insufficient_funds = upstream_message.to_ascii_lowercase().contains("insufficient")
        || upstream_message.to_ascii_lowercase().contains("credit")
        || upstream_message.to_ascii_lowercase().contains("fund");
    if status == reqwest::StatusCode::PAYMENT_REQUIRED || insufficient_funds {
        return AgentReviewFailure {
            code: "billing_error",
            user_message: "Anthropic rejected the review because the account has insufficient funds or credits.".to_string(),
            detail,
        };
    }

    AgentReviewFailure {
        code: "upstream_api_error",
        user_message: format!("Anthropic rejected the agent review request: {upstream_message}"),
        detail,
    }
}

/// Extract JSON from the Claude response text (handles stray markdown fences).
fn parse_decision(text: &str) -> Result<AgentDecision, String> {
    // Strip markdown code fences if present.
    let cleaned = text.trim();
    let json_str = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        &cleaned[start..=end]
    } else {
        return Err("no JSON object found in response".to_string());
    };

    let decision: AgentDecision = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    // Validate status value.
    match decision.status.as_str() {
        "HumanReview" | "AgentSkip" => {}
        other => return Err(format!("unexpected status value: {other}")),
    }

    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::property::{ListingStatus, StoredProperty};
    use crate::models::search_profile::SearchProfile;

    fn make_property() -> StoredProperty {
        StoredProperty {
            id: 42,
            search_profile_id: None,
            title: "123 Main St, Vancouver".to_string(),
            description: String::new(),
            price: Some(1_800_000),
            price_currency: Some("CAD".to_string()),
            street_address: Some("123 Main St".to_string()),
            city: Some("Vancouver".to_string()),
            region: None, postal_code: None, country: None,
            bedrooms: Some(4), bathrooms: Some(2),
            sqft: Some(2200), land_sqft: Some(3300),
            year_built: Some(1985),
            property_type: Some("Single Family Residential".to_string()),
            parking_garage: Some(1), parking_total: Some(2),
            parking_carport: None, parking_pad: None,
            radiant_floor_heating: None, ac: Some(false), laundry_in_unit: Some(true),
            skytrain_station: None, skytrain_walk_min: None,
            lat: None, lon: None,
            offer_price: None, property_tax: Some(8_500),
            hoa_monthly: None, down_payment_pct: Some(0.20),
            mortgage_interest_rate: Some(0.05), amortization_years: Some(25),
            mortgage_monthly: Some(8_400), monthly_total: Some(9_100), monthly_cost: None,
            has_rental_suite: Some(true), rental_income: Some(2_000),
            school_elementary: Some("Grandview".to_string()), school_elementary_rating: Some(7.0),
            school_middle: None, school_middle_rating: None,
            school_secondary: Some("Templeton".to_string()), school_secondary_rating: Some(6.5),
            mls_number: None, listed_date: None, source_status: None,
            redfin_url: None, realtor_url: None, rew_url: None, zillow_url: None,
            status: ListingStatus::AgentPending,
            notes: None, agent_review_comment: None,
            agent_review_state: None,
            agent_review_error_code: None,
            agent_review_error_message: None,
            agent_review_started_at: None,
            agent_review_finished_at: None,
            created_at: String::new(), updated_at: None,
        }
    }

    fn make_profiles() -> Vec<SearchProfile> {
        vec![SearchProfile {
            id: 1,
            title: "East Van House".to_string(),
            description: "Single family in East Vancouver, min 3br, budget $2M".to_string(),
            position: 0,
            created_at: String::new(),
            updated_at: None,
            listing_count: 0,
        }]
    }

    #[test]
    fn test_build_prompt_contains_key_fields() {
        let prompt = build_prompt(&make_property(), &make_profiles());
        assert!(prompt.contains("123 Main St, Vancouver"));
        assert!(prompt.contains("1,800,000"));
        assert!(prompt.contains("East Van House"));
        assert!(prompt.contains("HumanReview"));
        assert!(prompt.contains("AgentSkip"));
    }

    #[test]
    fn test_parse_decision_human_pending() {
        let text = r#"{"status":"HumanReview","search_profile_id":1,"comment":"Good match"}"#;
        let d = parse_decision(text).unwrap();
        assert_eq!(d.status, "HumanReview");
        assert_eq!(d.search_profile_id, Some(1));
    }

    #[test]
    fn test_parse_decision_agent_skip() {
        let text = r#"{"status":"AgentSkip","comment":"No profile matches Burnaby"}"#;
        let d = parse_decision(text).unwrap();
        assert_eq!(d.status, "AgentSkip");
        assert_eq!(d.search_profile_id, None);
    }

    #[test]
    fn test_parse_decision_strips_markdown_fences() {
        let text = "```json\n{\"status\":\"AgentSkip\",\"comment\":\"No match\"}\n```";
        let d = parse_decision(text).unwrap();
        assert_eq!(d.status, "AgentSkip");
    }

    #[test]
    fn test_parse_decision_rejects_invalid_status() {
        let text = r#"{"status":"Interested","comment":"foo"}"#;
        assert!(parse_decision(text).is_err());
    }

    #[test]
    fn test_classify_claude_auth_failure() {
        let failure = classify_claude_api_failure(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
        );
        assert_eq!(failure.code, "authentication_error");
        assert!(failure.user_message.contains("API key"));
    }

    #[test]
    fn test_classify_claude_billing_failure() {
        let failure = classify_claude_api_failure(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":{"type":"invalid_request_error","message":"insufficient credits"}}"#,
        );
        assert_eq!(failure.code, "billing_error");
        assert!(failure.user_message.contains("insufficient funds or credits"));
    }
}
