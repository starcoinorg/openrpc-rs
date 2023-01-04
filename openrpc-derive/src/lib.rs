extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod attr;
mod params;
mod rpc_trait;
mod to_gen_schema;

#[proc_macro_attribute]
pub fn openrpc_schema(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_toks = parse_macro_input!(input as syn::Item);
    match rpc_trait::rpc_impl(input_toks) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
