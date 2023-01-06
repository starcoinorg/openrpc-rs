extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod attr;
mod params;
mod rpc_trait;
mod to_gen_schema;
#[proc_macro_attribute]
pub fn openrpc(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_tokens = parse_macro_input!(input as syn::Item);
    let output: TokenStream = match rpc_trait::rpc_trait(input_tokens) {
        Ok(output) => output.into(),
        Err(err) => err.to_compile_error().into(),
    };
    output
}
