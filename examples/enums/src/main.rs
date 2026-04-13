#[path = "queries.rs"]
#[cfg(test)]
mod queries;
#[cfg(test)]
use queries::{Queries, Status};
#[cfg(test)]
use sqlx::{Connection as _, PgConnection};

#[cfg(test)]
#[tokio::test]
async fn test_enum_roundtrip() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sqlc:sqlc@localhost:5432/sqlc_test".to_string());
    let mut conn = PgConnection::connect(&db_url).await.expect("connect");

    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(&mut conn)
        .await
        .unwrap();

    sqlx::query("DROP TYPE IF EXISTS status")
        .execute(&mut conn)
        .await
        .unwrap();

    sqlx::query("CREATE TYPE status AS ENUM ('active', 'inactive', 'pending')")
        .execute(&mut conn)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, status status NOT NULL DEFAULT 'active')"
    )
    .execute(&mut conn)
    .await
    .unwrap();

    sqlx::query("TRUNCATE users RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .await
        .unwrap();

    let mut q = Queries::new(conn);

    q.create_user("Alice".to_string(), Status::Active)
        .await
        .expect("create");

    let user = q.get_user(1).await.expect("get");
    assert_eq!(user.name, "Alice");
    assert_eq!(user.status, Status::Active);

    let active = q.list_users_by_status(Status::Active).await.expect("list");
    assert_eq!(active.len(), 1);
}

fn main() {}
