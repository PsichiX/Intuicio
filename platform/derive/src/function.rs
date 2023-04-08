use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, parse_str, AttributeArgs, FnArg, Ident, ItemFn, Lit, Meta, NestedMeta, Pat,
    Path, ReturnType, TypePath, Visibility,
};

#[derive(Default)]
struct Attributes {
    pub name: Option<Ident>,
    pub module_name: Option<Ident>,
    pub struct_type: Option<TypePath>,
    pub use_registry: bool,
    pub use_context: bool,
    pub debug: bool,
}

macro_rules! parse_attributes {
    ($attributes:ident) => {{
        let mut result = Attributes::default();
        let attributes = parse_macro_input!($attributes as AttributeArgs);
        for attribute in attributes {
            match attribute {
                NestedMeta::Meta(meta) => match meta {
                    Meta::Path(path) => {
                        if path.is_ident("use_registry") {
                            result.use_registry = true;
                        } else if path.is_ident("use_context") {
                            result.use_context = true;
                        } else if path.is_ident("debug") {
                            result.debug = true;
                        }
                    }
                    Meta::NameValue(name_value) => {
                        if name_value.path.is_ident("name") {
                            match name_value.lit {
                                Lit::Str(content) => {
                                    result.name =
                                        Some(Ident::new(&content.value(), Span::call_site().into()))
                                }
                                _ => {}
                            }
                        } else if name_value.path.is_ident("module_name") {
                            match name_value.lit {
                                Lit::Str(content) => {
                                    result.module_name =
                                        Some(Ident::new(&content.value(), Span::call_site().into()))
                                }
                                _ => {}
                            }
                        } else if name_value.path.is_ident("struct_type") {
                            match name_value.lit {
                                Lit::Str(content) => {
                                    result.struct_type = Some(TypePath {
                                        qself: None,
                                        path: parse_str::<Path>(&content.value()).unwrap(),
                                    })
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        result
    }};
}

pub fn intuicio_function(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let attributes2 = attributes.clone();
    let Attributes {
        name,
        module_name,
        struct_type,
        use_registry,
        use_context,
        debug,
    } = parse_attributes!(attributes2);
    let input2 = input.clone();
    let item = parse_macro_input!(input2 as ItemFn);
    let vis = item.vis.clone();
    let ident = item.sig.ident.clone();
    let name = if let Some(name) = name {
        quote! { result.name = stringify!(#name).to_owned(); }
    } else {
        quote! {}
    };
    let visibility = match vis {
        Visibility::Inherited => {
            quote! { result.visibility = intuicio_core::Visibility::Private; }
        }
        Visibility::Restricted(_) | Visibility::Crate(_) => {
            quote! { result.visibility = intuicio_core::Visibility::Module; }
        }
        Visibility::Public(_) => quote! {},
    };
    let module_name = if let Some(module_name) = module_name {
        quote! { result.module_name = Some(stringify!(#module_name).to_owned()); }
    } else {
        quote! {}
    };
    let struct_handle = if let Some(struct_type) = struct_type {
        quote! { result.struct_handle = Some(registry.find_struct(intuicio_core::struct_type::StructQuery::of_type_name::<#struct_type>()).unwrap()); }
    } else {
        quote! {}
    };
    let arg_idents = item
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => panic!("`self` arguments are not accepted!"),
            FnArg::Typed(meta) => match &*meta.pat {
                Pat::Ident(ident) => {
                    if (use_registry && ident.ident == "registry")
                        || (use_context && ident.ident == "context")
                    {
                        None
                    } else {
                        Some(&ident.ident)
                    }
                }
                _ => panic!("Only identifiers are accepted as argument names!"),
            },
        })
        .collect::<Vec<_>>();
    let call_arg_idents = item
        .sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(_) => panic!("`self` arguments are not accepted!"),
            FnArg::Typed(meta) => match &*meta.pat {
                Pat::Ident(ident) => &ident.ident,
                _ => panic!("Only identifiers are accepted as argument names!"),
            },
        })
        .collect::<Vec<_>>();
    let arg_types = item
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => panic!("`self` arguments are not accepted!"),
            FnArg::Typed(meta) => {
                let ident = match &*meta.pat {
                    Pat::Ident(ident) => &ident.ident,
                    _ => panic!("Only identifiers are accepted as argument names!"),
                };
                if (use_registry && ident == "registry") || (use_context && ident == "context") {
                    None
                } else {
                    Some(&meta.ty)
                }
            }
        })
        .collect::<Vec<_>>();
    let return_idents = match item.sig.output {
        ReturnType::Default => vec![],
        ReturnType::Type(_, _) => vec!["result"],
    };
    let return_types = match item.sig.output {
        ReturnType::Default => vec![],
        ReturnType::Type(_, ref ty) => vec![ty.clone()],
    };
    let result = quote! {
        pub mod #ident {
            use super::*;

            #[allow(dead_code)]
            pub fn intuicio_function(
                context: &mut intuicio_core::context::Context,
                registry: &intuicio_core::registry::Registry,
            ) {
                use intuicio_data::data_stack::DataStackPack;
                #[allow(unused_mut)]
                let (#(mut #arg_idents,)*) = <(#(#arg_types,)*)>::stack_pop(context.stack());
                let result = super::#ident(#(#call_arg_idents,)*);
                (result,).stack_push_reversed(context.stack());
            }

            #[allow(dead_code)]
            pub fn define_signature(
                registry: &intuicio_core::registry::Registry
            ) -> intuicio_core::function::FunctionSignature {
                let mut result = intuicio_core::function::FunctionSignature::new(stringify!(#ident));
                #visibility
                #name
                #module_name
                #struct_handle
                #(
                    result.inputs.push(
                        intuicio_core::function::FunctionParameter::new(
                            stringify!(#arg_idents),
                            registry.find_struct(intuicio_core::struct_type::StructQuery::of_type_name::<#arg_types>()).unwrap()
                        )
                    );
                )*
                #(
                    result.outputs.push(
                        intuicio_core::function::FunctionParameter::new(
                            #return_idents,
                            registry.find_struct(intuicio_core::struct_type::StructQuery::of_type_name::<#return_types>()).unwrap()
                        )
                    );
                )*
                result
            }

            #[allow(dead_code)]
            pub fn define_function(
                registry: &intuicio_core::registry::Registry
            ) -> intuicio_core::function::Function {
                intuicio_core::function::Function::new(
                    define_signature(registry),
                    intuicio_core::function::FunctionBody::pointer(intuicio_function),
                )
            }
        }

        #item
    }
    .into();
    if debug {
        println!(
            "* Debug of `intuicio_function` attribute macro\n- Attributes: {}\n- Input: {}\n- Result: {}",
            attributes, input, result
        );
    }
    result
}