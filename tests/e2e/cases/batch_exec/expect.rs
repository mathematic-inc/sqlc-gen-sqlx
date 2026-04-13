use futures_util::TryStreamExt;
use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::Queries;

#[tokio::test]
async fn batchexec_streams_one_result_per_input() -> Result<(), Box<dyn std::error::Error>> {
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
    let deleted = q
        .batch_delete_author([1_i64, 2, 3])
        .try_collect::<Vec<_>>()
        .await?;

    assert_eq!(deleted, vec![(), (), ()]);

    let mut verify = PgConnection::connect(&db_url).await?;
    let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM authors")
        .fetch_one(&mut verify)
        .await?;

    assert_eq!(remaining, 0);

    Ok(())
}
