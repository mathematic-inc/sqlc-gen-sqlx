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

use super::query::{
    bind_calls, dynamic_bind_statements, dynamic_sql_setup, has_dynamic_slice, maybe_params_struct,
    resolve_columns, resolve_params, row_struct, sql_const,
};

fn batch_items_type(
    query_name: &str,
    params: &[super::query::Param],
    derives: &[String],
) -> Result<(Option<TokenStream>, TokenStream), Error> {
    if params.len() >= 2 {
        let (struct_tokens, struct_ident) = maybe_params_struct(query_name, params, derives)?
            .expect("guarded by params.len() >= 2");
        Ok((Some(struct_tokens), quote! { #struct_ident }))
    } else {
        let p = &params[0];
        let ty: syn::Type = parse_str(&p.resolved.rust_type).map_err(|e| {
            Error::Codegen(format!("invalid Rust type '{}': {e}", p.resolved.rust_type))
        })?;
        Ok((None, quote! { #ty }))
    }
}

fn single_item_alias(params: &[super::query::Param]) -> Option<TokenStream> {
    (params.len() == 1).then(|| {
        let ident = params[0].ident.clone();
        quote! { let #ident = item; }
    })
}

fn batch_stream_method(
    fn_name: &proc_macro2::Ident,
    items_ty: &TokenStream,
    result_ty: &TokenStream,
    next_body: TokenStream,
) -> TokenStream {
    quote! {
        pub fn #fn_name<'a, I>(
            &'a mut self,
            items: I,
        ) -> impl futures_core::stream::Stream<Item = Result<#result_ty, sqlx::Error>> + 'a
        where
            I: IntoIterator<Item = #items_ty> + 'a,
            I::IntoIter: 'a,
            E: 'a,
        {
            futures_util::stream::try_unfold(
                (&mut self.db, items.into_iter()),
                |(db, mut items)| async move {
                    let Some(item) = items.next() else {
                        return Ok(None);
                    };

                    #next_body
                },
            )
        }
    }
}

/// `:batchexec` → `fn foo(items) -> impl Stream<Item = Result<(), Error>>`
pub fn gen_batchexec(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    if params.is_empty() {
        return Err(Error::Codegen(format!(
            ":batchexec query '{}' has no parameters; batch over empty params is not meaningful",
            query.name
        )));
    }

    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let (params_struct, items_ty) = batch_items_type(query.name, &params, &config.row_derives)?;
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let next_body = if dynamic_slice {
        let item_arg = (params.len() >= 2).then(|| format_ident!("item"));
        let item_alias = single_item_alias(&params);
        let sql_setup = dynamic_sql_setup(&const_name, &params, item_arg.as_ref());
        let bind_setup = dynamic_bind_statements(&params, item_arg.as_ref());
        quote! {
            #item_alias
            #sql_setup
            let mut query = sqlx::query(&sql);
            #bind_setup
            query.execute(db.as_executor()).await?;
            Ok(Some(((), (db, items))))
        }
    } else if params.len() >= 2 {
        let item_ident = format_ident!("item");
        let binds = bind_calls(&params, Some(&item_ident));
        quote! {
            sqlx::query(#const_name)
                #binds
                .execute(db.as_executor())
                .await?;
            Ok(Some(((), (db, items))))
        }
    } else {
        quote! {
            sqlx::query(#const_name)
                .bind(item)
                .execute(db.as_executor())
                .await?;
            Ok(Some(((), (db, items))))
        }
    };

    let method = batch_stream_method(&fn_name, &items_ty, &quote! { () }, next_body);
    Ok((quote! { #params_struct #const_tokens }, method))
}

/// `:batchone` → `fn foo(items) -> impl Stream<Item = Result<Row, Error>>`
pub fn gen_batchone(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    if params.is_empty() {
        return Err(Error::Codegen(format!(
            ":batchone query '{}' has no parameters",
            query.name
        )));
    }

    let columns = resolve_columns(query.columns.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let row_name = crate::ident::type_ident(&crate::ident::query_row_name(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let row_tokens = row_struct(query.name, &columns, &config.row_derives)?;
    let (params_struct, items_ty) = batch_items_type(query.name, &params, &config.row_derives)?;
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let next_body = if dynamic_slice {
        let item_arg = (params.len() >= 2).then(|| format_ident!("item"));
        let item_alias = single_item_alias(&params);
        let sql_setup = dynamic_sql_setup(&const_name, &params, item_arg.as_ref());
        let bind_setup = dynamic_bind_statements(&params, item_arg.as_ref());
        quote! {
            #item_alias
            #sql_setup
            let mut query = sqlx::query_as::<_, #row_name>(&sql);
            #bind_setup
            let row = query.fetch_one(db.as_executor()).await?;
            Ok(Some((row, (db, items))))
        }
    } else if params.len() >= 2 {
        let item_ident = format_ident!("item");
        let binds = bind_calls(&params, Some(&item_ident));
        quote! {
            let row = sqlx::query_as::<_, #row_name>(#const_name)
                #binds
                .fetch_one(db.as_executor())
                .await?;
            Ok(Some((row, (db, items))))
        }
    } else {
        quote! {
            let row = sqlx::query_as::<_, #row_name>(#const_name)
                .bind(item)
                .fetch_one(db.as_executor())
                .await?;
            Ok(Some((row, (db, items))))
        }
    };

    let method = batch_stream_method(&fn_name, &items_ty, &quote! { #row_name }, next_body);
    Ok((quote! { #params_struct #const_tokens #row_tokens }, method))
}

/// `:batchmany` → `fn foo(items) -> impl Stream<Item = Result<Vec<Row>, Error>>`
pub fn gen_batchmany(
    query: &QueryView<'_>,
    type_map: &TypeMap,
    config: &Config,
    col_overrides: &std::collections::HashMap<String, ResolvedType>,
) -> Result<(TokenStream, TokenStream), Error> {
    let params = resolve_params(query.params.iter(), type_map, col_overrides)?;
    if params.is_empty() {
        return Err(Error::Codegen(format!(
            ":batchmany query '{}' has no parameters",
            query.name
        )));
    }

    let columns = resolve_columns(query.columns.iter(), type_map, col_overrides)?;
    let fn_name = format_ident!("{}", to_snake_case(query.name));
    let row_name = crate::ident::type_ident(&crate::ident::query_row_name(query.name));
    let (const_tokens, const_name) = sql_const(query.name, query.text);
    let row_tokens = row_struct(query.name, &columns, &config.row_derives)?;
    let (params_struct, items_ty) = batch_items_type(query.name, &params, &config.row_derives)?;
    let dynamic_slice = has_dynamic_slice(query.text, &params);

    let next_body = if dynamic_slice {
        let item_arg = (params.len() >= 2).then(|| format_ident!("item"));
        let item_alias = single_item_alias(&params);
        let sql_setup = dynamic_sql_setup(&const_name, &params, item_arg.as_ref());
        let bind_setup = dynamic_bind_statements(&params, item_arg.as_ref());
        quote! {
            #item_alias
            #sql_setup
            let mut query = sqlx::query_as::<_, #row_name>(&sql);
            #bind_setup
            let rows = query.fetch_all(db.as_executor()).await?;
            Ok(Some((rows, (db, items))))
        }
    } else if params.len() >= 2 {
        let item_ident = format_ident!("item");
        let binds = bind_calls(&params, Some(&item_ident));
        quote! {
            let rows = sqlx::query_as::<_, #row_name>(#const_name)
                #binds
                .fetch_all(db.as_executor())
                .await?;
            Ok(Some((rows, (db, items))))
        }
    } else {
        quote! {
            let rows = sqlx::query_as::<_, #row_name>(#const_name)
                .bind(item)
                .fetch_all(db.as_executor())
                .await?;
            Ok(Some((rows, (db, items))))
        }
    };

    let method = batch_stream_method(&fn_name, &items_ty, &quote! { Vec<#row_name> }, next_body);
    Ok((quote! { #params_struct #const_tokens #row_tokens }, method))
}
