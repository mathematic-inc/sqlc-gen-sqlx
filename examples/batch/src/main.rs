#[cfg(test)]
#[path = "queries.rs"]
mod queries;
#[cfg(test)]
use futures_util::TryStreamExt;
#[cfg(test)]
use queries::Queries;
#[cfg(test)]
use sqlx::{Connection as _, PgConnection};

#[cfg(test)]
#[tokio::test]
async fn test_batch_roundtrip() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sqlc:sqlc@localhost:5432/sqlc_test".to_string());
    let mut conn = PgConnection::connect(&db_url).await.expect("connect");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS authors (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, bio TEXT)"
    )
    .execute(&mut conn)
    .await
    .unwrap();

    sqlx::query("TRUNCATE authors RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .await
        .unwrap();

    sqlx::query("INSERT INTO authors (name) VALUES ($1), ($2)")
        .bind("Alice")
        .bind("Bob")
        .execute(&mut conn)
        .await
        .expect("insert");

    let mut q = Queries::new(conn);

    let authors: Vec<_> = q
        .batch_get_author(vec![1, 2])
        .try_collect()
        .await
        .expect("batch get");
    assert_eq!(authors.len(), 2);
    assert_eq!(authors[0].name, "Alice");
    assert_eq!(authors[1].name, "Bob");

    let _: Vec<()> = q
        .batch_delete_author(vec![1, 2])
        .try_collect()
        .await
        .expect("batch delete");

    let mut verify_conn = PgConnection::connect(&db_url).await.expect("reconnect");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM authors")
        .fetch_one(&mut verify_conn)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

fn main() {}
