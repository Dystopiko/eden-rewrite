use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Result, spanned::Spanned};

use super::parse_field_attrs;

pub fn expand(data: &syn::DataStruct) -> Result<TokenStream> {
    if data.fields.is_empty() {
        return Ok(quote! { Ok(()) });
    }

    let mut body = TokenStream::new();

    for (no, field) in data.fields.iter().enumerate() {
        let attrs = parse_field_attrs(&field.attrs)?;
        if attrs.skip {
            continue;
        }

        // Generate field accessor - either by name or by index (for tuple structs)
        let accessor = field
            .ident
            .clone()
            .unwrap_or_else(|| Ident::new(&no.to_string(), field.span()));

        if let Some(with) = attrs.with {
            // Use custom validator
            body.extend(quote! {
                #with ( &self.#accessor, ctx )?;
            });
            continue;
        }

        // Use default validation
        body.extend(quote! {
            self.#accessor.validate(ctx)?;
        });
    }

    body.extend(quote! { Ok(()) });
    Ok(body)
}
