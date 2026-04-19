//! Implementation of the `#[derive(Validate)]` macro.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Result};

use crate::attributes::meta::parse_lit_into_path;

mod strukt;

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    let body = match input.data {
        Data::Struct(ref data) => self::strukt::expand(data)?,
        Data::Enum(_data) => {
            return Err(syn::Error::new_spanned(
                input.ident,
                "Validate derive is not yet supported for enums",
            ));
        }
        Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span,
                "Validate derive is not supported for unions",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    Ok(quote! {
        impl #impl_generics crate::validation::Validate for #name #ty_generics #where_clause {
            #[allow(unused)]
            fn validate(
                &self,
                ctx: &crate::context::SourceContext<'_>
            ) -> ::core::result::Result<(), ::eden_file_diagnostics::RenderedDiagnostic> {
                #body
            }
        }
    })
}

/// Parsed field-level attributes for validation.
struct ParsedFieldAttrs {
    /// Custom validation function path
    with: Option<syn::Path>,
    /// Whether to skip validation for this field
    skip: bool,
}

fn parse_nested_meta(
    meta: syn::meta::ParseNestedMeta<'_>,
    with: &mut Option<syn::Path>,
    skip: &mut bool,
) -> Result<()> {
    let path = &meta.path;
    if path.is_ident("with") {
        let Some(value) = parse_lit_into_path(&meta)? else {
            return Err(meta.error("expected `with` attribute to be a string: `with = \"...\"`"));
        };
        *with = Some(value);
    } else if path.is_ident("skip") {
        *skip = true;
    } else {
        let path = path.to_token_stream().to_string().replace(' ', "");
        return Err(meta.error(format_args!("unknown Validate field attribute `{path}`")));
    }
    Ok(())
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<ParsedFieldAttrs> {
    let mut with = None;
    let mut skip = false;

    for attr in attrs {
        if !attr.path().is_ident("validate") {
            continue;
        }
        attr.parse_nested_meta(|meta| parse_nested_meta(meta, &mut with, &mut skip))?;
    }

    Ok(ParsedFieldAttrs { with, skip })
}
