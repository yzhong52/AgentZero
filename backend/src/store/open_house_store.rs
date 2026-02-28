use crate::models::open_house::{OpenHouse, OpenHouseEvent};
use sqlx::{Row, SqlitePool};

/// Insert open house events for a listing, ignoring duplicates (same listing + start_time).
pub async fn upsert_open_houses(
    pool: &SqlitePool,
    listing_id: i64,
    events: &[OpenHouseEvent],
) -> Result<(), sqlx::Error> {
    for event in events {
        sqlx::query(
            "INSERT INTO open_houses (listing_id, start_time, end_time)
             VALUES (?, ?, ?)
             ON CONFLICT (listing_id, start_time) DO NOTHING",
        )
        .bind(listing_id)
        .bind(&event.start_time)
        .bind(event.end_time.as_deref())
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Return all open house events for a listing, ordered by start_time ascending.
pub async fn list_open_houses(
    pool: &SqlitePool,
    listing_id: i64,
) -> Result<Vec<OpenHouse>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, listing_id, start_time, end_time, visited, created_at
         FROM open_houses WHERE listing_id = ? ORDER BY start_time ASC",
    )
    .bind(listing_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| OpenHouse {
            id: r.get("id"),
            listing_id: r.get("listing_id"),
            start_time: r.get("start_time"),
            end_time: r.get("end_time"),
            visited: r.get::<i64, _>("visited") != 0,
            created_at: r.get("created_at"),
        })
        .collect())
}

/// Toggle the visited flag on a single open house event, scoped to the given listing.
///
/// Returns `Ok(false)` if the open house ID does not belong to `listing_id`.
pub async fn patch_open_house_visited(
    pool: &SqlitePool,
    listing_id: i64,
    oh_id: i64,
    visited: bool,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("UPDATE open_houses SET visited = ? WHERE id = ? AND listing_id = ?")
            .bind(if visited { 1i64 } else { 0i64 })
            .bind(oh_id)
            .bind(listing_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}
