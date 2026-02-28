use crate::models::search::Search;
use sqlx::{Row, SqlitePool};

/// Insert a new search and return it.
pub async fn create(pool: &SqlitePool, title: &str, description: &str) -> Result<Search, sqlx::Error> {
    let row = sqlx::query(
        r#"INSERT INTO searches (title, description)
           VALUES (?, ?)
           RETURNING id, title, description, created_at, updated_at"#,
    )
    .bind(title)
    .bind(description)
    .fetch_one(pool)
    .await?;

    Ok(Search {
        id: row.get("id"),
        title: row.get("title"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        listing_count: 0,
    })
}

/// List all searches, newest first, with a count of listings in each.
pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Search>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT s.id, s.title, s.description, s.created_at, s.updated_at,
                  COUNT(l.id) AS listing_count
           FROM searches s
           LEFT JOIN listings l ON l.search_id = s.id
           GROUP BY s.id
           ORDER BY s.created_at DESC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Search {
            id: r.get("id"),
            title: r.get("title"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            listing_count: r.get("listing_count"),
        })
        .collect())
}

/// Get a single search by ID.
pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Search, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT s.id, s.title, s.description, s.created_at, s.updated_at,
                  COUNT(l.id) AS listing_count
           FROM searches s
           LEFT JOIN listings l ON l.search_id = s.id
           WHERE s.id = ?
           GROUP BY s.id"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(Search {
        id: row.get("id"),
        title: row.get("title"),
        description: row.get("description"),
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
) -> Result<Search, sqlx::Error> {
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

/// Delete a search. Listings that belonged to it will have search_id set to NULL.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    // Detach listings first (set search_id = NULL).
    sqlx::query("UPDATE listings SET search_id = NULL WHERE search_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM searches WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
