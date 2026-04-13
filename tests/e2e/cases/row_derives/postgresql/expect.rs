use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::{GetAuthorRow, Queries};

#[tokio::test]
async fn row_derives_are_emitted_from_config() -> Result<(), Box<dyn std::error::Error>> {
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

    sqlx::query("INSERT INTO authors (name, bio) VALUES ('Alice', 'Rust enthusiast')")
        .execute(&mut conn)
        .await?;

    let mut q = Queries::new(conn);
    let author = q.get_author(1).await?;

    assert_eq!(
        author,
        GetAuthorRow {
            id: 1,
            name: "Alice".to_string(),
            bio: Some("Rust enthusiast".to_string()),
        }
    );

    Ok(())
}
