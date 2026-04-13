#[path = "queries.rs"]
#[cfg(test)]
mod queries;
#[cfg(test)]
use queries::Queries;
#[cfg(test)]
use sqlx::{Connection as _, PgConnection};

#[cfg(test)]
#[tokio::test]
async fn test_advanced_types_roundtrip() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sqlc:sqlc@localhost:5432/sqlc_test".to_string());
    let mut conn = PgConnection::connect(&db_url).await.expect("connect");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS events (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, flags VARBIT(8) NOT NULL, event_window TSTZRANGE NOT NULL)"
    )
    .execute(&mut conn)
    .await
    .unwrap();

    sqlx::query("TRUNCATE events RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .await
        .unwrap();

    let flags = bit_vec::BitVec::from_bytes(&[0b10101010]);
    let now = chrono::Utc::now();
    let later = now + chrono::Duration::hours(1);
    let window = sqlx::postgres::types::PgRange::from((
        std::ops::Bound::Included(now),
        std::ops::Bound::Excluded(later),
    ));

    sqlx::query("INSERT INTO events (name, flags, event_window) VALUES ($1, $2, $3)")
        .bind("conference")
        .bind(&flags)
        .bind(window)
        .execute(&mut conn)
        .await
        .expect("insert");

    let mut q = Queries::new(conn);
    let event = q.get_event(1).await.expect("get");
    assert_eq!(event.name, "conference");
}

fn main() {}
