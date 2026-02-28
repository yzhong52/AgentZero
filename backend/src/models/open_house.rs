use serde::{Deserialize, Serialize};

/// A single open house event for a listing.
#[derive(Serialize, Deserialize, Clone)]
pub struct OpenHouse {
    pub id: i64,
    pub listing_id: i64,
    pub start_time: String,
    pub end_time: Option<String>,
    pub visited: bool,
    pub created_at: String,
}

/// A parsed open house event (before DB insertion).
#[derive(Serialize)]
pub struct OpenHouseEvent {
    pub start_time: String,
    pub end_time: Option<String>,
}
