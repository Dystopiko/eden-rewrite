use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attributes;
mod validate;

#[proc_macro_derive(Validate, attributes(validate))]
pub fn expand_validate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    into_proc_macro!(self::validate::expand(input))
}

macro_rules! into_proc_macro {
    ($expr:expr) => {
        match $expr {
            Ok(inner) => inner,
            Err(error) => error.into_compile_error(),
        }
        .into()
    };
}
use into_proc_macro;
