use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{Ident, ItemStruct, Lit, Meta, NestedMeta, Visibility, parse_macro_input};

#[derive(Default)]
struct StructAttributes {
    pub name: Option<Ident>,
    pub module_name: Option<Ident>,
    pub override_send: Option<bool>,
    pub override_sync: Option<bool>,
    pub override_copy: Option<bool>,
    pub debug: bool,
    pub meta: Option<String>,
}

#[derive(Default)]
struct FieldAttributes {
    pub name: Option<Ident>,
    pub ignore: bool,
    pub meta: Option<String>,
}

macro_rules! parse_struct_attributes {
    ($attributes:expr) => {{
        let mut result = StructAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return TokenStream::from(err.to_compile_error()),
            };
            match attribute {
                Meta::List(list) => {
                    if list.path.is_ident("intuicio") {
                        for meta in list.nested.iter() {
                            match meta {
                                NestedMeta::Meta(meta) => match meta {
                                    Meta::Path(path) => {
                                        if path.is_ident("debug") {
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
                                        } else if name_value.path.is_ident("module_name") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.module_name = Some(Ident::new(
                                                        &content.value(),
                                                        Span::call_site().into(),
                                                    ))
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("override_send") {
                                            match &name_value.lit {
                                                Lit::Bool(content) => {
                                                    result.override_send = Some(content.value)
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("override_sync") {
                                            match &name_value.lit {
                                                Lit::Bool(content) => {
                                                    result.override_sync = Some(content.value)
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("override_copy") {
                                            match &name_value.lit {
                                                Lit::Bool(content) => {
                                                    result.override_copy = Some(content.value)
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
        result
    }};
}

macro_rules! parse_field_attributes {
    ($attributes:expr) => {{
        let mut result = FieldAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return Some(TokenStream::from(err.to_compile_error()).into()),
            };
            match attribute {
                Meta::List(list) => {
                    if list.path.is_ident("intuicio") {
                        for meta in list.nested.iter() {
                            match meta {
                                NestedMeta::Meta(meta) => match meta {
                                    Meta::Path(path) => {
                                        if path.is_ident("ignore") {
                                            result.ignore = true;
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
        Some(result)
    }};
}

pub fn intuicio_struct(input: TokenStream) -> TokenStream {
    let input2 = input.clone();
    let ItemStruct {
        attrs,
        ident,
        vis,
        fields,
        ..
    } = parse_macro_input!(input2 as ItemStruct);
    let StructAttributes {
        name,
        module_name,
        override_send,
        override_sync,
        override_copy,
        debug,
        meta,
    } = parse_struct_attributes!(attrs);
    let name = if let Some(name) = name {
        quote! { stringify!(#name) }
    } else {
        quote! { std::any::type_name::<#ident>() }
    };
    let visibility = match vis {
        Visibility::Inherited => {
            quote! { result = result.visibility(intuicio_core::Visibility::Private); }
        }
        Visibility::Restricted(_) | Visibility::Crate(_) => {
            quote! { result = result.visibility(intuicio_core::Visibility::Module); }
        }
        Visibility::Public(_) => quote! {},
    };
    let module_name = if let Some(module_name) = module_name {
        quote! { result = result.module_name(stringify!(#module_name)); }
    } else {
        quote! {}
    };
    let fields = fields
        .iter()
        .filter_map(|field| {
            let FieldAttributes { name, ignore, meta } = parse_field_attributes!(&field.attrs)?;
            if ignore {
                return None;
            }
            let field_name = match field.ident.as_ref() {
                Some(ident) => ident,
                None => panic!("Struct: {} has field without a name!", ident),
            };
            let name = if let Some(name) = name {
                quote! { stringify!(#name) }
            } else {
                quote! { stringify!(#field_name) }
            };
            let field_type = &field.ty;
            let visibility = match field.vis {
                Visibility::Inherited => {
                    quote! { field.visibility = intuicio_core::Visibility::Private; }
                }
                Visibility::Restricted(_) | Visibility::Crate(_) => {
                    quote! { field.visibility = intuicio_core::Visibility::Module; }
                }
                Visibility::Public(_) => quote! {},
            };
            let meta = if let Some(meta) = meta {
                quote! { field.meta = intuicio_core::meta::Meta::parse(#meta).ok(); }
            } else {
                quote! {}
            };
            Some(quote! {
                let mut field = intuicio_core::types::struct_type::StructField::new(
                    #name,
                    registry
                        .find_type(intuicio_core::types::TypeQuery::of_type_name::<#field_type>())
                        .unwrap(),
                );
                #visibility
                #meta
                result = result.field(
                    field,
                    intuicio_core::__internal__offset_of__!(#ident, #field_name),
                );
            })
        })
        .collect::<Vec<_>>();
    let override_send = if let Some(override_send) = override_send {
        quote! { result = unsafe { result.override_send(#override_send) }; }
    } else {
        quote! {}
    };
    let override_sync = if let Some(override_sync) = override_sync {
        quote! { result = unsafe { result.override_sync(#override_sync) }; }
    } else {
        quote! {}
    };
    let override_copy = if let Some(override_copy) = override_copy {
        quote! { result = unsafe { result.override_copy(#override_copy) }; }
    } else {
        quote! {}
    };
    let meta = if let Some(meta) = meta {
        quote! { result.meta = intuicio_core::meta::Meta::parse(#meta).ok(); }
    } else {
        quote! {}
    };
    let result = quote! {
        impl intuicio_core::IntuicioStruct for #ident {
            #[allow(dead_code)]
            fn define_struct(
                registry: &intuicio_core::registry::Registry,
            ) -> intuicio_core::types::struct_type::Struct {
                let name = #name;
                let mut result = intuicio_core::types::struct_type::NativeStructBuilder::new_named::<#ident>(name);
                #visibility
                #module_name
                #(#fields)*
                #override_send
                #override_sync
                #override_copy
                #meta
                result.build()
            }
        }
    }.into();
    if debug {
        println!(
            "* Debug of `IntuicioStruct` derive macro\n- Input: {}\n- Result: {}",
            input, result
        );
    }
    result
}
