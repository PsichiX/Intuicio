use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_macro_input, Expr, Fields, Ident, Index, ItemEnum, Lit, Meta, NestedMeta, Visibility,
};

#[derive(Default)]
struct EnumAttributes {
    pub name: Option<Ident>,
    pub module_name: Option<Ident>,
    pub override_send: Option<bool>,
    pub override_sync: Option<bool>,
    pub override_copy: Option<bool>,
    pub debug: bool,
    pub meta: Option<String>,
    pub is_repr_u8: bool,
}

#[derive(Default)]
struct VariantAttributes {
    pub name: Option<Ident>,
    pub ignore: bool,
    pub meta: Option<String>,
    pub is_default: bool,
}

#[derive(Default)]
struct FieldAttributes {
    pub name: Option<Ident>,
    pub ignore: bool,
    pub meta: Option<String>,
}

macro_rules! parse_enum_attributes {
    ($attributes:expr) => {{
        let mut result = EnumAttributes::default();
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
                    } else if list.path.is_ident("repr") {
                        for meta in list.nested.iter() {
                            if let NestedMeta::Meta(Meta::Path(path)) = meta {
                                if path.is_ident("u8") {
                                    result.is_repr_u8 = true;
                                }
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

macro_rules! parse_variant_attributes {
    ($attributes:expr) => {{
        let mut result = VariantAttributes::default();
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

pub fn intuicio_enum(input: TokenStream) -> TokenStream {
    let input2 = input.clone();
    let ItemEnum {
        attrs,
        ident,
        vis,
        variants,
        ..
    } = parse_macro_input!(input2 as ItemEnum);
    let EnumAttributes {
        name,
        module_name,
        override_send,
        override_sync,
        override_copy,
        debug,
        meta,
        is_repr_u8,
    } = parse_enum_attributes!(attrs);
    if !is_repr_u8 {
        panic!("Enum: {} does not have `repr(u8)` attribute!", ident);
    }
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
    let mut discriminant = 0u8;
    let mut default_variant = None;
    let variants = variants
        .iter()
        .filter_map(|variant| {
            let VariantAttributes {
                name,
                ignore,
                meta,
                is_default
            } = parse_variant_attributes!(&variant.attrs)?;
            if ignore {
                return None;
            }
            let variant_name = variant.ident.clone();
            let name = if let Some(name) = name {
                quote! { stringify!(#name) }
            } else {
                quote! { stringify!(#variant_name) }
            };
            if let Some((_, value)) = variant.discriminant.as_ref() {
                let Expr::Lit(value) = value else {
                    panic!("Enum: {} variant: {} has non-literal discriminant!", ident, variant_name);
                };
                let Lit::Int(value) = &value.lit else {
                    panic!("Enum: {} variant: {} has non-integer discriminant!", ident, variant_name);
                };
                discriminant = value.base10_parse().unwrap();
            }
            let fields = match &variant.fields {
                Fields::Named(fields) => {
                    fields
                        .named
                        .iter()
                        .filter_map(|field| {
                            let FieldAttributes {
                                name,
                                ignore,
                                meta
                            } = parse_field_attributes!(&field.attrs)?;
                            if ignore {
                                return None;
                            }
                            let field_name = match field.ident.as_ref() {
                                Some(ident) => ident,
                                None => panic!("Enum: {} variant: {} has field without a name!", ident, variant_name),
                            };
                            let name = if let Some(name) = name {
                                quote! { stringify!(#name) }
                            } else {
                                quote! { stringify!(#field_name) }
                            };
                            let field_type = &field.ty;
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
                                #meta
                                variant = variant.with_field_with_offset(
                                    field,
                                    intuicio_core::__internal__offset_of_enum__!(#ident :: #variant_name { #field_name } => #discriminant),
                                );
                            })
                        })
                        .collect::<Vec<_>>()
                }
                Fields::Unnamed(fields) => {
                    fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .filter_map(|(index, field)| {
                            let FieldAttributes {
                                name,
                                ignore,
                                meta
                            } = parse_field_attributes!(&field.attrs)?;
                            if ignore {
                                return None;
                            }
                            let name = if let Some(name) = name {
                                quote! { stringify!(#name) }
                            } else {
                                quote! { stringify!(#index) }
                            };
                            let field_type = &field.ty;
                            let meta = if let Some(meta) = meta {
                                quote! { field.meta = intuicio_core::meta::Meta::parse(#meta).ok(); }
                            } else {
                                quote! {}
                            };
                            let field_name = Index::from(index);
                            Some(quote! {
                                let mut field = intuicio_core::types::struct_type::StructField::new(
                                    #name,
                                    registry
                                        .find_type(intuicio_core::types::TypeQuery::of_type_name::<#field_type>())
                                        .unwrap(),
                                );
                                #meta
                                variant = variant.with_field_with_offset(
                                    field,
                                    intuicio_core::__internal__offset_of_enum__!(#ident :: #variant_name ( #field_name ) => #discriminant),
                                );
                            })
                        })
                        .collect::<Vec<_>>()
                },
                Fields::Unit => vec![],
            };
            let meta = if let Some(meta) = meta {
                quote! { variant.meta = intuicio_core::meta::Meta::parse(#meta).ok(); }
            } else {
                quote! {}
            };
            if is_default {
                default_variant = Some(discriminant);
            }
            let disc = discriminant;
            discriminant += 1;
            Some(quote! {
                let mut variant = intuicio_core::types::enum_type::EnumVariant::new(#name);
                #(#fields)*
                #meta
                result = result.variant(variant, #disc);
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
    let default_variant = if let Some(discriminant) = default_variant {
        quote! { result = result.set_default_variant(#discriminant); }
    } else {
        quote! {}
    };
    let result = quote! {
        impl intuicio_core::IntuicioEnum for #ident {
            #[allow(dead_code)]
            fn define_enum(
                registry: &intuicio_core::registry::Registry,
            ) -> intuicio_core::types::enum_type::Enum {
                let name = #name;
                let mut result = intuicio_core::types::enum_type::NativeEnumBuilder::new_named::<#ident>(name);
                #visibility
                #module_name
                #(#variants)*
                #default_variant
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
            "* Debug of `IntuicioEnum` derive macro\n- Input: {}\n- Result: {}",
            input, result
        );
    }
    result
}
