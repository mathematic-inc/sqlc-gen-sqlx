use crate::{
    catalog, config::Config, emit::FileEmitter, error::Error, plugin::GenerateRequestView,
    types::TypeMap,
};

mod batch;
mod composites;
mod copyfrom;
mod enums;
mod query;

pub fn generate(request: &GenerateRequestView<'_>, config: &Config) -> Result<String, Error> {
    let mut type_map = TypeMap::new(&config.overrides, &config.copy_cheap_types);
    let catalog_info = catalog::walk(request, &mut type_map)?;
    let col_overrides = crate::types::build_column_overrides(&config.overrides);
    let mut emitter = FileEmitter::new(request.sqlc_version, env!("CARGO_PKG_VERSION"));

    // Emit type definitions before query code.
    for info in &catalog_info.enums {
        emitter.push(enums::gen_enum(info, &config.enum_derives)?);
    }
    for info in &catalog_info.composites {
        emitter.push(composites::gen_composite(info, &config.composite_derives)?);
    }

    let mut module_items: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut impl_fns: Vec<proc_macro2::TokenStream> = Vec::new();

    for q in request.queries.iter() {
        let (outer, inner) = match q.cmd {
            ":exec" => query::gen_exec(q, &type_map, config, &col_overrides)?,
            ":execrows" => query::gen_execrows(q, &type_map, config, &col_overrides)?,
            ":execresult" => query::gen_execresult(q, &type_map, config, &col_overrides)?,
            ":execlastid" => query::gen_execlastid(q, &type_map, config, &col_overrides)?,
            ":batchexec" => batch::gen_batchexec(q, &type_map, config, &col_overrides)?,
            ":batchone" => batch::gen_batchone(q, &type_map, config, &col_overrides)?,
            ":batchmany" => batch::gen_batchmany(q, &type_map, config, &col_overrides)?,
            ":copyfrom" => copyfrom::gen_copyfrom(q, &type_map, config, &col_overrides)?,
            ":one" => query::gen_one(q, &type_map, config, &col_overrides)?,
            ":many" => query::gen_many(q, &type_map, config, &col_overrides)?,
            cmd => {
                eprintln!("sqlc-gen-sqlx: skipping unsupported annotation {cmd}");
                continue;
            }
        };
        module_items.push(outer);
        impl_fns.push(inner);
    }

    for item in module_items {
        emitter.push(item);
    }

    emitter.push(quote::quote! {
        pub trait AsExecutor {
            type Exec<'c>: sqlx::Executor<'c, Database = sqlx::Postgres>
            where
                Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_>;
        }

        impl AsExecutor for sqlx::PgPool {
            type Exec<'c> = &'c sqlx::PgPool where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { &*self }
        }

        impl AsExecutor for &sqlx::PgPool {
            type Exec<'c> = &'c sqlx::PgPool where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { *self }
        }

        impl AsExecutor for sqlx::PgConnection {
            type Exec<'c> = &'c mut sqlx::PgConnection where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { self }
        }

        impl AsExecutor for sqlx::Transaction<'_, sqlx::Postgres> {
            type Exec<'c> = &'c mut sqlx::PgConnection where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { &mut **self }
        }

        impl AsExecutor for sqlx::pool::PoolConnection<sqlx::Postgres> {
            type Exec<'c> = &'c mut sqlx::PgConnection where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { &mut **self }
        }

        impl<T: AsExecutor + ?Sized> AsExecutor for &mut T {
            type Exec<'c> = T::Exec<'c> where Self: 'c;
            fn as_executor(&mut self) -> Self::Exec<'_> { (**self).as_executor() }
        }

        pub struct Queries<E> {
            db: E,
        }

        impl<E> Queries<E> {
            pub fn new(db: E) -> Self {
                Self { db }
            }
        }
    });

    if !impl_fns.is_empty() {
        emitter.push(quote::quote! {
            impl<E: AsExecutor> Queries<E> {
                #(#impl_fns)*
            }
        });
    }

    emitter.finish()
}
