use crate::error::Error;
use syn::visit_mut::VisitMut;

/// Parse a borrowed Rust type string (e.g. `&str`, `Option<&str>`,
/// `&[String]`, `Option<&[i64]>`) and replace every anonymous or `'_`
/// reference lifetime with the named lifetime `lifetime`.
///
/// Used when emitting borrowed types into positions that cannot rely on
/// elision: struct field types and `where Item = ...` clauses.
pub fn inject_lifetime(rust_type: &str, lifetime: &str) -> Result<String, Error> {
    let mut ty: syn::Type = syn::parse_str(rust_type)
        .map_err(|e| Error::Codegen(format!("invalid Rust type '{rust_type}': {e}")))?;
    let mut visitor = LifetimeInjector {
        lifetime: syn::Lifetime::new(lifetime, proc_macro2::Span::call_site()),
    };
    visitor.visit_type_mut(&mut ty);
    Ok(quote::quote!(#ty).to_string())
}

struct LifetimeInjector {
    lifetime: syn::Lifetime,
}

impl VisitMut for LifetimeInjector {
    fn visit_type_reference_mut(&mut self, node: &mut syn::TypeReference) {
        let needs_inject = match &node.lifetime {
            None => true,
            Some(lt) => lt.ident == "_",
        };
        if needs_inject {
            node.lifetime = Some(self.lifetime.clone());
        }
        syn::visit_mut::visit_type_reference_mut(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inject(s: &str) -> String {
        inject_lifetime(s, "'a")
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn injects_into_bare_reference() {
        assert_eq!(inject("&str"), "& 'a str");
    }

    #[test]
    fn injects_into_anonymous_lifetime() {
        assert_eq!(inject("&'_ str"), "& 'a str");
    }

    #[test]
    fn injects_into_slice() {
        assert_eq!(inject("&[String]"), "& 'a [String]");
    }

    #[test]
    fn injects_inside_option() {
        assert_eq!(inject("Option<&str>"), "Option < & 'a str >");
    }

    #[test]
    fn injects_inside_option_slice() {
        assert_eq!(inject("Option<&[i64]>"), "Option < & 'a [i64] >");
    }

    #[test]
    fn leaves_named_lifetime_alone() {
        assert_eq!(inject("&'b str"), "& 'b str");
    }

    #[test]
    fn injects_inside_nested_borrowed_slice() {
        assert_eq!(inject("&[&str]"), "& 'a [& 'a str]");
    }
}
