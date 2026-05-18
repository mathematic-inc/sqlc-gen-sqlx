#[cfg(test)]
use sqlx::{Connection as _, PgConnection};

#[path = "queries.rs"]
#[cfg(test)]
mod queries;
#[cfg(test)]
use queries::CreateAuthorParams;

#[cfg(test)]
#[tokio::test]
async fn test_borrowed_author_roundtrip() {
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

    // Borrow string literals directly — no allocation needed.
    let author = queries::create_author(
        &mut conn,
        CreateAuthorParams {
            name: "Alice",
            bio: Some("Loves Rust"),
        },
    )
    .await
    .expect("create");
    assert_eq!(author.name, "Alice");
    assert_eq!(author.bio.as_deref(), Some("Loves Rust"));

    // Borrow an owned String the caller already has.
    let stored_name: String = author.name.clone();
    let fetched = queries::get_author_by_name(&mut conn, &stored_name)
        .await
        .expect("get_by_name");
    assert_eq!(fetched.id, author.id);

    queries::delete_author(&mut conn, author.id)
        .await
        .expect("delete");
}

fn main() {}
