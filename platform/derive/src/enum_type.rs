//! Expansion of the `IntuicioEnum` derive.
//!
//! Produces an `IntuicioEnum::define_enum` impl that fills a
//! `NativeEnumBuilder`: one `EnumVariant` per variant with its
//! discriminant, and inside each variant one `StructField` per field, at
//! the offset the compiler chose for that variant.
//!
//! `repr(u8)` is required, because the runtime reads the discriminant as a
//! single byte. Discriminants are counted from `0` in declaration order and
//! an explicit `= N` literal resets the count. A variant marked `ignore` is
//! left out of the description but still takes up its discriminant, so the
//! variants after it keep the values Rust gave them.
use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, Fields, Index, ItemEnum, Lit, Meta, NestedMeta, Visibility, parse_macro_input};

/// Everything `#[intuicio(...)]` can carry on the enum.
#[derive(Default)]
struct EnumAttributes {
    /// Registered name, [`None`] uses the full Rust type name.
    pub name: Option<String>,
    /// Module the type is registered under.
    pub module_name: Option<String>,
    /// Claims or denies `Send` regardless of the Rust type.
    pub override_send: Option<bool>,
    /// Claims or denies `Sync` regardless of the Rust type.
    pub override_sync: Option<bool>,
    /// Claims or denies copy semantics regardless of the Rust type.
    pub override_copy: Option<bool>,
    /// Whether to print the expansion while compiling.
    pub debug: bool,
    /// `Meta` source attached to the type.
    pub meta: Option<String>,
    /// Whether `#[repr(u8)]` was found, which the derive requires.
    pub is_repr_u8: bool,
}

/// Everything `#[intuicio(...)]` can carry on a variant.
#[derive(Default)]
struct VariantAttributes {
    /// Registered name, [`None`] keeps the Rust one.
    pub name: Option<String>,
    /// Whether to leave the variant out of the description.
    pub ignore: bool,
    /// `Meta` source attached to the variant.
    pub meta: Option<String>,
    /// Whether this variant is the one a default value starts in.
    pub is_default: bool,
}

/// Everything `#[intuicio(...)]` can carry on a variant field.
#[derive(Default)]
struct FieldAttributes {
    /// Registered name, [`None`] keeps the Rust one.
    pub name: Option<String>,
    /// Whether to leave the field out of the description.
    pub ignore: bool,
    /// `Meta` source attached to the field.
    pub meta: Option<String>,
}

/// Reads `#[intuicio(...)]` and `#[repr(...)]` on the enum into
/// [`EnumAttributes`].
///
/// Returns a compile error from the surrounding function when an attribute
/// fails to parse, so it only works inside one returning [`TokenStream`].
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
                                    Meta::Path(path) if path.is_ident("debug") => {
                                        result.debug = true;
                                    }
                                    Meta::NameValue(name_value) => {
                                        if name_value.path.is_ident("name") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.name = Some(content.value())
                                                }
                                                _ => {}
                                            }
                                        } else if name_value.path.is_ident("module_name") {
                                            match &name_value.lit {
                                                Lit::Str(content) => {
                                                    result.module_name = Some(content.value())
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

/// Reads `#[intuicio(...)]` on one variant into [`VariantAttributes`].
///
/// Wraps its result in [`Some`] so it can be used inside the `filter_map`
/// over variants, and returns a compile error from that closure on a parse
/// failure.
macro_rules! parse_variant_attributes {
    ($attributes:expr) => {{
        let mut result = VariantAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return Some(TokenStream::from(err.to_compile_error()).into()),
            };
            match attribute {
                Meta::List(list) if list.path.is_ident("intuicio") => {
                    for meta in list.nested.iter() {
                        match meta {
                            NestedMeta::Meta(meta) => match meta {
                                Meta::Path(path) => {
                                    if path.is_ident("ignore") {
                                        result.ignore = true;
                                    } else if path.is_ident("default") {
                                        result.is_default = true;
                                    }
                                }
                                Meta::NameValue(name_value) => {
                                    if name_value.path.is_ident("name") {
                                        match &name_value.lit {
                                            Lit::Str(content) => {
                                                result.name = Some(content.value())
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
                _ => {}
            }
        }
        Some(result)
    }};
}

/// Reads `#[intuicio(...)]` on one variant field into [`FieldAttributes`].
///
/// Wraps its result in [`Some`] so it can be used inside the `filter_map`
/// over fields, and returns a compile error from that closure on a parse
/// failure.
macro_rules! parse_field_attributes {
    ($attributes:expr) => {{
        let mut result = FieldAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return Some(TokenStream::from(err.to_compile_error()).into()),
            };
            match attribute {
                Meta::List(list) if list.path.is_ident("intuicio") => {
                    for meta in list.nested.iter() {
                        match meta {
                            NestedMeta::Meta(meta) => match meta {
                                Meta::Path(path) if path.is_ident("ignore") => {
                                    result.ignore = true;
                                }
                                Meta::NameValue(name_value) => {
                                    if name_value.path.is_ident("name") {
                                        match &name_value.lit {
                                            Lit::Str(content) => {
                                                result.name = Some(content.value())
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
                _ => {}
            }
        }
        Some(result)
    }};
}

/// Expands the derive. See the [module docs](self) for the shape of the
/// output.
///
/// # Panics
///
/// Panics without `#[repr(u8)]`, on a discriminant that is not an integer
/// literal, and on a named field without a name.
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
        panic!("Enum: {ident} does not have `repr(u8)` attribute!");
    }
    let name = if let Some(name) = name {
        quote! { #name }
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
        quote! { result = result.module_name(#module_name); }
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
            let variant_name = variant.ident.clone();
            if let Some((_, value)) = variant.discriminant.as_ref() {
                let Expr::Lit(value) = value else {
                    panic!("Enum: {ident} variant: {variant_name} has non-literal discriminant!");
                };
                let Lit::Int(value) = &value.lit else {
                    panic!("Enum: {ident} variant: {variant_name} has non-integer discriminant!");
                };
                discriminant = value.base10_parse().unwrap();
            }
            let disc = discriminant;
            discriminant += 1;
            if ignore {
                return None;
            }
            let name = if let Some(name) = name {
                quote! { #name }
            } else {
                quote! { stringify!(#variant_name) }
            };
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
                                None => panic!("Enum: {ident} variant: {variant_name} has field without a name!"),
                            };
                            let name = if let Some(name) = name {
                                quote! { #name }
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
                                    intuicio_core::__internal__offset_of_enum__!(#ident :: #variant_name { #field_name } => #disc),
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
                                quote! { #name }
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
                                    intuicio_core::__internal__offset_of_enum__!(#ident :: #variant_name ( #field_name ) => #disc),
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
                default_variant = Some(disc);
            }
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
        quote! { result = result.maybe_meta(intuicio_core::meta::Meta::parse(#meta).ok()); }
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
        println!("* Debug of `IntuicioEnum` derive macro\n- Input: {input}\n- Result: {result}");
    }
    result
}
