use crate::error::Error;

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub output: String,
    pub overrides: Vec<TypeOverride>,
    pub row_derives: Vec<String>,
    pub enum_derives: Vec<String>,
    pub composite_derives: Vec<String>,
    pub copy_cheap_types: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output: "queries.rs".to_string(),
            overrides: vec![],
            row_derives: vec![],
            enum_derives: vec![],
            composite_derives: vec![],
            copy_cheap_types: vec![],
        }
    }
}

impl Config {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let cfg: Config = serde_json::from_slice(bytes)?;
        for o in &cfg.overrides {
            let target = || {
                o.db_type
                    .as_deref()
                    .or(o.column.as_deref())
                    .unwrap_or("<unspecified>")
                    .to_string()
            };
            if o.rs_type.is_none() && o.borrowed_rs_type.is_none() {
                return Err(Error::Codegen(format!(
                    "override for '{}' must set at least one of 'rs_type' or 'borrowed_rs_type'",
                    target()
                )));
            }
            if let Some(borrowed) = &o.borrowed_rs_type {
                validate_borrowed_type(borrowed, &target())?;
            }
        }
        Ok(cfg)
    }
}

/// `borrowed_rs_type` must parse as a Rust type AND contain at least one
/// reference (`&T`). Without a reference there is nothing for the codegen's
/// lifetime injector to do — the field name is then misleading and the user
/// has likely written the wrong thing.
fn validate_borrowed_type(rs_type: &str, target: &str) -> Result<(), Error> {
    syn::parse_str::<syn::Type>(rs_type).map_err(|e| {
        Error::Codegen(format!(
            "override for '{target}': borrowed_rs_type '{rs_type}' is not a valid Rust type: {e}"
        ))
    })?;
    if !rs_type.contains('&') {
        return Err(Error::Codegen(format!(
            "override for '{target}': borrowed_rs_type '{rs_type}' must contain a reference \
             (e.g. '&str' or 'Option<&str>'); use 'rs_type' for owned types"
        )));
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub struct TypeOverride {
    pub db_type: Option<String>,
    pub column: Option<String>,
    /// Owned Rust type for rows and array contents. Optional when
    /// `borrowed_rs_type` is set; missing values fall back to the built-in
    /// default for the matched PG type.
    pub rs_type: Option<String>,
    /// Borrowed Rust type for scalar parameter positions. When present, the
    /// override participates in borrowed mode: parameter signatures use this
    /// type (with lifetime injection where the position requires a named
    /// lifetime), while rows and array contents continue to use the owned
    /// form.
    pub borrowed_rs_type: Option<String>,
    #[serde(default)]
    pub copy_cheap: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_json() {
        let c = Config::from_bytes(b"{}").unwrap();
        assert_eq!(c.output, "queries.rs");
        assert!(c.overrides.is_empty());
        assert!(c.row_derives.is_empty());
    }

    #[test]
    fn parses_output_name() {
        let c = Config::from_bytes(br#"{"output": "db.rs"}"#).unwrap();
        assert_eq!(c.output, "db.rs");
    }

    #[test]
    fn parses_type_override() {
        let json = br#"{"overrides":[{"db_type":"timestamptz","rs_type":"chrono::DateTime<chrono::Utc>"}]}"#;
        let c = Config::from_bytes(json).unwrap();
        assert_eq!(c.overrides.len(), 1);
        assert_eq!(c.overrides[0].db_type, Some("timestamptz".to_string()));
        assert_eq!(
            c.overrides[0].rs_type.as_deref(),
            Some("chrono::DateTime<chrono::Utc>")
        );
        assert!(c.overrides[0].borrowed_rs_type.is_none());
    }

    #[test]
    fn parses_column_override() {
        let json = br#"{"overrides":[{"column":"users.created_at","rs_type":"chrono::DateTime<chrono::Local>","copy_cheap":true}]}"#;
        let c = Config::from_bytes(json).unwrap();
        assert_eq!(c.overrides[0].column, Some("users.created_at".to_string()));
        assert!(c.overrides[0].copy_cheap);
    }

    #[test]
    fn parses_derives() {
        let json = br#"{"row_derives":["serde::Serialize"],"enum_derives":["serde::Serialize","serde::Deserialize"]}"#;
        let c = Config::from_bytes(json).unwrap();
        assert_eq!(c.row_derives, ["serde::Serialize"]);
        assert_eq!(c.enum_derives.len(), 2);
    }

    #[test]
    fn parses_borrowed_only_override() {
        let json = br#"{"overrides":[{"db_type":"text","borrowed_rs_type":"&str"}]}"#;
        let c = Config::from_bytes(json).unwrap();
        assert_eq!(c.overrides.len(), 1);
        assert!(c.overrides[0].rs_type.is_none());
        assert_eq!(c.overrides[0].borrowed_rs_type.as_deref(), Some("&str"));
    }

    #[test]
    fn parses_owned_and_borrowed_override() {
        let json =
            br#"{"overrides":[{"db_type":"text","rs_type":"MyStr","borrowed_rs_type":"&MyStr"}]}"#;
        let c = Config::from_bytes(json).unwrap();
        assert_eq!(c.overrides[0].rs_type.as_deref(), Some("MyStr"));
        assert_eq!(c.overrides[0].borrowed_rs_type.as_deref(), Some("&MyStr"));
    }

    #[test]
    fn rejects_non_borrowed_in_borrowed_rs_type() {
        let json = br#"{"overrides":[{"db_type":"text","borrowed_rs_type":"Option<String>"}]}"#;
        let err = Config::from_bytes(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("must contain a reference"),
            "expected reference-required error, got: {msg}"
        );
        assert!(msg.contains("text"), "expected target name in error: {msg}");
    }

    #[test]
    fn rejects_invalid_rust_in_borrowed_rs_type() {
        let json = br#"{"overrides":[{"db_type":"text","borrowed_rs_type":"&not a type!!"}]}"#;
        let err = Config::from_bytes(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("not a valid Rust type"),
            "expected parse error, got: {msg}"
        );
    }

    #[test]
    fn accepts_option_of_reference_in_borrowed_rs_type() {
        let json = br#"{"overrides":[{"db_type":"text","borrowed_rs_type":"Option<&str>"}]}"#;
        Config::from_bytes(json).expect("Option<&str> should be accepted as borrowed");
    }
}
