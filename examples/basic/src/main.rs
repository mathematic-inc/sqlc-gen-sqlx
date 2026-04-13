#[cfg(test)]
use sqlx::{Connection as _, PgConnection};

#[path = "queries.rs"]
#[cfg(test)]
mod queries;
#[cfg(test)]
use queries::{CreateAuthorParams, Queries};

#[cfg(test)]
#[tokio::test]
async fn test_author_roundtrip() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sqlc:sqlc@localhost:5432/sqlc_test".to_string());
    let mut conn = PgConnection::connect(&db_url).await.expect("connect");

    sqlx::query("CREATE TABLE IF NOT EXISTS authors (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, bio TEXT)")
        .execute(&mut conn)
        .await
        .unwrap();

    sqlx::query("TRUNCATE authors RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .await
        .unwrap();

    let mut q = Queries::new(conn);

    let author = q
        .create_author(CreateAuthorParams {
            name: "Alice".to_string(),
            bio: Some("Loves Rust".to_string()),
        })
        .await
        .expect("create");
    assert_eq!(author.name, "Alice");

    let fetched = q.get_author(author.id).await.expect("get");
    assert_eq!(fetched.bio, Some("Loves Rust".to_string()));

    let all = q.list_authors().await.expect("list");
    assert!(!all.is_empty());

    let rows = q.delete_author_rows(author.id).await.expect("delete");
    assert_eq!(rows, 1);
}

fn main() {}
