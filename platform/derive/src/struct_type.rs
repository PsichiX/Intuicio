//! Expansion of the `IntuicioStruct` derive.
//!
//! Produces an `IntuicioStruct::define_struct` impl that fills a
//! `NativeStructBuilder`: name, module and visibility, then one
//! `StructField` per field with the offset taken from the compiler through
//! `offset_of`. Fields marked `ignore` are left out, so they stay invisible
//! to scripts while remaining part of the Rust value.
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Lit, Meta, NestedMeta, Visibility, parse_macro_input};

/// Everything `#[intuicio(...)]` can carry on the struct.
#[derive(Default)]
struct StructAttributes {
    /// Registered name, [`None`] uses the full Rust type name.
    pub name: Option<String>,
    /// Module the type is registered under.
    pub module_name: Option<String>,
    /// Whether to describe the type without a default constructor, letting
    /// scripts hold values without being able to make one.
    pub uninitialized: bool,
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
}

/// Everything `#[intuicio(...)]` can carry on a field.
#[derive(Default)]
struct FieldAttributes {
    /// Registered name, [`None`] keeps the Rust one.
    pub name: Option<String>,
    /// Whether to leave the field out of the description.
    pub ignore: bool,
    /// `Meta` source attached to the field.
    pub meta: Option<String>,
}

/// Reads `#[intuicio(...)]` on the struct into [`StructAttributes`].
///
/// Returns a compile error from the surrounding function when an attribute
/// fails to parse, so it only works inside one returning [`TokenStream`].
macro_rules! parse_struct_attributes {
    ($attributes:expr) => {{
        let mut result = StructAttributes::default();
        for attribute in $attributes {
            let attribute = match attribute.parse_meta() {
                Ok(attribute) => attribute,
                Err(err) => return TokenStream::from(err.to_compile_error()),
            };
            match attribute {
                Meta::List(list) if list.path.is_ident("intuicio") => {
                    for meta in list.nested.iter() {
                        match meta {
                            NestedMeta::Meta(meta) => match meta {
                                Meta::Path(path) => {
                                    if path.is_ident("debug") {
                                        result.debug = true;
                                    } else if path.is_ident("uninitialized") {
                                        result.uninitialized = true;
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
                }
                _ => {}
            }
        }
        result
    }};
}

/// Reads `#[intuicio(...)]` on one field into [`FieldAttributes`].
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
/// Panics on a field without a name, so tuple structs are not supported.
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
        uninitialized,
        override_send,
        override_sync,
        override_copy,
        debug,
        meta,
    } = parse_struct_attributes!(attrs);
    let construct = if uninitialized {
        quote! { intuicio_core::types::struct_type::NativeStructBuilder::new_named_uninitialized::<#ident>(name) }
    } else {
        quote! { intuicio_core::types::struct_type::NativeStructBuilder::new_named::<#ident>(name) }
    };
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
    let fields = fields
        .iter()
        .filter_map(|field| {
            let FieldAttributes { name, ignore, meta } = parse_field_attributes!(&field.attrs)?;
            if ignore {
                return None;
            }
            let field_name = match field.ident.as_ref() {
                Some(ident) => ident,
                None => panic!("Struct: {ident} has field without a name!"),
            };
            let name = if let Some(name) = name {
                quote! { #name }
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
        quote! { result = result.maybe_meta(intuicio_core::meta::Meta::parse(#meta).ok()); }
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
                let mut result = #construct;
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
    }
    .into();
    if debug {
        println!("* Debug of `IntuicioStruct` derive macro\n- Input: {input}\n- Result: {result}");
    }
    result
}
