use serde::{Deserialize, Serialize};

/// A saved search / project that groups related property listings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Search {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: Option<String>,
    /// Number of listings belonging to this search (populated by queries).
    #[serde(default)]
    pub listing_count: i64,
}
