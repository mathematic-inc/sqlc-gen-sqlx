# sqlc-gen-sqlx

A [sqlc](https://sqlc.dev) plugin that generates type-safe [sqlx](https://github.com/launchbadge/sqlx) Rust code from SQL queries.

## What it generates

For each SQL query annotated with a sqlc command, the plugin emits:

- A `const SQL: &str` holding the query text.
- A strongly-typed row struct (`QueryNameRow`) for `:one` / `:many`.
- An optional params struct (`QueryNameParams`) when a query has 2+ parameters.
- A `&mut self` method on `pub struct Queries<E>` that executes the query.

`Queries<E>` wraps anything implementing the generated `AsExecutor` trait. `AsExecutor` is implemented for `PgPool`, `&PgPool`, `PgConnection`, `Transaction<'_, Postgres>`, `PoolConnection<Postgres>`, and `&mut T` of each:

```rust
// From a pool:
let mut q = Queries::new(&pool);
let author = q.get_author(1).await?;

// Borrowed or owned pool connection:
let mut conn = pool.acquire().await?;
let mut q = Queries::new(&mut conn);
// ...or Queries::new(conn) to take ownership.

// Transactions:
let mut tx = pool.begin().await?;
let mut q = Queries::new(&mut tx);
q.delete_author(1).await?;
tx.commit().await?;
```

## Installation

Add the plugin to your `sqlc.yaml`:

```yaml
version: "2"
plugins:
  - name: sqlc-gen-sqlx
    wasm:
      url: https://github.com/your-org/sqlc-gen-sqlx/releases/download/v0.1.0/sqlc-gen-sqlx.wasm
      sha256: "<sha256 of the wasm file>"
sql:
  - engine: postgresql
    queries: queries.sql
    schema: schema.sql
    codegen:
      - plugin: sqlc-gen-sqlx
        out: src/
        options:
          output: queries.rs
```

## Configuration

All options are passed in `codegen[*].options`:

| Key | Type | Default | Description |
|---|---|---|---|
| `output` | string | `queries.rs` | Output filename |
| `overrides` | array | `[]` | Type overrides (see below) |
| `row_derives` | array | `[]` | Extra derives for row and params structs |
| `enum_derives` | array | `[]` | Extra derives for generated enum types |
| `composite_derives` | array | `[]` | Extra derives for generated composite types |
| `copy_cheap_types` | array | `[]` | Type names to mark as copy-cheap |

### Type overrides

Override the Rust type used for a PostgreSQL column type or a specific column:

```yaml
options:
  overrides:
    - db_type: "timestamptz"
      rs_type: "time::OffsetDateTime"
      copy_cheap: false
    - column: "users.created_at"
      rs_type: "chrono::DateTime<chrono::Local>"
      copy_cheap: false
```

## Supported PostgreSQL types

| PostgreSQL | Rust |
|---|---|
| `bool` | `bool` |
| `int2` / `smallint` | `i16` |
| `int4` / `integer` / `int` | `i32` |
| `int8` / `bigint` | `i64` |
| `float4` / `real` | `f32` |
| `float8` / `double precision` | `f64` |
| `numeric` / `decimal` | `bigdecimal::BigDecimal` |
| `text` / `varchar` / `bpchar` / `citext` | `String` |
| `bytea` | `Vec<u8>` |
| `uuid` | `uuid::Uuid` |
| `json` / `jsonb` | `serde_json::Value` |
| `timestamptz` | `chrono::DateTime<chrono::Utc>` |
| `timestamp` | `chrono::NaiveDateTime` |
| `date` | `chrono::NaiveDate` |
| `time` | `chrono::NaiveTime` |
| `inet` / `cidr` | `ipnetwork::IpNetwork` |
| `macaddr` | `mac_address::MacAddress` |
| `hstore` | `std::collections::HashMap<String, Option<String>>` |
| `interval` | `sqlx::postgres::types::PgInterval` |
| `money` | `sqlx::postgres::types::PgMoney` |
| `oid` | `sqlx::postgres::types::Oid` |
| `int4range` | `sqlx::postgres::types::PgRange<i32>` |
| `int8range` | `sqlx::postgres::types::PgRange<i64>` |
| `numrange` | `sqlx::postgres::types::PgRange<bigdecimal::BigDecimal>` |
| `tsrange` | `sqlx::postgres::types::PgRange<chrono::NaiveDateTime>` |
| `tstzrange` | `sqlx::postgres::types::PgRange<chrono::DateTime<chrono::Utc>>` |
| `daterange` | `sqlx::postgres::types::PgRange<chrono::NaiveDate>` |
| `bit` / `varbit` | `bit_vec::BitVec` |
| PostgreSQL ENUM | generated Rust enum |
| PostgreSQL composite | generated Rust struct |

Array types (`type[]`) become `Vec<T>`. Nullable columns become `Option<T>`.

## Supported query annotations

| Annotation | Return type | Description |
|---|---|---|
| `:exec` | `Result<(), sqlx::Error>` | Execute, discard result |
| `:execrows` | `Result<u64, sqlx::Error>` | Execute, return rows affected |
| `:execresult` | `Result<sqlx::postgres::PgQueryResult, sqlx::Error>` | Execute, return full result |
| `:execlastid` | `Result<T, sqlx::Error>` | Execute with RETURNING, return scalar |
| `:one` | `Result<QueryRow, sqlx::Error>` | Fetch exactly one row |
| `:many` | `Result<Vec<QueryRow>, sqlx::Error>` | Fetch all rows |
| `:batchexec` | `impl Stream<Item = Result<(), sqlx::Error>>` | Lazily execute once per item |
| `:batchone` | `impl Stream<Item = Result<QueryRow, sqlx::Error>>` | Lazily fetch one row per item |
| `:batchmany` | `impl Stream<Item = Result<Vec<QueryRow>, sqlx::Error>>` | Lazily fetch all rows per item |
| `:copyfrom` | `Result<u64, sqlx::Error>` | Chunked bulk insert from any `IntoIterator` |

All functions are `&mut self` methods on `Queries<E>`. The bound is `E: AsExecutor`, where `AsExecutor` is the trait emitted in each generated file. Impls cover `PgPool`, `&PgPool`, `PgConnection`, `Transaction<'_, Postgres>`, `PoolConnection<Postgres>`, and `&mut T` of each.

Batch methods generate `Stream`-returning APIs and reference `futures_core` and `futures_util` directly. Consumer crates should include those dependencies alongside `sqlx`.

## sqlc extensions

- **`sqlc.slice()`**: Parameters marked as slice expand to `Vec<T>` and support runtime placeholder expansion for `IN (sqlc.slice(...))`-style queries.
- **`sqlc.embed(table)`**: Result columns from an embedded table become a nested struct with `#[sqlx(flatten)]`.

## License

MIT OR Apache-2.0
