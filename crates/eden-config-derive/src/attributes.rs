// Copied from: https://github.com/serde-rs/serde/blob/fa7da4a93567ed347ad0735c28e439fca688ef26/serde_derive/src/internals/attr.rs#L1411-L1474
pub mod meta {
    use syn::{Error, Result, meta::ParseNestedMeta};

    /// Parse a string literal attribute value into a type.
    pub fn parse_lit_into_type(meta: &ParseNestedMeta) -> Result<Option<syn::Type>> {
        let Some(str) = get_lit_str(meta)? else {
            return Ok(None);
        };

        str.parse().map(Some).map_err(|_| {
            syn::Error::new(
                str.span(),
                format!("failed to parse path: {:?}", str.value()),
            )
        })
    }

    /// Parse a string literal attribute value into a path.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[validate(with = "path::to::validator")]
    /// ```
    pub fn parse_lit_into_path(meta: &ParseNestedMeta) -> Result<Option<syn::Path>> {
        let Some(str) = get_lit_str(meta)? else {
            return Ok(None);
        };

        str.parse().map(Some).map_err(|_| {
            syn::Error::new(
                str.span(),
                format!("failed to parse path: {:?}", str.value()),
            )
        })
    }

    /// Extract a string literal from a meta attribute.
    pub fn get_lit_str(meta: &ParseNestedMeta) -> Result<Option<syn::LitStr>> {
        let expr: syn::Expr = meta.value()?.parse()?;

        let mut value = &expr;
        while let syn::Expr::Group(e) = value {
            value = &e.expr;
        }

        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit),
            ..
        }) = value
        else {
            return Ok(None);
        };

        let suffix = lit.suffix();
        if !suffix.is_empty() {
            return Err(Error::new(
                lit.span(),
                format!("unexpected suffix `{}` on string literal", suffix),
            ));
        }

        Ok(Some(lit.clone()))
    }
}
