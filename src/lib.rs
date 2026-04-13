pub mod catalog;
pub mod codegen;
pub mod config;
pub mod emit;
pub mod error;
pub mod ident;
pub mod plugin;
pub mod types;

pub use error::Error;

use buffa::{Message as _, MessageView as _};
use plugin::{GenerateRequestView, GenerateResponse};

/// WASI binary entry: read stdin → process → write stdout.
pub fn run() -> Result<(), Error> {
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    std::io::stdin().lock().read_to_end(&mut buf)?;
    let out = run_with_bytes(&buf)?;
    std::io::stdout().lock().write_all(&out)?;
    Ok(())
}

/// Testable core: bytes in, bytes out.
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
            name: config.output.clone(),
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
