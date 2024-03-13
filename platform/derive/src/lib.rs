mod enum_type;
mod function;
mod methods;
mod struct_type;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn intuicio_function(attributes: TokenStream, input: TokenStream) -> TokenStream {
    crate::function::intuicio_function(attributes, input)
}

#[proc_macro_derive(IntuicioStruct, attributes(intuicio))]
pub fn intuicio_struct(input: TokenStream) -> TokenStream {
    crate::struct_type::intuicio_struct(input)
}

#[proc_macro_derive(IntuicioEnum, attributes(intuicio))]
pub fn intuicio_enum(input: TokenStream) -> TokenStream {
    crate::enum_type::intuicio_enum(input)
}

#[proc_macro_attribute]
pub fn intuicio_methods(attributes: TokenStream, input: TokenStream) -> TokenStream {
    crate::methods::intuicio_methods(attributes, input)
}

#[proc_macro_attribute]
pub fn intuicio_method(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}
