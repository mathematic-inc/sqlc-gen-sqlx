use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::Queries;

#[tokio::test]
async fn list_authors_by_ids_expands_dynamic_slice() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = std::env::var("DATABASE_URL")?;
    let mut conn = PgConnection::connect(&db_url).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS authors (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            bio TEXT
        )",
    )
    .execute(&mut conn)
    .await?;

    sqlx::query("TRUNCATE authors RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .await?;

    sqlx::query(
        "INSERT INTO authors (name, bio) VALUES
            ('Alice', 'Rust enthusiast'),
            ('Bob', 'Rustacean'),
            ('Cara', 'SQL fan')",
    )
    .execute(&mut conn)
    .await?;

    let mut q = Queries::new(conn);
    let authors = q.list_authors_by_ids(vec![1, 3]).await?;
    let names = authors
        .iter()
        .map(|author| author.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["Alice", "Cara"]);

    Ok(())
}
