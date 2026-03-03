use crate::models::saved_search::SavedSearch;
use sqlx::{Row, SqlitePool};

/// Insert a new search and return it. Position is set to max+1 so it appears last.
pub async fn create(pool: &SqlitePool, title: &str, description: &str) -> Result<SavedSearch, sqlx::Error> {
    let row = sqlx::query(
        r#"INSERT INTO searches (title, description, position)
           VALUES (?, ?, COALESCE((SELECT MAX(position) FROM searches), -1) + 1)
           RETURNING id, title, description, position, created_at, updated_at"#,
    )
    .bind(title)
    .bind(description)
    .fetch_one(pool)
    .await?;

    Ok(SavedSearch {
        id: row.get("id"),
        title: row.get("title"),
        description: row.get("description"),
        position: row.get("position"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        listing_count: 0,
    })
}

/// List all searches ordered by position, with a count of listings in each.
pub async fn list_all(pool: &SqlitePool) -> Result<Vec<SavedSearch>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT s.id, s.title, s.description, s.position, s.created_at, s.updated_at,
                  COUNT(l.id) AS listing_count
           FROM searches s
           LEFT JOIN listings l ON l.search_criteria_id = s.id
           GROUP BY s.id
           ORDER BY s.position ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| SavedSearch {
            id: r.get("id"),
            title: r.get("title"),
            description: r.get("description"),
            position: r.get("position"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            listing_count: r.get("listing_count"),
        })
        .collect())
}

/// Get a single search by ID.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<SavedSearch, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT s.id, s.title, s.description, s.position, s.created_at, s.updated_at,
                  COUNT(l.id) AS listing_count
           FROM searches s
           LEFT JOIN listings l ON l.search_criteria_id = s.id
           WHERE s.id = ?
           GROUP BY s.id"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(SavedSearch {
        id: row.get("id"),
        title: row.get("title"),
        description: row.get("description"),
        position: row.get("position"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        listing_count: row.get("listing_count"),
    })
}

/// Update a search's title and/or description.
pub async fn update(
    pool: &SqlitePool,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
) -> Result<SavedSearch, sqlx::Error> {
    if let Some(t) = title {
        sqlx::query("UPDATE searches SET title = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(t)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(d) = description {
        sqlx::query(
            "UPDATE searches SET description = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(d)
        .bind(id)
        .execute(pool)
        .await?;
    }
    get_by_id(pool, id).await
}

/// Reorder searches: accepts a list of search IDs in the desired order.
/// Each ID is assigned position = its index in the list.
pub async fn reorder(pool: &SqlitePool, ids: &[i64]) -> Result<(), sqlx::Error> {
    for (pos, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE searches SET position = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(pos as i64)
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Delete a search. Listings that belonged to it must be moved first;
/// this function moves them to the first remaining search.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    // Move listings to the first remaining search (by position) if one exists.
    sqlx::query(
        r#"UPDATE listings SET search_criteria_id = (
               SELECT id FROM searches WHERE id != ? ORDER BY position ASC LIMIT 1
           ) WHERE search_criteria_id = ?"#,
    )
    .bind(id)
    .bind(id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM searches WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
