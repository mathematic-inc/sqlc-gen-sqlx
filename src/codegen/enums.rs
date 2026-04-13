// src/codegen/enums.rs
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_str;

use crate::{
    catalog::EnumInfo,
    error::Error,
    ident::{type_ident, variant_ident},
};

/// Emit a Rust enum from a PG ENUM type.
///
/// Generated output example:
/// ```text
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
/// #[sqlx(type_name = "status")]
/// pub enum Status {
///     #[sqlx(rename = "active")]
///     Active,
///     #[sqlx(rename = "inactive")]
///     Inactive,
/// }
/// ```
pub fn gen_enum(info: &EnumInfo, extra_derives: &[String]) -> Result<TokenStream, Error> {
    let rust_name = type_ident(&info.rust_name);
    let type_name = &info.type_name;

    let mut variant_tokens = Vec::new();
    for val in &info.vals {
        let variant_name = variant_ident(val); // PascalCase, keyword-safe
        let rename = val.as_str();
        variant_tokens.push(quote! {
            #[sqlx(rename = #rename)]
            #variant_name,
        });
    }

    let mut derive_paths = Vec::new();
    for d in extra_derives {
        let path: syn::Path =
            parse_str(d).map_err(|e| Error::Codegen(format!("invalid derive path '{d}': {e}")))?;
        derive_paths.push(quote! { #path });
    }

    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, #(#derive_paths),*)]
        #[sqlx(type_name = #type_name)]
        pub enum #rust_name {
            #(#variant_tokens)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::EnumInfo;

    fn status_enum() -> EnumInfo {
        EnumInfo {
            schema: "public".to_string(),
            pg_name: "status".to_string(),
            rust_name: "Status".to_string(),
            type_name: "status".to_string(),
            vals: vec![
                "active".to_string(),
                "inactive".to_string(),
                "banned".to_string(),
            ],
        }
    }

    #[test]
    fn generates_enum_with_variants() {
        let tokens = gen_enum(&status_enum(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(code.contains("Status"), "expected 'Status' in:\n{code}");
        assert!(code.contains("Active"), "expected 'Active' in:\n{code}");
        assert!(code.contains("Inactive"), "expected 'Inactive' in:\n{code}");
        assert!(code.contains("Banned"), "expected 'Banned' in:\n{code}");
    }

    #[test]
    fn generates_sqlx_rename_attrs() {
        let tokens = gen_enum(&status_enum(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(
            code.contains(r#""active""#),
            "expected rename = \"active\" in:\n{code}"
        );
        assert!(
            code.contains(r#""inactive""#),
            "expected rename = \"inactive\" in:\n{code}"
        );
    }

    #[test]
    fn generates_sqlx_type_name_attr() {
        let tokens = gen_enum(&status_enum(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(
            code.contains(r#""status""#),
            "expected type_name = \"status\" in:\n{code}"
        );
    }

    #[test]
    fn appends_extra_derives() {
        let tokens = gen_enum(&status_enum(), &["serde::Serialize".to_string()]).unwrap();
        let code = tokens.to_string();
        // quote serializes :: as " :: " with spaces
        assert!(
            code.contains("serde :: Serialize") || code.contains("serde::Serialize"),
            "expected serde::Serialize in:\n{code}"
        );
    }

    #[test]
    fn invalid_extra_derive_returns_error() {
        let result = gen_enum(&status_enum(), &["not a path !!!".to_string()]);
        assert!(result.is_err(), "expected error for invalid derive path");
    }
}
