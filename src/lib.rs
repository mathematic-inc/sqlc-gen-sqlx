//! A sqlc plugin that generates type-safe sqlx Rust code from SQL queries.

/// Enum and composite types of the sqlc catalog.
pub mod catalog;
/// Rust code for the queries and types of a sqlc request.
pub mod codegen;
/// Plugin options.
pub mod config;
/// Formatted Rust source of the output file.
pub mod emit;
/// Errors of the plugin.
pub mod error;
/// Rust identifiers made from PostgreSQL names.
pub mod ident;
/// Rust types for PostgreSQL types.
pub mod types;

// `buf generate` writes this module.
#[allow(
    clippy::all,
    clippy::nursery,
    clippy::pedantic,
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#[path = "mod.rs"]
mod generated;
pub use generated::plugin;

pub use error::Error;

use buffa::{Message as _, MessageView as _};
use plugin::{GenerateRequestView, GenerateResponse};

/// WASI binary entry: read stdin → process → write stdout.
///
/// # Errors
///
/// Returns an error when stdin or stdout fails, or when [`run_with_bytes`] fails.
pub fn run() -> Result<(), Error> {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    std::io::stdin().lock().read_to_end(&mut buf)?;
    let out = run_with_bytes(&buf)?;
    std::io::stdout().lock().write_all(&out)?;
    Ok(())
}

/// Testable core: bytes in, bytes out.
///
/// # Errors
///
/// Returns an error when the request or the plugin options are not valid, or when the code
/// generation fails.
pub fn run_with_bytes(buf: &[u8]) -> Result<Vec<u8>, Error> {
    let request = GenerateRequestView::decode_view(buf)?;
    let config = if request.plugin_options.is_empty() {
        config::Config::default()
    } else {
        config::Config::from_bytes(request.plugin_options)?
    };
    let code = codegen::generate(&request, &config)?;
    let response = GenerateResponse {
        files: vec![plugin::File {
            name: config.output,
            contents: code.into_bytes(),
            ..Default::default()
        }],
        ..Default::default()
    };
    Ok(response.encode_to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use buffa::Message as _;

    fn empty_request_bytes() -> Vec<u8> {
        plugin::GenerateRequest {
            plugin_options: b"{}".to_vec(),
            ..Default::default()
        }
        .encode_to_vec()
    }

    #[test]
    fn empty_request_returns_one_file() {
        let bytes = empty_request_bytes();
        let out = run_with_bytes(&bytes).expect("run_with_bytes failed");
        let resp = plugin::GenerateResponse::decode_from_slice(&out).unwrap();
        assert_eq!(resp.files.len(), 1);
        assert_eq!(resp.files[0].name, "queries.rs");
    }
}
