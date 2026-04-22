use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse_str;

use crate::{
    config::Config,
    error::Error,
    ident::{field_ident, query_params_name, to_pascal_case, to_snake_case, type_ident},
    plugin::{ColumnView, ParameterView, QueryView},
    types::{ResolvedType, TypeMap},
};

/// Resolved parameter: Rust identifier + type.
pub(crate) struct Param {
    pub(crate) number: i32,
    pub(crate) ident: proc_macro2::Ident,
    pub(crate) source_name: String,
    pub(crate) is_slice: bool,
    pub(crate) resolved: ResolvedType,
}

/// Columns from an embedded table (`sqlc.embed(table)`), grouped together.
pub(crate) struct EmbeddedGroup {
    /// snake_case field name in the parent struct, e.g. `author`
    pub(crate) embed_field_ident: proc_macro2::Ident,
    /// PascalCase struct name for the sub-struct, e.g. `AuthorEmbed`
    pub(crate) struct_ident: proc_macro2::Ident,
    /// Fields of the sub-struct in declaration order
    pub(crate) fields: Vec<(proc_macro2::Ident, ResolvedType)>,
}

/// Result of resolving a query's output columns.
pub(crate) struct ResolvedColumnSet {
    /// Columns that map directly to fields in the row struct.
    pub(crate) flat: Vec<(proc_macro2::Ident, ResolvedType)>,
    /// Groups of columns that map to embedded sub-structs (from `sqlc.embed()`).
    pub(crate) embedded: Vec<EmbeddedGroup>,
}

pub(crate) fn resolve_params<'a>(
    params: impl Iterator<Item = &'a ParameterView<'a>>,
    type_map: &TypeMap,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<Vec<Param>, Error> {
    let mut out = Vec::new();
    for p in params {
        let col: &ColumnView<'_> = p
            .column
            .as_option()
            .ok_or_else(|| Error::Codegen("parameter missing column".into()))?;
        let pg_type = col.r#type.as_option().map(|t| t.name).unwrap_or("");
        let nullable = !col.not_null;
        let array_dims = if col.is_sqlc_slice {
            1usize
        } else if col.array_dims > 0 {
            col.array_dims as usize
        } else {
            usize::from(col.is_array)
        };
        let col_key = col
            .table
            .as_option()
            .map(|t| format!("{}.{}", t.name, col.name));
        let resolved = type_map
            .resolve_column_dims(
                pg_type,
                nullable,
                array_dims,
                col_key.as_deref(),
                col_overrides,
            )
            .ok_or_else(|| Error::Codegen(format!("unknown PG type: {pg_type}")))?;
        let param_name = if col.is_named_param && !col.original_name.is_empty() {
            col.original_name
        } else {
            col.name
        };
        out.push(Param {
            number: p.number,
            ident: field_ident(param_name),
            source_name: param_name.to_string(),
            is_slice: col.is_sqlc_slice,
            resolved,
        });
    }
    Ok(out)
}

/// Emit a params struct when the query has ≥2 parameters.
pub(crate) fn maybe_params_struct(
    query_name: &str,
    params: &[Param],
    extra_derives: &[String],
) -> Result<Option<(TokenStream, proc_macro2::Ident)>, Error> {
    if params.len() < 2 {
        return Ok(None);
    }
    let struct_name = type_ident(&query_params_name(query_name));
    let mut field_tokens = Vec::new();
    for p in params {
        let ident = &p.ident;
        let ty: syn::Type = parse_str(&p.resolved.rust_type).map_err(|e| {
            Error::Codegen(format!("invalid Rust type '{}': {e}", p.resolved.rust_type))
        })?;
        field_tokens.push(quote! { pub #ident: #ty, });
    }
    let mut derive_paths = Vec::new();
    for d in extra_derives {
        let path: syn::Path =
            parse_str(d).map_err(|e| Error::Codegen(format!("invalid derive path '{d}': {e}")))?;
        derive_paths.push(quote! { #path });
    }
    let tokens = quote! {
        #[derive(Debug, Clone, #(#derive_paths),*)]
        pub struct #struct_name {
            #(#field_tokens)*
        }
    };
    Ok(Some((tokens, struct_name)))
}

/// Emit `.bind(...)` calls for a query function body.
pub(crate) fn bind_calls(params: &[Param], use_arg: Option<&proc_macro2::Ident>) -> TokenStream {
    params
        .iter()
        .map(|p| {
            let ident = &p.ident;
            let value = if let Some(arg) = use_arg {
                quote! { #arg.#ident }
            } else {
                quote! { #ident }
            };
            quote! { .bind(#value) }
        })
        .collect()
}

fn param_value_expr(param: &Param, use_arg: Option<&proc_macro2::Ident>) -> TokenStream {
    let ident = &param.ident;
    if let Some(arg) = use_arg {
        quote! { #arg.#ident }
    } else {
        quote! { #ident }
    }
}

fn ordered_params(params: &[Param]) -> Vec<&Param> {
    let mut ordered = params.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|p| p.number);
    ordered
}

fn reverse_non_slice_params(params: &[Param]) -> Vec<&Param> {
    let mut ordered = params.iter().filter(|p| !p.is_slice).collect::<Vec<_>>();
    ordered.sort_by_key(|p| std::cmp::Reverse(p.number));
    ordered
}

pub(crate) fn has_dynamic_slice(sql: &str, params: &[Param]) -> bool {
    params
        .iter()
        .any(|param| param.is_slice && !uses_native_array_binding(sql, param.number))
}

fn uses_native_array_binding(sql: &str, param_number: i32) -> bool {
    let compact = sql
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let placeholder = format!("${param_number}");

    compact.contains(&format!("ANY({placeholder})"))
        || compact.contains(&format!("ANY({placeholder}::"))
        || compact.contains(&format!("ALL({placeholder})"))
        || compact.contains(&format!("ALL({placeholder}::"))
}

pub(crate) fn dynamic_sql_setup(
    sql_const: &proc_macro2::Ident,
    params: &[Param],
    use_arg: Option<&proc_macro2::Ident>,
) -> TokenStream {
    let mut tokens = Vec::new();
    tokens.push(quote! {
        let mut sql = #sql_const.to_string();
    });

    for param in reverse_non_slice_params(params) {
        let original = format!("${}", param.number);
        let temporary = format!("__SQLC_PARAM_{}__", param.number);
        tokens.push(quote! {
            sql = sql.replace(#original, #temporary);
        });
    }

    let ordered = ordered_params(params);
    if ordered.len() > 1 {
        tokens.push(quote! {
            let mut next_placeholder = 1usize;
        });
    } else if ordered.len() == 1 {
        tokens.push(quote! {
            let next_placeholder = 1usize;
        });
    }
    let last_idx = ordered.len().saturating_sub(1);
    for (idx, param) in ordered.iter().enumerate() {
        let is_last = idx == last_idx;
        let placeholder_ident = format_ident!("placeholder_{}", param.number as usize);
        tokens.push(quote! {
            let #placeholder_ident = next_placeholder;
        });

        if param.is_slice {
            let value_expr = param_value_expr(param, use_arg);
            let marker = format!("/*SLICE:{}*/?", param.source_name);
            let numbered_marker = format!("/*SLICE:{}*/${}", param.source_name, param.number);
            let bare_placeholder = format!("${}", param.number);
            let advance = if is_last {
                quote! {}
            } else {
                quote! { next_placeholder += slice_len; }
            };
            tokens.push(quote! {
                let slice_len = (#value_expr).len();
                let replacement = if slice_len == 0 {
                    "NULL".to_string()
                } else {
                    (#placeholder_ident..(#placeholder_ident + slice_len))
                        .map(|n| format!("${}", n))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                if sql.contains(#marker) {
                    sql = sql.replace(#marker, &replacement);
                } else {
                    if sql.contains(#numbered_marker) {
                        sql = sql.replace(#numbered_marker, &replacement);
                    } else {
                        sql = sql.replace(#bare_placeholder, &replacement);
                    }
                }
                #advance
            });
        } else {
            let temporary = format!("__SQLC_PARAM_{}__", param.number);
            let advance = if is_last {
                quote! {}
            } else {
                quote! { next_placeholder += 1; }
            };
            tokens.push(quote! {
                sql = sql.replace(#temporary, &format!("${}", #placeholder_ident));
                #advance
            });
        }
    }

    quote! { #(#tokens)* }
}

pub(crate) fn dynamic_bind_statements(
    params: &[Param],
    use_arg: Option<&proc_macro2::Ident>,
) -> TokenStream {
    ordered_params(params)
        .into_iter()
        .map(|param| {
            let value_expr = param_value_expr(param, use_arg);
            if param.is_slice {
                quote! {
                    for value in #value_expr {
                        query = query.bind(value);
                    }
                }
            } else {
                quote! {
                    query = query.bind(#value_expr);
                }
            }
        })
        .collect()
}

/// Generate a SQL string constant, returning both the token stream and the
/// constant's identifier so callers don't need to recompute it.
pub(crate) fn sql_const(query_name: &str, sql: &str) -> (TokenStream, proc_macro2::Ident) {
    let const_name = format_ident!("{}", to_snake_case(query_name).to_uppercase());
    let tokens = quote! {
        const #const_name: &str = #sql;
    };
    (tokens, const_name)
}

/// Resolve result columns into flat fields and embedded groups.
pub(crate) fn resolve_columns<'a>(
    cols: impl Iterator<Item = &'a ColumnView<'a>>,
    type_map: &TypeMap,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<ResolvedColumnSet, Error> {
    let mut flat: Vec<(proc_macro2::Ident, ResolvedType)> = Vec::new();
    let mut embedded_groups: Vec<(String, Vec<(proc_macro2::Ident, ResolvedType)>)> = Vec::new();

    for col in cols {
        let pg_type = col.r#type.as_option().map(|t| t.name).unwrap_or("");
        if pg_type.is_empty() {
            return Err(Error::Codegen(format!("column '{}' has no type", col.name)));
        }
        let nullable = !col.not_null;
        let array_dims = if col.array_dims > 0 {
            col.array_dims as usize
        } else {
            usize::from(col.is_array)
        };
        let col_key = col
            .table
            .as_option()
            .map(|t| format!("{}.{}", t.name, col.name));
        let resolved = type_map
            .resolve_column_dims(
                pg_type,
                nullable,
                array_dims,
                col_key.as_deref(),
                col_overrides,
            )
            .ok_or_else(|| Error::Codegen(format!("unknown PG type: {pg_type}")))?;

        if let Some(embed_id) = col.embed_table.as_option() {
            let embed_name = embed_id.name.to_string();
            if let Some(group) = embedded_groups.iter_mut().find(|(n, _)| n == &embed_name) {
                group.1.push((field_ident(col.name), resolved));
            } else {
                embedded_groups.push((embed_name, vec![(field_ident(col.name), resolved)]));
            }
        } else {
            flat.push((field_ident(col.name), resolved));
        }
    }

    let embedded = embedded_groups
        .into_iter()
        .map(|(name, fields)| EmbeddedGroup {
            embed_field_ident: field_ident(&name),
            struct_ident: type_ident(&format!("{}Embed", to_pascal_case(&name))),
            fields,
        })
        .collect();

    Ok(ResolvedColumnSet { flat, embedded })
}

/// Emit the row struct for :one / :many, handling both flat and embedded columns.
pub(crate) fn row_struct(
    query_name: &str,
    cols: &ResolvedColumnSet,
    extra_derives: &[String],
) -> Result<TokenStream, Error> {
    let struct_name = type_ident(&crate::ident::query_row_name(query_name));

    let mut derive_paths: Vec<proc_macro2::TokenStream> = Vec::new();
    for d in extra_derives {
        let path: syn::Path =
            parse_str(d).map_err(|e| Error::Codegen(format!("invalid derive path '{d}': {e}")))?;
        derive_paths.push(quote! { #path });
    }

    let mut embed_struct_tokens: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_tokens: Vec<proc_macro2::TokenStream> = Vec::new();

    for embed in &cols.embedded {
        let embed_struct_ident = &embed.struct_ident;
        let mut embed_field_tokens: Vec<proc_macro2::TokenStream> = Vec::new();
        for (ident, resolved) in &embed.fields {
            let ty: syn::Type = parse_str(&resolved.rust_type).map_err(|e| {
                Error::Codegen(format!("invalid Rust type '{}': {e}", resolved.rust_type))
            })?;
            embed_field_tokens.push(quote! { pub #ident: #ty, });
        }
        embed_struct_tokens.push(quote! {
            #[derive(Debug, Clone, sqlx::FromRow, #(#derive_paths),*)]
            pub struct #embed_struct_ident {
                #(#embed_field_tokens)*
            }
        });
        let f_ident = &embed.embed_field_ident;
        field_tokens.push(quote! {
            #[sqlx(flatten)]
            pub #f_ident: #embed_struct_ident,
        });
    }

    for (ident, resolved) in &cols.flat {
        let ty: syn::Type = parse_str(&resolved.rust_type).map_err(|e| {
            Error::Codegen(format!("invalid Rust type '{}': {e}", resolved.rust_type))
        })?;
        field_tokens.push(quote! { pub #ident: #ty, });
    }

    Ok(quote! {
        #(#embed_struct_tokens)*
        #[derive(Debug, Clone, sqlx::FromRow, #(#derive_paths),*)]
        pub struct #struct_name {
            #(#field_tokens)*
        }
    })
}

/// Build the parameters portion of a query function signature.
/// Returns `(params_struct_tokens, arg_ident, fn_params_tokens)`.
pub(crate) fn build_fn_params(
    query_name: &str,
    params: &[Param],
    derives: &[String],
) -> Result<(Option<TokenStream>, Option<proc_macro2::Ident>, TokenStream), Error> {
    if params.len() >= 2 {
        // SAFETY: `maybe_params_struct` returns `Some` when `params.len() >= 2`.
        let (struct_tokens, struct_ident) = maybe_params_struct(query_name, params, derives)?
            .expect("guarded by params.len() >= 2 check above");
        let arg = format_ident!("arg");
        Ok((
            Some(struct_tokens),
            Some(arg.clone()),
            quote! { #arg: #struct_ident },
        ))
    } else if params.len() == 1 {
        let p = &params[0];
        let ident = &p.ident;
        let ty: syn::Type = parse_str(&p.resolved.rust_type).map_err(|e| {
            Error::Codegen(format!("invalid Rust type '{}': {e}", p.resolved.rust_type))
        })?;
        Ok((None, None, quote! { #ident: #ty }))
    } else {
        Ok((None, None, quote! {}))
    }
}

/// `:one` → `async fn foo(exec, [params]) -> Result<FooRow, sqlx::Error>`
pub fn gen_one(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let columns = resolve_columns(query.columns.iter(), type_map, col_overrides)?;

    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let row_name = type_ident(&crate::ident::query_row_name(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);

    let row_tokens = row_struct(query.name, &columns, &config.row_derives)?;

    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;

    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let inner_fn = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<#row_name, sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query_as::<_, #row_name>(&sql);
                #bind_setup
                query.fetch_one(self.db.as_executor()).await
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<#row_name, sqlx::Error> {
                sqlx::query_as::<_, #row_name>(#const_name)
                    #binds
                    .fetch_one(self.db.as_executor())
                    .await
            }
        }
    };

    let outer = quote! {
        #params_struct
        #const_tokens
        #row_tokens
    };

    Ok((outer, inner_fn))
}

/// `:many` → `async fn foo(exec, [params]) -> Result<Vec<FooRow>, sqlx::Error>`
pub fn gen_many(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let columns = resolve_columns(query.columns.iter(), type_map, col_overrides)?;

    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let row_name = type_ident(&crate::ident::query_row_name(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);

    let row_tokens = row_struct(query.name, &columns, &config.row_derives)?;
    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;
    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let inner_fn = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<Vec<#row_name>, sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query_as::<_, #row_name>(&sql);
                #bind_setup
                query.fetch_all(self.db.as_executor()).await
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<Vec<#row_name>, sqlx::Error> {
                sqlx::query_as::<_, #row_name>(#const_name)
                    #binds
                    .fetch_all(self.db.as_executor())
                    .await
            }
        }
    };

    let outer = quote! {
        #params_struct
        #const_tokens
        #row_tokens
    };

    Ok((outer, inner_fn))
}

/// `:execrows` → `async fn foo(exec, [params]) -> Result<u64, sqlx::Error>`
pub fn gen_execrows(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;
    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);
    let inner = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<u64, sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query(&sql);
                #bind_setup
                let result = query.execute(self.db.as_executor()).await?;
                Ok(result.rows_affected())
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<u64, sqlx::Error> {
                let result = sqlx::query(#const_name)
                    #binds
                    .execute(self.db.as_executor())
                    .await?;
                Ok(result.rows_affected())
            }
        }
    };
    Ok((quote! { #params_struct #const_tokens }, inner))
}

/// `:execresult` → `async fn foo(exec, [params]) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error>`
pub fn gen_execresult(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;
    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);
    let inner = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query(&sql);
                #bind_setup
                query.execute(self.db.as_executor()).await
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
                sqlx::query(#const_name)
                    #binds
                    .execute(self.db.as_executor())
                    .await
            }
        }
    };
    Ok((quote! { #params_struct #const_tokens }, inner))
}

/// `:exec` → `async fn foo(exec, [params]) -> Result<(), sqlx::Error>`
pub fn gen_exec(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let sql = query.text;
    let (const_tokens, const_name) = sql_const(query.name, sql);

    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;

    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let inner_fn = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<(), sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query(&sql);
                #bind_setup
                query.execute(self.db.as_executor()).await?;
                Ok(())
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<(), sqlx::Error> {
                sqlx::query(#const_name)
                    #binds
                    .execute(self.db.as_executor())
                    .await?;
                Ok(())
            }
        }
    };

    let outer = quote! {
        #params_struct
        #const_tokens
    };

    Ok((outer, inner_fn))
}

/// `:execlastid` → `async fn foo(exec, [params]) -> Result<T, sqlx::Error>`
/// where T is the type of the single RETURNING column.
pub fn gen_execlastid(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let (params_struct, arg_ident, fn_params) =
        build_fn_params(query.name, &params, &config.row_derives)?;
    let binds = bind_calls(&params, arg_ident.as_ref());
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    // Resolve the single return column
    let cols = resolve_columns(query.columns.iter(), type_map, col_overrides)?;
    let (_, first_resolved) = cols
        .flat
        .first()
        .ok_or_else(|| Error::Codegen(":execlastid query has no result columns".into()))?;
    let ret_ty: syn::Type = parse_str(&first_resolved.rust_type).map_err(|e| {
        Error::Codegen(format!(
            "invalid return type '{}': {e}",
            first_resolved.rust_type
        ))
    })?;

    let inner = if dynamic_slice {
        let sql_setup = dynamic_sql_setup(&const_name, &params, arg_ident.as_ref());
        let bind_setup = dynamic_bind_statements(&params, arg_ident.as_ref());
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<#ret_ty, sqlx::Error> {
                #sql_setup
                let mut query = sqlx::query_as(&sql);
                #bind_setup
                let (_row,): (#ret_ty,) = query.fetch_one(self.db.as_executor()).await?;
                Ok(_row)
            }
        }
    } else {
        quote! {
            pub async fn #fn_name(&mut self, #fn_params) -> Result<#ret_ty, sqlx::Error> {
                let (_row,): (#ret_ty,) = sqlx::query_as(#const_name)
                    #binds
                    .fetch_one(self.db.as_executor())
                    .await?;
                Ok(_row)
            }
        }
    };
    Ok((quote! { #params_struct #const_tokens }, inner))
}
