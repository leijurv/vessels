#![recursion_limit = "512"]

extern crate proc_macro;

#[macro_use]
extern crate synstructure;

use crate::proc_macro::TokenStream;

mod protocol;
mod value_derive;

use proc_macro2::Span;
use syn::Ident;

pub(crate) fn prefix<'a>(ident: &Ident, name: &'a str) -> Ident {
    Ident::new(
        &format!("_{}_PROTOCOL_IMPLEMENTATION_{}", ident, name),
        Span::call_site(),
    )
}

#[proc_macro_attribute]
pub fn protocol(attr: TokenStream, item: TokenStream) -> TokenStream {
    protocol::protocol(attr, item)
}

decl_derive!([Value] => value_derive::value_derive);
