# sqlc-gen-sqlx

A [sqlc](https://sqlc.dev) plugin that generates type-safe [sqlx](https://github.com/transact-rs/sqlx) Rust code from SQL queries.

## What it generates

For each SQL query annotated with a sqlc command, the plugin emits:

- A `const SQL: &str` holding the query text.
- A strongly-typed row struct (`QueryNameRow`) for `:one` / `:many`.
- An optional params struct (`QueryNameParams`) when a query has 2+ parameters.
- A free `pub async fn` (or `pub fn` for batch streams) that executes the query, taking the executor as its first argument.

The executor argument is generic over the `AsExecutor` trait emitted in the same file. `AsExecutor` is implemented for `&PgPool`, `&mut PgConnection`, `&mut Transaction<'_, Postgres>`, `&mut PoolConnection<Postgres>`, and `&mut T` of each — i.e. the natural sqlx reference types:

```rust
// From a pool:
let author = queries::get_author(&pool, 1).await?;

// Pool connection:
let mut conn = pool.acquire().await?;
let author = queries::get_author(&mut conn, 1).await?;

// Transaction:
let mut tx = pool.begin().await?;
queries::delete_author(&mut tx, 1).await?;
tx.commit().await?;
```

## Installation

Add the plugin to your `sqlc.yaml`:

```yaml
version: "2"
plugins:
  - name: sqlc-gen-sqlx
    wasm:
      url: https://github.com/mathematic-inc/sqlc-gen-sqlx/releases/download/v0.2.2/sqlc-gen-sqlx.wasm
      sha256: "b03b1b7fcf1887cbb1cdbdd2b83c0252badd92060764fac0017c0aaa22e4c923"
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
| `overrides` | array | `[]` | Type overrides (`rs_type`, optional `borrowed_rs_type`; see below) |
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

### Borrowed parameters

Add `borrowed_rs_type` to a type or column override to take that type by
reference in parameter positions. Row struct fields, array contents, and the
`Item` of `:copyfrom` chunks continue to use the owned form:

```yaml
options:
  overrides:
    - db_type: "text"
      borrowed_rs_type: "&str"
```

With that override, generated signatures borrow scalar `text` parameters and
the codegen threads lifetimes only where needed:

```rust
// Scalar — lifetime elided
pub async fn get_author_by_name<E: AsExecutor>(
    mut db: E, name: &str,
) -> Result<GetAuthorByNameRow, sqlx::Error> { ... }

// Multiple params — struct carries `'a`, fn uses `'_`
pub struct CreateAuthorParams<'a> {
    pub name: &'a str,
    pub bio: Option<&'a str>,
}
pub async fn create_author<E: AsExecutor>(
    mut db: E, arg: CreateAuthorParams<'_>,
) -> Result<CreateAuthorRow, sqlx::Error> { ... }

// Row struct stays owned — results are returned by value
pub struct GetAuthorByNameRow { pub name: String, /* ... */ }
```

`rs_type` is optional alongside `borrowed_rs_type`. Omit it to keep the
built-in owned default; set both to fully customize:

```yaml
overrides:
  - db_type: "text"
    rs_type: "MyStr"           # used for row fields & array contents
    borrowed_rs_type: "&MyStr" # used for scalar params
```

For `text[]` and `sqlc.slice(text)` the wrapper becomes a borrowed slice while
the inner item stays owned (`&[String]`), so callers can pass `&my_vec`
directly without re-collecting.

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

All functions are free `pub async fn` (or `pub fn` for batch streams) at module scope, taking the executor as their first argument. The bound is `E: AsExecutor`, where `AsExecutor` is the trait emitted in each generated file. Impls cover `&PgPool`, `&mut PgConnection`, `&mut Transaction<'_, Postgres>`, `&mut PoolConnection<Postgres>`, and `&mut T` of each.

Batch methods generate `Stream`-returning APIs and reference `futures_core` and `futures_util` directly. Consumer crates should include those dependencies alongside `sqlx`.

## sqlc extensions

- **`sqlc.slice()`**: Parameters marked as slice expand to `Vec<T>` and support runtime placeholder expansion for `IN (sqlc.slice(...))`-style queries.
- **`sqlc.embed(table)`**: Result columns from an embedded table become a nested struct with `#[sqlx(flatten)]`.

## Contributing

We review change proposals in Discussions before code. [Start a Discussion](https://github.com/mathematic-inc/sqlc-gen-sqlx/discussions/new) and wait for a maintainer's review. If we accept the proposal, a Mathematic maintainer or agent will implement it and open the pull request. When Mathematic implements a proposal, the implementation PR will link to the Discussion and credit its original author. GitHub limits pull request creation to Mathematic maintainers and repository collaborators with write, maintain, or admin access, plus authorized maintenance agents. See [CONTRIBUTING.md](./CONTRIBUTING.md) for the full process.

## License

MIT OR Apache-2.0
