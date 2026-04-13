use sqlx::{Connection as _, PgConnection};

#[path = "../src/db.rs"]
mod queries;

use queries::Queries;

#[tokio::test]
async fn custom_output_filename_is_respected() -> Result<(), Box<dyn std::error::Error>> {
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
            ('Bob', 'Rustacean')",
    )
    .execute(&mut conn)
    .await?;

    let mut q = Queries::new(conn);
    let authors = q.list_authors().await?;

    assert_eq!(authors.len(), 2);
    assert_eq!(authors[0].name, "Alice");
    assert_eq!(authors[1].name, "Bob");

    Ok(())
}
