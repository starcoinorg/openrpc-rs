use crate::attr::AttributeKind;
use quote::quote;
use crate::attr::RpcMethodAttribute;
use syn::{
    Result,
    punctuated::Punctuated,
};

pub fn generate_schema_method(
    methods: &[MethodRegistration],
) -> Result<syn::ImplItem> {
    let mut schema_methods = vec![];
    for method in methods {
        match method {
            MethodRegistration::Standard { method, .. } => {
                //TODO: Add rpc doc
                let _attrs = get_doc_comments(&method.trait_item.attrs);

                let rpc_name = method.name();
                let args = compute_args(&method.trait_item);
                let arg_names = compute_arg_identifiers(&args)?;
                let returns = match &method.attr.kind {
                    AttributeKind::Rpc { returns, .. } => {
                        compute_returns(&method.trait_item, returns)?
                    }
                };
                let args_types = compute_arg_type(&args)?;
                let arg_schemas = quote! {  {
                        let mut arg_schemas = vec![];
                        #(arg_schemas.push(
                        ContentDescriptorOrReference::new_content_descriptor::<#args_types>(
                            stringify!(#arg_names).to_string(),
                            None,
                        )
                        ));*;
                        arg_schemas
                    }
                            };
                let schema_method = quote! {{
                        let mut method_object = MethodObject::new(#rpc_name.to_string(), None);
                        let returns = ContentDescriptorOrReference::new_content_descriptor::<#returns>(
                        stringify!(#returns).to_string(),
                        None,
                        );
                        method_object.result = returns;
                                method_object.params = #arg_schemas;
                        method_object
                    }};
                schema_methods.push(schema_method);
            }
        }
    }

    let generate_schema_method = syn::parse_quote! {
        pub fn gen_schema() -> OpenrpcDocument {
            let mut document = OpenrpcDocument::default();
            let args_tuple = [#(#schema_methods,)*];
            for a in args_tuple.to_vec(){
            document.add_object_method(a);
            }
            document
        }};
    
    Ok(generate_schema_method)
}

fn get_doc_comments(attrs: &[syn::Attribute]) -> Vec<syn::Attribute> {
    let mut doc_comments = vec![];
    for attr in attrs {
        match attr {
            syn::Attribute {
                path: syn::Path { segments, .. },
                ..
            } => match &segments[0] {
                syn::PathSegment { ident, .. } => {
                    if ident.to_string() == "doc" {
                        doc_comments.push(attr.to_owned());
                    }
                }
            },
        }
    }
    doc_comments
}

fn compute_args(method: &syn::TraitItemMethod) -> Punctuated<syn::FnArg, syn::token::Comma> {
    let mut args = Punctuated::new();
    for arg in &method.sig.inputs {
        let ty = match arg {
            syn::FnArg::Typed(syn::PatType { ty, .. }) => ty,
            _ => continue,
        };
        let segments = match &**ty {
            syn::Type::Path(syn::TypePath {
                                path: syn::Path { ref segments, .. },
                                ..
                            }) => segments,
            _ => continue,
        };
        let ident = match &segments[0] {
            syn::PathSegment { ident, .. } => ident,
        };
        if ident.to_string() == "Self" {
            continue;
        }
        args.push(arg.to_owned());
    }
    args
}

fn compute_arg_type(args: &Punctuated<syn::FnArg, syn::token::Comma>) -> Result<Vec<syn::Type>> {
    let mut types = vec![];
    for arg in args {
        let ty = match arg {
            syn::FnArg::Typed(syn::PatType { ty, .. }) => ty,
            _ => continue,
        };

        types.push(ty.as_ref().clone());
    }
    Ok(types)
}

fn compute_arg_identifiers(
    args: &Punctuated<syn::FnArg, syn::token::Comma>,
) -> Result<Vec<&syn::Ident>> {
    let mut arg_names = vec![];
    for arg in args {
        let pat = match arg {
            syn::FnArg::Typed(syn::PatType { pat, .. }) => pat,
            _ => continue,
        };
        let ident = match **pat {
            syn::Pat::Ident(syn::PatIdent { ref ident, .. }) => ident,
            syn::Pat::Wild(ref wild) => {
                let span = wild.underscore_token.spans[0];
                let msg = "No wildcard patterns allowed in rpc trait.";
                return Err(syn::Error::new(span, msg));
            }
            _ => continue,
        };
        arg_names.push(ident);
    }
    Ok(arg_names)
}

fn compute_returns(method: &syn::TraitItemMethod, returns: &Option<String>) -> Result<syn::Type> {
    let returns: Option<syn::Type> = match returns {
        Some(returns) => Some(syn::parse_str(returns)?),
        None => None,
    };
    let returns = match returns {
        None => try_infer_returns(&method.sig.output),
        _ => returns,
    };
    let returns = match returns {
        Some(returns) => returns,
        None => {
            let span = method.attrs[0].pound_token.spans[0];
            let msg = "Missing returns attribute.";
            return Err(syn::Error::new(span, msg));
        }
    };
    Ok(returns)
}

fn try_infer_returns(output: &syn::ReturnType) -> Option<syn::Type> {
    let extract_path_segments = |ty: &syn::Type| match ty {
        syn::Type::Path(syn::TypePath {
                            path: syn::Path { segments, .. },
                            ..
                        }) => Some(segments.clone()),
        _ => None,
    };

    match output {
        syn::ReturnType::Type(_, ty) => {
            let segments = extract_path_segments(&**ty)?;
            let check_segment = |seg: &syn::PathSegment| match seg {
                syn::PathSegment {
                    ident, arguments, ..
                } => {
                    let id = ident.to_string();
                    let inner = get_first_type_argument(arguments);
                    if id.ends_with("Result") {
                        Ok(inner)
                    } else {
                        Err(inner)
                    }
                }
            };
            // Try out first argument (Result<X>) or nested types like:
            // BoxFuture<Result<X>>
            match check_segment(&segments[0]) {
                Ok(returns) => Some(returns?),
                Err(inner) => {
                    let segments = extract_path_segments(&inner?)?;
                    check_segment(&segments[0]).ok().flatten()
                }
            }
        }
        _ => None,
    }
}

fn get_first_type_argument(args: &syn::PathArguments) -> Option<syn::Type> {
    match args {
        syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                               args, ..
                                           }) => {
            if !args.is_empty() {
                match &args[0] {
                    syn::GenericArgument::Type(ty) => Some(ty.clone()),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

pub enum MethodRegistration {
    Standard {
        method: RpcMethod,
        has_metadata: bool,
    },
}

#[derive(Clone)]
pub struct RpcMethod {
    pub attr: RpcMethodAttribute,
    pub trait_item: syn::TraitItemMethod,
}

impl RpcMethod {
    pub fn new(attr: RpcMethodAttribute, trait_item: syn::TraitItemMethod) -> RpcMethod {
        RpcMethod { attr, trait_item }
    }

    pub fn attr(&self) -> &RpcMethodAttribute {
        &self.attr
    }

    pub fn name(&self) -> &str {
        &self.attr.name
    }
}
