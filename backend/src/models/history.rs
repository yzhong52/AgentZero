use serde::Serialize;

/// A record of a field value change on a listing (e.g. price went from X to Y).
#[derive(Serialize, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub listing_id: i64,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_at: String,
}
