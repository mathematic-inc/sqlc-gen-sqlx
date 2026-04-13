use futures_util::TryStreamExt;
use sqlx::{Connection as _, PgConnection};

#[path = "../src/queries.rs"]
mod queries;

use queries::Queries;

#[tokio::test]
async fn batchmany_streams_row_groups_per_input() -> Result<(), Box<dyn std::error::Error>> {
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
    let batches = q
        .batch_list_authors_by_bio([Some("%Rust%".to_string()), Some("%SQL%".to_string())])
        .try_collect::<Vec<_>>()
        .await?;

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].len(), 2);
    assert_eq!(batches[1].len(), 1);
    assert_eq!(batches[0][0].name, "Alice");
    assert_eq!(batches[0][1].name, "Bob");
    assert_eq!(batches[1][0].name, "Cara");

    Ok(())
}
