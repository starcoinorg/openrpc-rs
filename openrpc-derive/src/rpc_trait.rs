use crate::attr::{AttributeKind, RpcMethodAttribute};
use crate::to_gen_schema::generate_schema_method;
use crate::to_gen_schema::{MethodRegistration, RpcMethod};
use quote::quote;
use syn::{
    fold::{self, Fold},
    parse_quote, Result,
};

const METADATA_TYPE: &str = "Metadata";

const OPENRPC_SCHEMA_MODE_PREFIX: &str = "openrpc_schema_";

struct RpcTrait {
    methods: Vec<RpcMethod>,
    has_metadata: bool,
}

impl<'a> Fold for RpcTrait {
    fn fold_trait_item_method(&mut self, method: syn::TraitItemMethod) -> syn::TraitItemMethod {
        let mut foldable_method = method.clone();
        // strip rpc attributes
        foldable_method.attrs.retain(|a| {
            let rpc_method = self.methods.iter().find(|m| m.trait_item == method);
            rpc_method.map_or(true, |rpc| rpc.attr.attr != *a)
        });
        fold::fold_trait_item_method(self, foldable_method)
    }

    fn fold_trait_item_type(&mut self, ty: syn::TraitItemType) -> syn::TraitItemType {
        if ty.ident == METADATA_TYPE {
            self.has_metadata = true;
            let mut ty = ty.clone();
            ty.bounds.push(parse_quote!(_jsonrpc_core::Metadata));
            return ty;
        }
        ty
    }
}

fn compute_method_registrations(item_trait: &syn::ItemTrait) -> Result<Vec<MethodRegistration>> {
    let methods_result: Result<Vec<_>> = item_trait
        .items
        .iter()
        .filter_map(|trait_item| {
            if let syn::TraitItem::Method(method) = trait_item {
                match RpcMethodAttribute::parse_attr(method) {
                    Ok(Some(attr)) => Some(Ok(RpcMethod::new(attr, method.clone()))),
                    Ok(None) => None, // non rpc annotated trait method
                    Err(err) => Some(Err(syn::Error::new_spanned(method, err))),
                }
            } else {
                None
            }
        })
        .collect();
    let methods = methods_result?;

    let mut method_registrations: Vec<MethodRegistration> = Vec::new();

    for method in methods.iter() {
        match &method.attr().kind {
            AttributeKind::Rpc { has_metadata, .. } => {
                method_registrations.push(MethodRegistration::Standard {
                    method: method.clone(),
                    has_metadata: *has_metadata,
                })
            }
        }
    }
    Ok(method_registrations)
}

fn rpc_wrapper_mod_name(rpc_trait: &syn::ItemTrait) -> syn::Ident {
    let name = rpc_trait.ident.clone();
    let mod_name = format!("{}{}", OPENRPC_SCHEMA_MODE_PREFIX, name.to_string());
    syn::Ident::new(&mod_name, proc_macro2::Span::call_site())
}

pub fn rpc_trait(input: syn::Item) -> Result<proc_macro2::TokenStream> {
    let rpc_trait = match input.clone() {
        syn::Item::Trait(item_trait) => item_trait,
        item => {
            return Err(syn::Error::new_spanned(
                item,
                "The #[rpc] custom attribute only works with trait declarations",
            ));
        }
    };
    let method_registrations = compute_method_registrations(&rpc_trait)?;
    let mod_name_ident = rpc_wrapper_mod_name(&rpc_trait);
    let generate_schema_method = generate_schema_method(&method_registrations)?;

    let jsonrpc_quote = quote!(
        use jsonrpc_derive::rpc;
        #[rpc]
        #input
    );
    let openrpc_quote = quote!(
        mod #mod_name_ident {
            use super::*;
            use openrpc_schema::document::*;
            #generate_schema_method

        }
        pub use self::#mod_name_ident::gen_schema;
    );
    Ok(if cfg!(feature = "jsonrpc") {
        quote!(#openrpc_quote #jsonrpc_quote)
    } else {
        quote!(#openrpc_quote)
    })
}
