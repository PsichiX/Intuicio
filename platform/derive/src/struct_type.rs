use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Ident, ItemStruct, Lit, Meta, NestedMeta, Visibility};

#[derive(Default)]
struct StructAttributes {
    pub name: Option<Ident>,
    pub module_name: Option<Ident>,
    pub debug: bool,
}

#[derive(Default)]
struct FieldAttributes {
    pub name: Option<Ident>,
    pub ignore: bool,
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
                Err(err) => return TokenStream::from(err.to_compile_error()),
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
        debug,
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
    let mut fields_attributes = Vec::with_capacity(fields.len());
    for field in fields.iter() {
        fields_attributes.push(parse_field_attributes!(&field.attrs));
    }
    let fields = fields
        .iter()
        .zip(fields_attributes.into_iter())
        .filter_map(|(field, attributes)| {
            let FieldAttributes { name, ignore } = attributes;
            if ignore {
                return None;
            }
            let field_name = match field.ident.as_ref() {
                Some(ident) => ident,
                None => panic!("Struct: {} has field without a name!", ident.to_string()),
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
            Some(quote! {
                let mut field = intuicio_core::struct_type::StructField::new(
                    #name,
                    registry
                        .find_struct(intuicio_core::struct_type::StructQuery::of_type_name::<#field_type>())
                        .unwrap(),
                );
                #visibility
                result = result.field(
                    field,
                    intuicio_core::__internal::offset_of!(#ident, #field_name),
                );
            })
        })
        .collect::<Vec<_>>();
    let result = quote! {
        impl intuicio_core::IntuicioStruct for #ident {
            #[allow(dead_code)]
            fn define_struct(
                registry: &intuicio_core::registry::Registry,
            ) -> intuicio_core::struct_type::Struct {
                let name = #name;
                let mut result = intuicio_core::struct_type::NativeStructBuilder::new_named::<#ident>(name);
                #visibility
                #module_name
                #(#fields)*
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
