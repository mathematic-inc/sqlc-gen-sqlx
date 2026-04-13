use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse_str;

use crate::{
    config::Config,
    error::Error,
    ident::to_snake_case,
    plugin::QueryView,
    types::{ResolvedType, TypeMap},
};

use super::query::{Param, maybe_params_struct, resolve_params, sql_const};

pub fn gen_copyfrom(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    if params.is_empty() {
        return Err(Error::Codegen(format!(
            ":copyfrom query '{}' has no parameters",
            query.name
        )));
    }

    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let batch_size_name = format_ident!("{}_BATCH_SIZE", to_snake_case(query.name).to_uppercase());
    let insert_prefix = insert_prefix(query.text)?;
    let (const_tokens, const_name) = sql_const(query.name, &insert_prefix);
    let batch_size = std::cmp::max(1usize, 65_535usize / params.len());

    let (params_struct, items_ty, builder_binds) = if params.len() >= 2 {
        let (struct_tokens, struct_ident) =
            maybe_params_struct(query.name, &params, &config.row_derives)?
                .expect("guarded by params.len() >= 2");
        (
            Some(struct_tokens),
            quote! { #struct_ident },
            push_bind_calls(&params, Some(&format_ident!("item"))),
        )
    } else {
        let p = &params[0];
        let ty: syn::Type = parse_str(&p.resolved.rust_type).map_err(|e| {
            Error::Codegen(format!("invalid Rust type '{}': {e}", p.resolved.rust_type))
        })?;
        (None, quote! { #ty }, push_bind_calls(&params, None))
    };

    let inner = quote! {
        pub async fn #fn_name<I>(&mut self, items: I) -> Result<u64, sqlx::Error>
        where
            I: IntoIterator<Item = #items_ty>,
        {
            let mut rows_affected = 0u64;
            let mut items = items.into_iter();

            loop {
                let chunk = items.by_ref().take(#batch_size_name).collect::<Vec<_>>();
                if chunk.is_empty() {
                    break;
                }

                let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(#const_name);
                query_builder.push_values(chunk, |mut b, item| {
                    #builder_binds
                });

                rows_affected += query_builder.build().execute(&mut self.db).await?.rows_affected();
            }

            Ok(rows_affected)
        }
    };

    Ok((
        quote! {
            #params_struct
            #const_tokens
            const #batch_size_name: usize = #batch_size;
        },
        inner,
    ))
}

fn insert_prefix(sql: &str) -> Result<String, Error> {
    let lowered = sql.to_ascii_lowercase();
    let values_idx = lowered.find("values").ok_or_else(|| {
        Error::Codegen(format!(
            ":copyfrom query must be an INSERT ... VALUES statement, got '{sql}'"
        ))
    })?;
    Ok(format!("{} ", sql[..values_idx].trim_end()))
}

fn push_bind_calls(params: &[Param], use_arg: Option<&proc_macro2::Ident>) -> TokenStream {
    params
        .iter()
        .map(|p| {
            let ident = &p.ident;
            let value = if use_arg.is_some() {
                quote! { item.#ident }
            } else {
                quote! { item }
            };
            quote! {
                b.push_bind(#value);
            }
        })
        .collect()
}
