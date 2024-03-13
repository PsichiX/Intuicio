use proc_macro::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, AttributeArgs, FnArg, Ident, ImplItem, ItemImpl, Lit, Meta, NestedMeta, Pat,
    ReturnType, Type, Visibility,
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
    pub transformer: Option<Ident>,
    pub dependency: Option<Ident>,
    pub meta: Option<String>,
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
                                        } else if name_value.path.is_ident("transformer") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.transformer = Some(Ident::new(
                                                        &content.value(),
                                                        Span::call_site().into(),
                                                    ))
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("dependency") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.dependency = Some(Ident::new(
                                                        &content.value(),
                                                        Span::call_site().into(),
                                                    ))
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("meta") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.meta = Some(content.value());
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
    let type_path = &item.self_ty;
    let type_handle = quote! {
        result.type_handle = Some(
            registry
                .find_type(intuicio_core::types::TypeQuery::of_type_name::<#type_path>())
                .unwrap_or_else(|| panic!("Could not find type: `{}`", std::any::type_name::<#type_path>()))
        );
    };
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
                transformer,
                dependency,
                meta,
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
        let meta = if let Some(meta) = meta {
            quote! { result.meta = intuicio_core::meta::Meta::parse(#meta).ok(); }
        } else {
            quote! {}
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
        let arg_types: Vec<_> = item
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => Some(*type_path.clone()),
                FnArg::Typed(meta) => {
                    let ident = match &*meta.pat {
                        Pat::Ident(ident) => &ident.ident,
                        _ => panic!("Only identifiers are accepted as argument names!"),
                    };
                    if (use_registry && ident == "registry") || (use_context && ident == "context")
                    {
                        None
                    } else {
                        Some(transformer
                            .as_ref()
                            .map(|transformer| match unpack_type(&meta.ty) {
                                UnpackedType::Owned(ty) => {
                                    syn::parse2::<Type>(quote!{
                                        <#transformer<#ty> as intuicio_core::transformer::ValueTransformer>::Owned
                                    }).unwrap()
                                }
                                UnpackedType::Ref(ty) => {
                                    syn::parse2::<Type>(quote!{
                                        <#transformer<#ty> as intuicio_core::transformer::ValueTransformer>::Ref
                                    }).unwrap()
                                }
                                UnpackedType::RefMut(ty) => {
                                    syn::parse2::<Type>(quote!{
                                        <#transformer<#ty> as intuicio_core::transformer::ValueTransformer>::RefMut
                                    }).unwrap()
                                }
                            })
                            .unwrap_or_else(|| *meta.ty.clone()))
                    }
                }
            })
            .collect();
        let (transform_arg_idents, arg_transforms): (Vec<_>, Vec<_>) =
            if let Some(transformer) = transformer.as_ref() {
                item.sig
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        FnArg::Receiver(_) => panic!("`self` arguments are not accepted!"),
                        FnArg::Typed(meta) => {
                            let ident = match &*meta.pat {
                                Pat::Ident(ident) => &ident.ident,
                                _ => panic!("Only identifiers are accepted as argument names!"),
                            };
                            if (use_registry && ident == "registry")
                                || (use_context && ident == "context")
                            {
                                None
                            } else {
                                Some((
                                    ident,
                                    match unpack_type(&meta.ty) {
                                        UnpackedType::Owned(_) => {
                                            quote! {#transformer::into_owned(#ident)}
                                        }
                                        UnpackedType::Ref(_) => {
                                            quote! {#transformer::into_ref(&#ident)}
                                        }
                                        UnpackedType::RefMut(_) => {
                                            quote! {#transformer::into_ref_mut(&mut #ident)}
                                        }
                                    },
                                ))
                            }
                        }
                    })
                    .unzip()
            } else {
                (vec![], vec![])
            };
        let transform_arg_deref = if transformer.is_some() {
            item.sig
                .inputs
                .iter()
                .filter_map(|arg| match arg {
                    FnArg::Receiver(_) => panic!("`self` arguments are not accepted!"),
                    FnArg::Typed(meta) => {
                        let ident = match &*meta.pat {
                            Pat::Ident(ident) => &ident.ident,
                            _ => panic!("Only identifiers are accepted as argument names!"),
                        };
                        if (use_registry && ident == "registry")
                            || (use_context && ident == "context")
                        {
                            None
                        } else {
                            match unpack_type(&meta.ty) {
                                UnpackedType::Owned(_) => None,
                                UnpackedType::Ref(_) => Some(quote! {let #ident = &#ident;}),
                                UnpackedType::RefMut(_) => Some(quote! {let #ident = &mut #ident;}),
                            }
                        }
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let return_idents = match item.sig.output {
            ReturnType::Default => vec![],
            ReturnType::Type(_, _) => vec!["result"],
        };
        let return_types = match item.sig.output {
            ReturnType::Default => vec![],
            ReturnType::Type(_, ref ty) => vec![transformer
                .as_ref()
                .map(|_| match unpack_type(ty) {
                    UnpackedType::Owned(ty) => ty,
                    UnpackedType::Ref(ty) => ty,
                    UnpackedType::RefMut(ty) => ty,
                })
                .unwrap_or_else(|| *ty.clone())],
        };
        let (dependency, return_transform) = if let Some(transformer) = transformer.as_ref() {
            match item.sig.output {
                ReturnType::Default => (vec![], vec![]),
                ReturnType::Type(_, ref ty) => match unpack_type(ty) {
                    UnpackedType::Owned(_) => (
                        vec![],
                        vec![quote! {let result = #transformer::from_owned(registry, result);}],
                    ),
                    UnpackedType::Ref(ty) => (
                        vec![dependency.as_ref().map(|dependency|{
                            quote! {
                                let __dependency__ = Some(
                                    <#transformer<#ty> as intuicio_core::transformer::ValueTransformer>::Dependency::as_ref(&#dependency)
                                );
                            }
                        }).unwrap_or_else(|| quote!{let __dependency__ = None;})],
                        vec![quote! {let result = #transformer::from_ref(registry, result, __dependency__);}],
                    ),
                    UnpackedType::RefMut(_) => (
                        vec![dependency.as_ref().map(|dependency|{
                            quote! {
                                let __dependency__ = Some(
                                    <#transformer<#ty> as intuicio_core::transformer::ValueTransformer>::Dependency::as_ref_mut(&#dependency)
                                );
                            }
                        }).unwrap_or_else(|| quote!{let __dependency__ = None;})],
                        vec![quote! {let result = #transformer::from_ref_mut(registry, result, None);}],
                    ),
                },
            }
        } else {
            (vec![], vec![])
        };
        let result = if return_types.is_empty() {
            quote! {
                {
                    #(#transform_arg_deref)*
                    #type_path::#ident(#(#call_arg_idents,)*)
                }
            }
        } else {
            quote! {
                let result = {
                    #(#transform_arg_deref)*
                    #type_path::#ident(#(#call_arg_idents,)*)
                };
                #(#return_transform)*
                (result,).stack_push_reversed(context.stack());
            }
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
                #(#dependency)*
                let (#(mut #transform_arg_idents,)*) = (#(#arg_transforms,)*);
                #result
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
                #type_handle
                #meta
                #(
                    result.inputs.push(
                        intuicio_core::function::FunctionParameter::new(
                            stringify!(#arg_idents),
                            registry
                                .find_type(intuicio_core::types::TypeQuery::of_type_name::<#arg_types>())
                                .unwrap_or_else(|| panic!(
                                    "Could not find type: `{}` for argument: `{}` for function: `{}`",
                                    std::any::type_name::<#arg_types>(),
                                    stringify!(#arg_idents),
                                    stringify!(#ident),
                                ))
                        )
                    );
                )*
                #(
                    result.outputs.push(
                        intuicio_core::function::FunctionParameter::new(
                            #return_idents,
                            registry
                                .find_type(intuicio_core::types::TypeQuery::of_type_name::<#return_types>())
                                .unwrap_or_else(|| panic!(
                                    "Could not find type: `{}` for result: `{}` for function: `{}`",
                                    std::any::type_name::<#return_types>(),
                                    stringify!(#return_idents),
                                    stringify!(#ident),
                                ))
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
                    #type_path::#define_signature_ident(registry),
                    intuicio_core::function::FunctionBody::pointer(#type_path::#intuicio_function_ident),
                )
            }
        };
        if debug {
            println!(
                "* Debug of `intuicio_method` attribute macro\n- Input: {}\n- Result: {}",
                item.to_token_stream(),
                result
            );
        }
        methods.push(result);
    }
    quote! {
        impl #type_path {
            #(#methods)*
        }

        #item
    }
    .into()
}

enum UnpackedType {
    Owned(Type),
    Ref(Type),
    RefMut(Type),
}

fn unpack_type(ty: &Type) -> UnpackedType {
    match ty {
        Type::Path(_) => UnpackedType::Owned(ty.clone()),
        Type::Reference(reference) => {
            if reference.mutability.is_some() {
                UnpackedType::RefMut(*reference.elem.clone())
            } else {
                UnpackedType::Ref(*reference.elem.clone())
            }
        }
        _ => panic!("Unsupported kind of type to unpack: {:#?}", ty),
    }
}
