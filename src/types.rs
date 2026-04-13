use crate::config::TypeOverride;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ResolvedType {
    pub rust_type: String,
    pub copy_cheap: bool,
}

impl ResolvedType {
    fn new(rust_type: impl Into<String>, copy_cheap: bool) -> Self {
        Self {
            rust_type: rust_type.into(),
            copy_cheap,
        }
    }
}

pub struct TypeMap {
    defaults: HashMap<&'static str, (&'static str, bool)>,
    type_overrides: HashMap<String, ResolvedType>,
    custom_types: HashMap<String, ResolvedType>,
}

impl TypeMap {
    pub fn new(overrides: &[TypeOverride], copy_cheap_types: &[String]) -> Self {
        let mut defaults: HashMap<&'static str, (&'static str, bool)> = HashMap::new();

        // Boolean
        for n in ["bool", "boolean", "pg_catalog.bool"] {
            defaults.insert(n, ("bool", true));
        }

        // Integer types
        for n in [
            "int2",
            "smallint",
            "pg_catalog.int2",
            "smallserial",
            "serial2",
            "pg_catalog.serial2",
        ] {
            defaults.insert(n, ("i16", true));
        }
        for n in [
            "int4",
            "integer",
            "int",
            "pg_catalog.int4",
            "serial",
            "serial4",
            "pg_catalog.serial4",
        ] {
            defaults.insert(n, ("i32", true));
        }
        for n in [
            "int8",
            "bigint",
            "pg_catalog.int8",
            "bigserial",
            "serial8",
            "pg_catalog.serial8",
        ] {
            defaults.insert(n, ("i64", true));
        }

        // Float types
        for n in ["float4", "real", "pg_catalog.float4"] {
            defaults.insert(n, ("f32", true));
        }
        for n in ["float8", "float", "double precision", "pg_catalog.float8"] {
            defaults.insert(n, ("f64", true));
        }

        // Decimal / numeric types
        for n in ["numeric", "decimal", "pg_catalog.numeric"] {
            defaults.insert(n, ("bigdecimal::BigDecimal", false));
        }

        // String types
        for n in [
            "text",
            "varchar",
            "pg_catalog.varchar",
            "pg_catalog.bpchar",
            "bpchar",
            "string",
            "citext",
            "name",
            "pg_catalog.name",
        ] {
            defaults.insert(n, ("String", false));
        }

        // Bytes
        for n in ["bytea", "blob", "pg_catalog.bytea"] {
            defaults.insert(n, ("Vec<u8>", false));
        }

        // UUID
        defaults.insert("uuid", ("uuid::Uuid", true));

        // JSON
        for n in ["json", "jsonb"] {
            defaults.insert(n, ("serde_json::Value", false));
        }

        // Timestamps
        for n in [
            "timestamptz",
            "pg_catalog.timestamptz",
            "timestamp with time zone",
        ] {
            defaults.insert(n, ("chrono::DateTime<chrono::Utc>", false));
        }
        for n in [
            "timestamp",
            "pg_catalog.timestamp",
            "timestamp without time zone",
        ] {
            defaults.insert(n, ("chrono::NaiveDateTime", false));
        }
        defaults.insert("date", ("chrono::NaiveDate", true));
        for n in ["time", "pg_catalog.time", "time without time zone"] {
            defaults.insert(n, ("chrono::NaiveTime", false));
        }

        // Network
        for n in ["inet", "cidr"] {
            defaults.insert(n, ("ipnetwork::IpNetwork", false));
        }
        defaults.insert("macaddr", ("mac_address::MacAddress", true));

        // Misc
        defaults.insert(
            "hstore",
            ("std::collections::HashMap<String, Option<String>>", false),
        );
        for n in ["interval", "pg_catalog.interval"] {
            defaults.insert(n, ("sqlx::postgres::types::PgInterval", false));
        }
        defaults.insert("money", ("sqlx::postgres::types::PgMoney", true));
        defaults.insert("oid", ("sqlx::postgres::types::Oid", true));
        defaults.insert("pg_catalog.oid", ("sqlx::postgres::types::Oid", true));
        for n in ["ltree", "lquery"] {
            defaults.insert(n, ("String", false));
        }

        // Range types
        for n in ["int4range", "pg_catalog.int4range"] {
            defaults.insert(n, ("sqlx::postgres::types::PgRange<i32>", false));
        }
        for n in ["int8range", "pg_catalog.int8range"] {
            defaults.insert(n, ("sqlx::postgres::types::PgRange<i64>", false));
        }
        for n in ["numrange", "pg_catalog.numrange"] {
            defaults.insert(
                n,
                (
                    "sqlx::postgres::types::PgRange<bigdecimal::BigDecimal>",
                    false,
                ),
            );
        }
        for n in ["tsrange", "pg_catalog.tsrange"] {
            defaults.insert(
                n,
                (
                    "sqlx::postgres::types::PgRange<chrono::NaiveDateTime>",
                    false,
                ),
            );
        }
        for n in ["tstzrange", "pg_catalog.tstzrange"] {
            defaults.insert(
                n,
                (
                    "sqlx::postgres::types::PgRange<chrono::DateTime<chrono::Utc>>",
                    false,
                ),
            );
        }
        for n in ["daterange", "pg_catalog.daterange"] {
            defaults.insert(
                n,
                ("sqlx::postgres::types::PgRange<chrono::NaiveDate>", false),
            );
        }

        // Bit types
        for n in ["bit", "varbit", "pg_catalog.varbit"] {
            defaults.insert(n, ("bit_vec::BitVec", false));
        }

        let mut type_overrides = HashMap::new();
        for o in overrides {
            if let Some(db_type) = &o.db_type {
                type_overrides.insert(
                    db_type.to_lowercase(),
                    ResolvedType::new(o.rs_type.clone(), o.copy_cheap),
                );
            }
        }

        for name in copy_cheap_types {
            let key = name.to_lowercase();
            if let Some(ovr) = type_overrides.get_mut(&key) {
                ovr.copy_cheap = true;
            } else if let Some(&(ty, _)) = defaults.get(key.as_str()) {
                type_overrides.insert(key, ResolvedType::new(ty.to_string(), true));
            }
        }

        Self {
            defaults,
            type_overrides,
            custom_types: HashMap::new(),
        }
    }

    /// Register a custom type (enum or composite) discovered from the catalog.
    /// Registered types are checked after both `type_overrides` and `defaults`,
    /// so user-level overrides and built-in defaults always take precedence.
    pub fn register(&mut self, pg_name: &str, rust_name: &str, copy_cheap: bool) {
        self.custom_types.insert(
            pg_name.to_lowercase(),
            ResolvedType::new(rust_name.to_string(), copy_cheap),
        );
    }

    pub fn resolve_pg_type(
        &self,
        pg_type: &str,
        nullable: bool,
        is_array: bool,
    ) -> Option<ResolvedType> {
        self.resolve_pg_type_dims(pg_type, nullable, usize::from(is_array))
    }

    pub fn resolve_pg_type_dims(
        &self,
        pg_type: &str,
        nullable: bool,
        array_dims: usize,
    ) -> Option<ResolvedType> {
        let key = pg_type.to_lowercase();
        let (inner, copy_cheap) = if let Some(ovr) = self.type_overrides.get(&key) {
            (ovr.rust_type.clone(), ovr.copy_cheap)
        } else if let Some(&(ty, cc)) = self.defaults.get(key.as_str()) {
            (ty.to_string(), cc)
        } else if let Some(custom) = self.custom_types.get(&key) {
            (custom.rust_type.clone(), custom.copy_cheap)
        } else {
            return None;
        };

        let rust_type = wrap_type(&inner, nullable, array_dims);
        let effective_copy_cheap = copy_cheap && !nullable && array_dims == 0;
        Some(ResolvedType {
            rust_type,
            copy_cheap: effective_copy_cheap,
        })
    }

    pub fn resolve_column(
        &self,
        pg_type: &str,
        nullable: bool,
        is_array: bool,
        column_key: Option<&str>,
        column_overrides: &HashMap<String, ResolvedType>,
    ) -> Option<ResolvedType> {
        self.resolve_column_dims(
            pg_type,
            nullable,
            usize::from(is_array),
            column_key,
            column_overrides,
        )
    }

    pub fn resolve_column_dims(
        &self,
        pg_type: &str,
        nullable: bool,
        array_dims: usize,
        column_key: Option<&str>,
        column_overrides: &HashMap<String, ResolvedType>,
    ) -> Option<ResolvedType> {
        if let Some(key) = column_key
            && let Some(ovr) = column_overrides.get(key)
        {
            let rust_type = wrap_type(&ovr.rust_type, nullable, array_dims);
            let cc = ovr.copy_cheap && !nullable && array_dims == 0;
            return Some(ResolvedType {
                rust_type,
                copy_cheap: cc,
            });
        }
        self.resolve_pg_type_dims(pg_type, nullable, array_dims)
    }
}

fn wrap_type(inner: &str, nullable: bool, array_dims: usize) -> String {
    let mut t = inner.to_string();
    for _ in 0..array_dims {
        t = format!("Vec<{t}>");
    }
    if nullable { format!("Option<{t}>") } else { t }
}

pub fn build_column_overrides(overrides: &[TypeOverride]) -> HashMap<String, ResolvedType> {
    overrides
        .iter()
        .filter_map(|o| {
            o.column.as_ref().map(|col| {
                (
                    col.clone(),
                    ResolvedType::new(o.rs_type.clone(), o.copy_cheap),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> TypeMap {
        TypeMap::new(&[], &[])
    }

    #[test]
    fn maps_text() {
        let t = map().resolve_pg_type("text", false, false).unwrap();
        assert_eq!(t.rust_type, "String");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn maps_int4_copy_cheap() {
        let t = map().resolve_pg_type("int4", false, false).unwrap();
        assert_eq!(t.rust_type, "i32");
        assert!(t.copy_cheap);
    }
    #[test]
    fn maps_bool() {
        let t = map().resolve_pg_type("bool", false, false).unwrap();
        assert_eq!(t.rust_type, "bool");
        assert!(t.copy_cheap);
    }
    #[test]
    fn maps_timestamptz() {
        let t = map().resolve_pg_type("timestamptz", false, false).unwrap();
        assert_eq!(t.rust_type, "chrono::DateTime<chrono::Utc>");
    }
    #[test]
    fn maps_uuid() {
        let t = map().resolve_pg_type("uuid", false, false).unwrap();
        assert_eq!(t.rust_type, "uuid::Uuid");
        assert!(t.copy_cheap);
    }
    #[test]
    fn maps_jsonb() {
        let t = map().resolve_pg_type("jsonb", false, false).unwrap();
        assert_eq!(t.rust_type, "serde_json::Value");
    }
    #[test]
    fn nullable_wraps_option() {
        let t = map().resolve_pg_type("text", true, false).unwrap();
        assert_eq!(t.rust_type, "Option<String>");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn array_wraps_vec() {
        let t = map().resolve_pg_type("text", false, true).unwrap();
        assert_eq!(t.rust_type, "Vec<String>");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn nullable_array() {
        let t = map().resolve_pg_type("text", true, true).unwrap();
        assert_eq!(t.rust_type, "Option<Vec<String>>");
    }
    #[test]
    fn multidimensional_array_wraps_nested_vec() {
        let t = map().resolve_pg_type_dims("int8", false, 2).unwrap();
        assert_eq!(t.rust_type, "Vec<Vec<i64>>");
    }
    #[test]
    fn nullable_multidimensional_array_wraps_option_nested_vec() {
        let t = map().resolve_pg_type_dims("text", true, 3).unwrap();
        assert_eq!(t.rust_type, "Option<Vec<Vec<Vec<String>>>>");
    }
    #[test]
    fn type_override_replaces_default() {
        use crate::config::TypeOverride;
        let ovr = TypeOverride {
            db_type: Some("timestamptz".to_string()),
            column: None,
            rs_type: "time::OffsetDateTime".to_string(),
            copy_cheap: false,
        };
        let t = TypeMap::new(&[ovr], &[])
            .resolve_pg_type("timestamptz", false, false)
            .unwrap();
        assert_eq!(t.rust_type, "time::OffsetDateTime");
    }
    #[test]
    fn column_override_beats_type_override() {
        use crate::config::TypeOverride;
        let overrides = vec![
            TypeOverride {
                db_type: Some("text".to_string()),
                column: None,
                rs_type: "TypeLevel".to_string(),
                copy_cheap: false,
            },
            TypeOverride {
                db_type: None,
                column: Some("users.name".to_string()),
                rs_type: "ColumnLevel".to_string(),
                copy_cheap: false,
            },
        ];
        let col_ovrs = build_column_overrides(&overrides);
        let map = TypeMap::new(&overrides, &[]);
        let t = map
            .resolve_column("text", false, false, Some("users.name"), &col_ovrs)
            .unwrap();
        assert_eq!(t.rust_type, "ColumnLevel");
    }
    #[test]
    fn maps_numeric() {
        let t = map().resolve_pg_type("numeric", false, false).unwrap();
        assert_eq!(t.rust_type, "bigdecimal::BigDecimal");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn maps_decimal() {
        let t = map().resolve_pg_type("decimal", false, false).unwrap();
        assert_eq!(t.rust_type, "bigdecimal::BigDecimal");
    }
    #[test]
    fn maps_pg_catalog_numeric() {
        let t = map()
            .resolve_pg_type("pg_catalog.numeric", false, false)
            .unwrap();
        assert_eq!(t.rust_type, "bigdecimal::BigDecimal");
    }
    #[test]
    fn unknown_type_returns_none() {
        assert!(
            map()
                .resolve_pg_type("no_such_type", false, false)
                .is_none()
        );
    }
    #[test]
    fn registers_custom_type() {
        let mut map = TypeMap::new(&[], &[]);
        map.register("my_enum", "MyEnum", false);
        let t = map.resolve_pg_type("my_enum", false, false).unwrap();
        assert_eq!(t.rust_type, "MyEnum");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn registered_type_nullable() {
        let mut map = TypeMap::new(&[], &[]);
        map.register("my_enum", "MyEnum", false);
        let t = map.resolve_pg_type("my_enum", true, false).unwrap();
        assert_eq!(t.rust_type, "Option<MyEnum>");
        assert!(!t.copy_cheap);
    }
    #[test]
    fn type_override_beats_registered_custom() {
        use crate::config::TypeOverride;
        let ovr = TypeOverride {
            db_type: Some("my_enum".to_string()),
            column: None,
            rs_type: "Override".to_string(),
            copy_cheap: false,
        };
        let mut map = TypeMap::new(&[ovr], &[]);
        map.register("my_enum", "MyEnum", false);
        let t = map.resolve_pg_type("my_enum", false, false).unwrap();
        // type_overrides must win over custom_types
        assert_eq!(t.rust_type, "Override");
    }
    #[test]
    fn registered_copy_cheap_type_is_cheap() {
        let mut map = TypeMap::new(&[], &[]);
        map.register("my_value_type", "MyValueType", true);
        let t = map.resolve_pg_type("my_value_type", false, false).unwrap();
        assert_eq!(t.rust_type, "MyValueType");
        assert!(
            t.copy_cheap,
            "registered type with copy_cheap=true should resolve as copy_cheap"
        );
    }

    #[test]
    fn registered_copy_cheap_nullable_is_not_cheap() {
        let mut map = TypeMap::new(&[], &[]);
        map.register("my_value_type", "MyValueType", true);
        let t = map.resolve_pg_type("my_value_type", true, false).unwrap();
        assert_eq!(t.rust_type, "Option<MyValueType>");
        assert!(
            !t.copy_cheap,
            "nullable type should not be copy_cheap even if base is"
        );
    }

    #[test]
    fn registered_copy_cheap_array_is_not_cheap() {
        let mut map = TypeMap::new(&[], &[]);
        map.register("my_value_type", "MyValueType", true);
        let t = map.resolve_pg_type("my_value_type", false, true).unwrap();
        assert_eq!(t.rust_type, "Vec<MyValueType>");
        assert!(
            !t.copy_cheap,
            "array type should not be copy_cheap even if base is"
        );
    }

    #[test]
    fn copy_cheap_types_marks_type_as_cheap() {
        let map = TypeMap::new(&[], &["text".to_string()]);
        // text is normally not copy_cheap (false in defaults)
        let t = map.resolve_pg_type("text", false, false).unwrap();
        assert_eq!(t.rust_type, "String");
        assert!(
            t.copy_cheap,
            "text should be copy_cheap after config promotion"
        );
    }

    #[test]
    fn copy_cheap_types_promotes_default_to_override() {
        // uuid is already copy_cheap in defaults; listing it in copy_cheap_types is a no-op behavior-wise
        let map = TypeMap::new(&[], &["uuid".to_string()]);
        let t = map.resolve_pg_type("uuid", false, false).unwrap();
        assert!(t.copy_cheap);
    }
}
