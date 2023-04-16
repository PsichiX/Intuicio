use proc_macro::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, AttributeArgs, FnArg, Ident, ImplItem, ItemImpl, Lit, Meta, NestedMeta, Pat,
    ReturnType, Visibility,
};

#[derive(Default)]
struct ImplAttributes {
    pub module_name: Option<Ident>,
}

#[derive(Default)]
struct MethodAttributes {
    pub name: Option<Ident>,
    pub use_registry: bool,
    pub use_context: bool,
    pub debug: bool,
}

macro_rules! parse_impl_attributes {
    ($attributes:ident) => {{
        let mut result = ImplAttributes::default();
        let attributes = parse_macro_input!($attributes as AttributeArgs);
        for attribute in attributes {
            match attribute {
                NestedMeta::Meta(Meta::NameValue(name_value)) => {
                    if name_value.path.is_ident("module_name") {
                        match name_value.lit {
                            Lit::Str(content) => {
                                result.module_name =
                                    Some(Ident::new(&content.value(), Span::call_site().into()))
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        result
    }};
}

macro_rules! parse_method_attributes {
    ($attributes:expr) => {{
        let mut found = false;
        let mut result = MethodAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return TokenStream::from(err.to_compile_error()),
            };
            match attribute {
                Meta::List(list) => {
                    if list.path.is_ident("intuicio_method") {
                        found = true;
                        for meta in list.nested.iter() {
                            match meta {
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
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.name = Some(Ident::new(
                                                        &content.value(),
                                                        Span::call_site().into(),
                                                    ))
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
                    }
                }
                _ => {}
            }
        }
        (result, found)
    }};
}

pub fn intuicio_methods(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let ImplAttributes { module_name } = parse_impl_attributes!(attributes);
    let item = parse_macro_input!(input as ItemImpl);
    if item.trait_.is_some() {
        panic!("Intuicio methods must be applied only for non-trait implementations!");
    }
    let module_name = if let Some(module_name) = module_name {
        quote! { result.module_name = Some(stringify!(#module_name).to_owned()); }
    } else {
        quote! {}
    };
    let struct_type = &item.self_ty;
    let struct_handle = quote! {result.struct_handle = Some(registry.find_struct(intuicio_core::struct_type::StructQuery::of_type_name::<#struct_type>()).unwrap()); };
    let items = item
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Method(method) => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut methods = Vec::with_capacity(items.len());
    for item in items {
        let (
            MethodAttributes {
                name,
                use_registry,
                use_context,
                debug,
            },
            found,
        ) = parse_method_attributes!(&item.attrs);
        if !found {
            continue;
        }
        let intuicio_function_ident = Ident::new(
            &format!("{}__intuicio_function", item.sig.ident),
            Span::call_site().into(),
        );
        let define_signature_ident = Ident::new(
            &format!("{}__define_signature", item.sig.ident),
            Span::call_site().into(),
        );
        let define_function_ident = Ident::new(
            &format!("{}__define_function", item.sig.ident),
            Span::call_site().into(),
        );
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
        let arg_idents = item
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => Some(Ident::new("this", Span::call_site().into())),
                FnArg::Typed(meta) => match &*meta.pat {
                    Pat::Ident(ident) => {
                        if (use_registry && ident.ident == "registry")
                            || (use_context && ident.ident == "context")
                        {
                            None
                        } else {
                            Some(ident.ident.clone())
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
                FnArg::Receiver(_) => Ident::new("this", Span::call_site().into()),
                FnArg::Typed(meta) => match &*meta.pat {
                    Pat::Ident(ident) => ident.ident.clone(),
                    _ => panic!("Only identifiers are accepted as argument names!"),
                },
            })
            .collect::<Vec<_>>();
        let arg_types = item
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => Some(struct_type),
                FnArg::Typed(meta) => {
                    let ident = match &*meta.pat {
                        Pat::Ident(ident) => &ident.ident,
                        _ => panic!("Only identifiers are accepted as argument names!"),
                    };
                    if (use_registry && ident == "registry") || (use_context && ident == "context")
                    {
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
            #[allow(dead_code)]
            #[allow(non_snake_case)]
            pub fn #intuicio_function_ident(
                context: &mut intuicio_core::context::Context,
                registry: &intuicio_core::registry::Registry,
            ) {
                use intuicio_data::data_stack::DataStackPack;
                #[allow(unused_mut)]
                let (#(mut #arg_idents,)*) = <(#(#arg_types,)*)>::stack_pop(context.stack());
                let result = #struct_type::#ident(#(#call_arg_idents,)*);
                (result,).stack_push_reversed(context.stack());
            }

            #[allow(dead_code)]
            #[allow(non_snake_case)]
            pub fn #define_signature_ident(
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
            #[allow(non_snake_case)]
            pub fn #define_function_ident(
                registry: &intuicio_core::registry::Registry
            ) -> intuicio_core::function::Function {
                intuicio_core::function::Function::new(
                    #struct_type::#define_signature_ident(registry),
                    intuicio_core::function::FunctionBody::pointer(#struct_type::#intuicio_function_ident),
                )
            }
        };
        if debug {
            println!(
                "* Debug of `intuicio_methods` attribute macro\n- Input: {}\n- Result: {}",
                item.to_token_stream(),
                result
            );
        }
        methods.push(result);
    }
    quote! {
        impl #struct_type {
            #(#methods)*
        }

        #item
    }
    .into()
}
