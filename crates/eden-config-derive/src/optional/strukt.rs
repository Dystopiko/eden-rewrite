use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Fields, Result, spanned::Spanned};

use crate::attributes::meta::parse_lit_into_type;

pub fn expand(input: super::ParsedInput<'_>, data: &syn::DataStruct) -> Result<TokenStream> {
    if matches!(&data.fields, Fields::Unit) {
        return Err(syn::Error::new(
            data.fields.span(),
            "Optional cannot be derived for unit structs",
        ));
    }

    let has_named_fields = matches!(&data.fields, Fields::Named(..));
    let optional_ident = format_ident!("Optional{}", input.ident);

    let vis = input.set_vis.unwrap_or(input.original_vis.clone());
    let generics = &input.generics;
    let where_clause = generics.where_clause.as_ref();

    let mut field_tokens = TokenStream::new();
    for field in data.fields.iter() {
        let parsed = parse_field_attrs(&field.attrs)?;

        let field_ty: TokenStream = if let Some(with) = parsed.with {
            quote! { #with }
        } else {
            let original_ty = &field.ty;
            quote! { ::core::option::Option<#original_ty> }
        };

        let field_vis = &field.vis;
        if has_named_fields {
            let field_name = field.ident.as_ref().unwrap();
            field_tokens.extend(quote! {
                #field_vis #field_name: #field_ty,
            });
        } else {
            field_tokens.extend(quote! {
                #field_vis #field_ty,
            });
        }
    }

    let passed_attrs = &input.passed_attrs;
    let body = if has_named_fields {
        quote! {
            #[derive(Clone, Debug, Default)]
            #(#passed_attrs)*
            #vis struct #optional_ident #generics #where_clause {
                #field_tokens
            }
        }
    } else {
        quote! {
            #[derive(Clone, Debug, Default)]
            #(#passed_attrs)*
            #vis struct #optional_ident #generics (#field_tokens) #where_clause;
        }
    };

    Ok(body)
}

/// Parsed field-level attributes.
struct ParsedFieldAttrs {
    /// Custom overriden type, other name for `as`
    with: Option<syn::Type>,
}

fn parse_nested_meta(
    meta: syn::meta::ParseNestedMeta<'_>,
    with: &mut Option<syn::Type>,
) -> Result<()> {
    let path = &meta.path;
    if path.is_ident("as") {
        let Some(value) = parse_lit_into_type(&meta)? else {
            return Err(meta.error("expected `as` attribute to be a string: `as = \"...\"`"));
        };
        *with = Some(value);
    } else {
        let path = path.to_token_stream().to_string().replace(' ', "");
        return Err(meta.error(format_args!("unknown Optional field attribute `{path}`")));
    }
    Ok(())
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<ParsedFieldAttrs> {
    let mut with = None;

    for attr in attrs {
        if !attr.path().is_ident("optional") {
            continue;
        }
        attr.parse_nested_meta(|meta| parse_nested_meta(meta, &mut with))?;
    }

    Ok(ParsedFieldAttrs { with })
}
