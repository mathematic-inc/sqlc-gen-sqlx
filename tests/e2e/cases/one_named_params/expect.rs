use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::{CreateAuthorParams, Queries};

#[tokio::test]
async fn create_author_uses_named_params() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut q = Queries::new(conn);
    let author = q
        .create_author(CreateAuthorParams {
            name: "Alice".to_string(),
            bio: Some("Rust enthusiast".to_string()),
        })
        .await?;

    assert_eq!(author.id, 1);
    assert_eq!(author.name, "Alice");
    assert_eq!(author.bio.as_deref(), Some("Rust enthusiast"));

    Ok(())
}
