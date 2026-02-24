use crate::models::history::HistoryEntry;
use sqlx::{Row, SqlitePool};

/// Record a field value change for a listing.
pub async fn insert_change(
    pool: &SqlitePool,
    listing_id: i64,
    field_name: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO listing_history (listing_id, field_name, old_value, new_value)
         VALUES (?, ?, ?, ?)",
    )
    .bind(listing_id)
    .bind(field_name)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return all history entries for a listing, newest first.
pub async fn list_history(
    pool: &SqlitePool,
    listing_id: i64,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, listing_id, field_name, old_value, new_value, changed_at
         FROM listing_history WHERE listing_id = ? ORDER BY changed_at DESC",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| HistoryEntry {
            id: r.get("id"),
            listing_id: r.get("listing_id"),
            field_name: r.get("field_name"),
            old_value: r.get("old_value"),
            new_value: r.get("new_value"),
            changed_at: r.get("changed_at"),
        })
        .collect())
}
