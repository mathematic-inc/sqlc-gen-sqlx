// src/codegen/composites.rs
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_str;

use crate::{catalog::CompositeInfo, error::Error, ident::type_ident};

/// Emit a Rust struct from a PG composite type.
///
/// Generated output example:
/// ```ignore
/// #[derive(Debug, Clone, sqlx::Type)]
/// #[sqlx(type_name = "address")]
/// pub struct Address {
///     pub street: Option<String>,
///     pub city: Option<String>,
///     pub zip: Option<i32>,
/// }
/// ```
pub fn gen_composite(info: &CompositeInfo, extra_derives: &[String]) -> Result<TokenStream, Error> {
    let rust_name = type_ident(&info.rust_name);
    let type_name = &info.type_name;

    let mut field_tokens = Vec::new();
    for field in &info.fields {
        let ident = &field.rust_ident;
        let ty: syn::Type = parse_str(&field.rust_type).map_err(|e| {
            Error::Codegen(format!(
                "composite '{}' field '{}' invalid type '{}': {e}",
                info.pg_name, field.pg_name, field.rust_type
            ))
        })?;
        field_tokens.push(quote! { pub #ident: #ty, });
    }

    let mut derive_paths = Vec::new();
    for d in extra_derives {
        let path: syn::Path =
            parse_str(d).map_err(|e| Error::Codegen(format!("invalid derive path '{d}': {e}")))?;
        derive_paths.push(quote! { #path });
    }

    Ok(quote! {
        #[derive(Debug, Clone, sqlx::Type, #(#derive_paths),*)]
        #[sqlx(type_name = #type_name)]
        pub struct #rust_name {
            #(#field_tokens)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CompositeField, CompositeInfo};

    fn address_composite() -> CompositeInfo {
        CompositeInfo {
            schema: "public".to_string(),
            pg_name: "address".to_string(),
            rust_name: "Address".to_string(),
            type_name: "address".to_string(),
            fields: vec![
                CompositeField {
                    pg_name: "street".to_string(),
                    rust_ident: quote::format_ident!("street"),
                    rust_type: "Option<String>".to_string(),
                },
                CompositeField {
                    pg_name: "city".to_string(),
                    rust_ident: quote::format_ident!("city"),
                    rust_type: "Option<String>".to_string(),
                },
                CompositeField {
                    pg_name: "zip".to_string(),
                    rust_ident: quote::format_ident!("zip"),
                    rust_type: "Option<i32>".to_string(),
                },
            ],
        }
    }

    #[test]
    fn generates_struct_name() {
        let tokens = gen_composite(&address_composite(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(code.contains("Address"), "expected 'Address' in:\n{code}");
    }

    #[test]
    fn generates_field_names() {
        let tokens = gen_composite(&address_composite(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(code.contains("street"), "expected 'street' in:\n{code}");
        assert!(code.contains("city"), "expected 'city' in:\n{code}");
        assert!(code.contains("zip"), "expected 'zip' in:\n{code}");
    }

    #[test]
    fn generates_option_field_types() {
        let tokens = gen_composite(&address_composite(), &[]).unwrap();
        let code = tokens.to_string();
        // quote renders generic types with spaces: Option < String >
        assert!(
            code.contains("Option < String >"),
            "expected Option<String> fields in:\n{code}"
        );
        assert!(
            code.contains("Option < i32 >"),
            "expected Option<i32> for zip in:\n{code}"
        );
    }

    #[test]
    fn generates_sqlx_type_derive() {
        let tokens = gen_composite(&address_composite(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(
            code.contains("sqlx :: Type") || code.contains("sqlx::Type"),
            "expected sqlx::Type derive in:\n{code}"
        );
    }

    #[test]
    fn generates_type_name_attr() {
        let tokens = gen_composite(&address_composite(), &[]).unwrap();
        let code = tokens.to_string();
        assert!(
            code.contains(r#""address""#),
            "expected type_name = \"address\" in:\n{code}"
        );
    }

    #[test]
    fn appends_extra_derives() {
        let tokens =
            gen_composite(&address_composite(), &["serde::Serialize".to_string()]).unwrap();
        let code = tokens.to_string();
        assert!(
            code.contains("serde :: Serialize") || code.contains("serde::Serialize"),
            "expected serde::Serialize in:\n{code}"
        );
    }

    #[test]
    fn invalid_extra_derive_returns_error() {
        let result = gen_composite(&address_composite(), &["not a path !!!".to_string()]);
        assert!(result.is_err(), "expected error for invalid derive path");
    }
}
