// src/catalog.rs
use crate::{
    error::Error,
    ident::{field_ident, to_pascal_case},
    plugin::GenerateRequestView,
    types::TypeMap,
};

pub struct EnumInfo {
    pub schema: String,
    pub pg_name: String,
    pub rust_name: String,
    /// Used for `#[sqlx(type_name = "...")]`: "status" in public schema,
    /// "myschema.status" in non-default schemas.
    pub type_name: String,
    /// Original PG variant values (pre-rename).
    pub vals: Vec<String>,
}

pub struct CompositeField {
    pub pg_name: String,
    pub rust_ident: proc_macro2::Ident,
    /// Always `Option<T>` — composite fields are always nullable on the wire.
    pub rust_type: String,
}

pub struct CompositeInfo {
    pub schema: String,
    pub pg_name: String,
    pub rust_name: String,
    pub type_name: String,
    pub fields: Vec<CompositeField>,
}

pub struct CatalogInfo {
    pub enums: Vec<EnumInfo>,
    pub composites: Vec<CompositeInfo>,
}

/// Walk the catalog, register discovered custom types into `type_map`, return info for codegen.
pub fn walk(
    request: &GenerateRequestView<'_>,
    type_map: &mut TypeMap,
) -> Result<CatalogInfo, Error> {
    let mut enums = Vec::new();
    let mut composites = Vec::new();

    let catalog = match request.catalog.as_option() {
        Some(c) => c,
        None => return Ok(CatalogInfo { enums, composites }),
    };

    let default_schema = catalog.default_schema;

    // Phase 1: enums — must be registered before composite fields can reference them.
    for schema in catalog.schemas.iter() {
        for e in schema.enums.iter() {
            let rust_name = make_rust_name(schema.name, e.name, default_schema);
            let type_name = sqlx_type_name(schema.name, e.name, default_schema);
            // EnumView.vals is RepeatedView<'a, &'a str> — each item is &str.
            let vals: Vec<String> = e.vals.iter().map(|v| v.to_string()).collect();
            let pg_key = if schema.name == default_schema || schema.name.is_empty() {
                e.name.to_string()
            } else {
                format!("{}.{}", schema.name, e.name)
            };
            type_map.register(&pg_key, &rust_name, false);
            enums.push(EnumInfo {
                schema: schema.name.to_string(),
                pg_name: e.name.to_string(),
                rust_name,
                type_name,
                vals,
            });
        }
    }

    // Phase 2: composites — uses type_map which now includes enum types.
    // ASSUMPTION: composite fields are exposed via schema.tables, where
    // table.rel.name == composite_type.name. If sqlc does not populate tables
    // for composite types, Task 7 will catch this and the walk logic must be
    // revised.
    for schema in catalog.schemas.iter() {
        let composite_names: std::collections::HashSet<&str> =
            schema.composite_types.iter().map(|c| c.name).collect();
        if composite_names.is_empty() {
            continue;
        }

        for table in schema.tables.iter() {
            let rel = match table.rel.as_option() {
                Some(r) => r,
                None => continue,
            };
            if !composite_names.contains(rel.name) {
                continue;
            }

            let rust_name = make_rust_name(schema.name, rel.name, default_schema);
            let type_name = sqlx_type_name(schema.name, rel.name, default_schema);

            let mut fields = Vec::new();
            for col in table.columns.iter() {
                let pg_type = col.r#type.as_option().map(|t| t.name).unwrap_or("");
                let array_dims = if col.array_dims > 0 {
                    col.array_dims as usize
                } else {
                    usize::from(col.is_array)
                };
                // Intentionally force nullable=true: sqlx deserializes composite
                // type fields as Option<T> regardless of the NOT NULL constraint.
                let rust_type = type_map
                    .resolve_pg_type_dims(pg_type, true, array_dims)
                    .ok_or_else(|| {
                        Error::Codegen(format!(
                            "composite '{}.{}' field '{}' has unknown type '{pg_type}'",
                            schema.name, rel.name, col.name
                        ))
                    })?;
                fields.push(CompositeField {
                    pg_name: col.name.to_string(),
                    rust_ident: field_ident(col.name),
                    rust_type: rust_type.rust_type,
                });
            }

            let pg_key = if schema.name == default_schema || schema.name.is_empty() {
                rel.name.to_string()
            } else {
                format!("{}.{}", schema.name, rel.name)
            };
            type_map.register(&pg_key, &rust_name, false);
            composites.push(CompositeInfo {
                schema: schema.name.to_string(),
                pg_name: rel.name.to_string(),
                rust_name,
                type_name,
                fields,
            });
        }
    }

    Ok(CatalogInfo { enums, composites })
}

fn make_rust_name(schema: &str, name: &str, default_schema: &str) -> String {
    if schema == default_schema || schema.is_empty() {
        to_pascal_case(name)
    } else {
        format!("{}{}", to_pascal_case(schema), to_pascal_case(name))
    }
}

fn sqlx_type_name(schema: &str, name: &str, default_schema: &str) -> String {
    if schema == default_schema || schema.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", schema, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_rust_name_public_schema() {
        assert_eq!(make_rust_name("public", "status", "public"), "Status");
    }

    #[test]
    fn make_rust_name_non_default_schema() {
        assert_eq!(
            make_rust_name("myschema", "status", "public"),
            "MyschemaStatus"
        );
    }

    #[test]
    fn sqlx_type_name_public_schema() {
        assert_eq!(sqlx_type_name("public", "status", "public"), "status");
    }

    #[test]
    fn sqlx_type_name_non_default_schema() {
        assert_eq!(
            sqlx_type_name("myschema", "status", "public"),
            "myschema.status"
        );
    }

    #[test]
    fn make_rust_name_empty_schema() {
        assert_eq!(make_rust_name("", "status", "public"), "Status");
    }

    #[test]
    fn sqlx_type_name_empty_schema() {
        assert_eq!(sqlx_type_name("", "status", "public"), "status");
    }

    #[test]
    fn make_rust_name_underscored_schema() {
        assert_eq!(
            make_rust_name("my_app_schema", "order_status", "public"),
            "MyAppSchemaOrderStatus"
        );
    }
}
