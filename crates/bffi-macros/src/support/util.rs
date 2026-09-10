//! Small parse helpers shared by the proc-macro crates.

/// Collects `///` doc-comment lines, trimming exactly one leading
/// space (`/// Adds.` becomes `Adds.`).
pub fn extract_docs(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr) = &meta.value else {
            continue;
        };
        let syn::Lit::Str(lit) = &expr.lit else {
            continue;
        };
        let mut doc = lit.value();
        if doc.starts_with(' ') {
            doc = doc[1..].to_owned();
        }
        docs.push(doc);
    }
    docs
}

/// Converts `Counter`/`HTTPServer`/`my_type` to
/// `counter`/`http_server`/`my_type`.
pub fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (index, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_uppercase() {
            let prev_lower = index > 0 && chars[index - 1].is_ascii_lowercase();
            let prev_underscore = index > 0 && chars[index - 1] == '_';
            let next_lower = chars.get(index + 1).is_some_and(|c| c.is_ascii_lowercase());
            // Insert an underscore at a hump (`aB`) or an acronym
            // boundary (`ABc`), never after an existing `_`.
            if index > 0 && !prev_underscore && (prev_lower || next_lower) {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{extract_docs, to_snake_case};

    #[test]
    fn snake_case_handles_humps_acronyms_and_underscores() {
        assert_eq!(to_snake_case("Counter"), "counter");
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("my_type"), "my_type");
        assert_eq!(to_snake_case("AB"), "ab");
    }

    #[test]
    fn extract_docs_trims_one_leading_space() {
        let attrs = vec![
            syn::parse_quote! {
                /// Adds two numbers.
            },
            syn::parse_quote! {
                /// And more.
            },
        ];
        assert_eq!(extract_docs(&attrs), ["Adds two numbers.", "And more."]);
    }

    #[test]
    fn extract_docs_ignores_non_doc_attributes() {
        let attrs = vec![
            syn::parse_quote! {
                #[inline]
            },
            syn::parse_quote! {
                /// Kept.
            },
        ];
        assert_eq!(extract_docs(&attrs), ["Kept."]);
    }
}
