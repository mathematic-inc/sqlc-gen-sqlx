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
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct TypeOverride {
    pub db_type: Option<String>,
    pub column: Option<String>,
    pub rs_type: String,
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
        assert_eq!(c.overrides[0].rs_type, "chrono::DateTime<chrono::Utc>");
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
}
