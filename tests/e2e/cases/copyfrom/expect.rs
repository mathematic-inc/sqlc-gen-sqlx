use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::{CopyAuthorsParams, Queries};

#[tokio::test]
async fn copyfrom_inserts_every_item() -> Result<(), Box<dyn std::error::Error>> {
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
    let inserted = q
        .copy_authors([
            CopyAuthorsParams {
                name: "Bob".to_string(),
                bio: Some("Rustacean".to_string()),
            },
            CopyAuthorsParams {
                name: "Cara".to_string(),
                bio: Some("SQL fan".to_string()),
            },
        ])
        .await?;

    assert_eq!(inserted, 2);

    let mut verify = PgConnection::connect(&db_url).await?;
    let names = sqlx::query_scalar::<_, String>("SELECT name FROM authors ORDER BY id")
        .fetch_all(&mut verify)
        .await?;

    assert_eq!(names, vec!["Bob".to_string(), "Cara".to_string()]);

    Ok(())
}
