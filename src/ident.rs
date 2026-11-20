use convert_case::{Case, Casing as _};

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Replace each character that is not alphanumeric or `_` with `_`.
#[must_use]
pub fn normalize_ident(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Convert a name to `snake_case`.
#[must_use]
pub fn to_snake_case(s: &str) -> String {
    normalize_ident(s).to_case(Case::Snake)
}

/// Convert a name to `PascalCase`.
#[must_use]
pub fn to_pascal_case(s: &str) -> String {
    normalize_ident(s).to_case(Case::Pascal)
}

/// Make a field name in `snake_case`, with `r#` for a keyword.
#[must_use]
pub fn field_ident(name: &str) -> proc_macro2::Ident {
    let snake = to_snake_case(name);
    if KEYWORDS.contains(&snake.as_str()) {
        quote::format_ident!("r#{}", snake)
    } else {
        quote::format_ident!("{}", snake)
    }
}

/// Make a type name in `PascalCase`.
#[must_use]
pub fn type_ident(name: &str) -> proc_macro2::Ident {
    quote::format_ident!("{}", to_pascal_case(name))
}

/// For enum variants: `PascalCase` with keyword escaping.
#[must_use]
pub fn variant_ident(name: &str) -> proc_macro2::Ident {
    let pascal = to_pascal_case(name);
    let snake = to_snake_case(&pascal);
    if KEYWORDS.contains(&pascal.as_str()) || KEYWORDS.contains(&snake.as_str()) {
        // Only use raw identifier if the pascal form itself can be raw-escaped.
        // Some keywords like `Self` cannot appear as raw identifiers, so we
        // append an underscore as a fallback.
        if ["Self", "true", "false"].contains(&pascal.as_str()) {
            quote::format_ident!("{}_", pascal)
        } else {
            quote::format_ident!("r#{}", pascal)
        }
    } else {
        quote::format_ident!("{}", pascal)
    }
}

/// Name of the params struct of a query.
#[must_use]
pub fn query_params_name(query_name: &str) -> String {
    format!("{}Params", to_pascal_case(query_name))
}

/// Name of the row struct of a query.
#[must_use]
pub fn query_row_name(query_name: &str) -> String {
    format!("{}Row", to_pascal_case(query_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_from_pascal() {
        assert_eq!(to_snake_case("GetAuthor"), "get_author");
    }
    #[test]
    fn snake_idempotent() {
        assert_eq!(to_snake_case("get_author"), "get_author");
    }
    #[test]
    fn pascal_from_snake() {
        assert_eq!(to_pascal_case("get_author"), "GetAuthor");
    }
    #[test]
    fn pascal_idempotent() {
        assert_eq!(to_pascal_case("GetAuthor"), "GetAuthor");
    }

    #[test]
    fn keyword_type_escaped() {
        let id = field_ident("type");
        assert_eq!(id.to_string(), "r#type");
    }
    #[test]
    fn keyword_for_escaped() {
        let id = field_ident("for");
        assert_eq!(id.to_string(), "r#for");
    }
    #[test]
    fn non_keyword_unchanged() {
        let id = field_ident("name");
        assert_eq!(id.to_string(), "name");
    }

    #[test]
    fn normalize_hyphens() {
        assert_eq!(normalize_ident("some-name"), "some_name");
    }
    #[test]
    fn normalize_colons() {
        assert_eq!(normalize_ident("some:name"), "some_name");
    }

    #[test]
    fn variant_ident_self_is_escaped() {
        let id = variant_ident("self");
        // "self" → PascalCase "Self" → keyword, but r#Self is invalid → Self_
        assert_eq!(id.to_string(), "Self_");
    }

    #[test]
    fn variant_ident_normal_value() {
        let id = variant_ident("active");
        assert_eq!(id.to_string(), "Active");
    }

    #[test]
    fn params_name_works() {
        assert_eq!(query_params_name("GetAuthor"), "GetAuthorParams");
    }
    #[test]
    fn row_name_works() {
        assert_eq!(query_row_name("GetAuthor"), "GetAuthorRow");
    }
}
