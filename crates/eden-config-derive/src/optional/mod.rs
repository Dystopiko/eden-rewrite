//! Implementation of the `#[derive(Optional)]` macro.

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Data, DeriveInput, Result, parenthesized};

mod strukt;

pub(super) struct ParsedInput<'a> {
    generics: &'a syn::Generics,
    ident: &'a syn::Ident,
    original_vis: &'a syn::Visibility,
    passed_attrs: Vec<syn::Meta>,
    set_vis: Option<syn::Visibility>,
}

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    let parsed = parse_input(&input)?;
    let body = match input.data {
        Data::Struct(ref data) => self::strukt::expand(parsed, data)?,
        Data::Enum(_data) => {
            return Err(syn::Error::new_spanned(
                input.ident,
                "Optional derive is not yet supported for enums",
            ));
        }
        Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span,
                "Optional derive is not supported for unions",
            ));
        }
    };

    Ok(body)
}

pub(super) fn parse_input<'a>(input: &'a DeriveInput) -> Result<ParsedInput<'a>> {
    let mut passed_attrs = Vec::new();
    let mut set_vis = None;

    for attr in input.attrs.iter() {
        if !attr.path().is_ident("optional") {
            continue;
        }
        attr.parse_nested_meta(|meta| parse_nested_meta(meta, &mut passed_attrs, &mut set_vis))?;
    }

    Ok(ParsedInput {
        generics: &input.generics,
        ident: &input.ident,
        original_vis: &input.vis,
        passed_attrs,
        set_vis,
    })
}

fn parse_nested_meta(
    meta: syn::meta::ParseNestedMeta<'_>,
    passed_attrs: &mut Vec<syn::Meta>,
    set_visibility: &mut Option<syn::Visibility>,
) -> Result<()> {
    let path = &meta.path;
    if path.is_ident("attr") {
        let content;
        parenthesized!(content in meta.input);

        let metadata = content.parse::<syn::Meta>()?;
        passed_attrs.push(metadata);
    } else if path.is_ident("vis") {
        let value: syn::Visibility = meta.value()?.parse()?;
        *set_visibility = Some(value);
    } else {
        let path = path.to_token_stream().to_string().replace(' ', "");
        return Err(meta.error(format_args!("unknown Optional attribute `{path}`")));
    }
    Ok(())
}
