//! Procedural macros that expose Rust items to the Intuicio runtime.
//!
//! Every Intuicio function, native or scripted, has the same shape:
//! `fn(&mut Context, &Registry)`. It pops its arguments off the context
//! stack and pushes its results back. Writing that shim by hand for each
//! native function is noisy and easy to get wrong, so these macros generate
//! it from the ordinary Rust signature, along with the description the
//! registry needs.
//!
//! | Macro | Applied to | Produces |
//! |---|---|---|
//! | [`macro@intuicio_function`] | a free `fn` | a module named after the `fn`, holding `define_function` |
//! | [`macro@intuicio_methods`] | an inherent `impl` | `<method>__define_function` for each marked method |
//! | [`macro@intuicio_method`] | a method inside such an `impl` | nothing, it only marks the method |
//! | [`macro@IntuicioStruct`] | a `struct` | an `IntuicioStruct::define_struct` impl |
//! | [`macro@IntuicioEnum`] | a `#[repr(u8)]` `enum` | an `IntuicioEnum::define_enum` impl |
//!
//! Nothing registers itself. Each macro only writes a `define_*` function
//! that you call, so registration stays explicit and ordered:
//!
//! ```ignore
//! #[intuicio_function(module_name = "lib")]
//! fn add(a: i32, b: i32) -> i32 {
//!     a + b
//! }
//!
//! registry.add_function(add::define_function(&registry));
//! ```
//!
//! Every `define_*` function looks the types in its signature up in the
//! registry by name and panics when one is missing, so types have to be
//! registered before the functions that mention them.
//!
//! Rust visibility carries over: `pub` stays public, `pub(crate)` and
//! `pub(in ...)` become `Visibility::Module`, and a private item becomes
//! `Visibility::Private`.
//!
//! # Transformers
//!
//! `transformer = "SomeTransformer"` routes every argument and result through
//! a `ValueTransformer`. The transformer decides which box travels on the stack
//! in place of `T`, `&T` and `&mut T`, for example a managed value instead of a
//! bare one. `dependency = "arg"` names the argument that a returned reference
//! borrows from, so the transformer can tie the result to an owner that is
//! still alive.
//!
//! # Attribute syntax
//!
//! Values are always string literals, even for names: `name = "add"`, not
//! `name = add`. Flags such as `debug` stand alone.
//!
//! Registered names are kept as strings and never have to be Rust identifiers,
//! so `name = "+"` or `module_name = "core/ops"` are fine. Only the names the
//! script side sees are affected. The Rust items keep their own names.
mod enum_type;
mod function;
mod methods;
mod struct_type;

use proc_macro::TokenStream;

/// Wraps a free function so scripts can call it.
///
/// Generates a module named after the function, holding the
/// `fn(&mut Context, &Registry)` shim, `define_signature` and
/// `define_function`. The original function is emitted unchanged beside it.
///
/// ```ignore
/// #[intuicio_function(module_name = "lib")]
/// fn add(a: i32, b: i32) -> i32 {
///     a + b
/// }
///
/// registry.add_function(add::define_function(&registry));
/// ```
///
/// # Attributes
///
/// | Attribute | Effect |
/// |---|---|
/// | `name = "..."` | registered name, defaults to the Rust name |
/// | `module_name = "..."` | module the function is registered under |
/// | `type_path = "..."` | associates the function with that type |
/// | `use_registry` | the argument named `registry` receives `&Registry` instead of a stack value |
/// | `use_context` | the argument named `context` receives `&mut Context` instead of a stack value |
/// | `transformer = "..."` | routes arguments and results through a `ValueTransformer` |
/// | `dependency = "..."` | argument a returned reference borrows from |
/// | `meta = "..."` | `Meta` source attached to the function |
/// | `args_meta(a = "...")` | `Meta` source attached to argument `a` |
/// | `debug` | prints the generated code during compilation |
///
/// # Panics
///
/// Expanding panics on a `self` argument, or on an argument whose pattern is
/// not a plain identifier. The generated `define_signature` panics at run
/// time when a type from the signature is missing from the registry.
#[proc_macro_attribute]
pub fn intuicio_function(attributes: TokenStream, input: TokenStream) -> TokenStream {
    crate::function::intuicio_function(attributes, input)
}

/// Describes a `struct` to the registry as a native type.
///
/// Implements `IntuicioStruct::define_struct`, reporting each field with the
/// offset the Rust compiler chose, so the runtime reads the real layout
/// instead of a copy.
///
/// ```ignore
/// #[derive(IntuicioStruct, Default)]
/// #[intuicio(name = "Bytes", module_name = "bytes")]
/// pub struct Bytes {
///     #[intuicio(ignore)]
///     buffer: Vec<u8>,
/// }
/// ```
///
/// # Attributes
///
/// On the struct, inside `#[intuicio(...)]`:
///
/// | Attribute | Effect |
/// |---|---|
/// | `name = "..."` | registered name, defaults to the full Rust type name |
/// | `module_name = "..."` | module the type is registered under |
/// | `uninitialized` | describe the type without a default constructor, so scripts can hold values but not make one |
/// | `override_send = bool` | claim or deny `Send` regardless of the Rust type |
/// | `override_sync = bool` | claim or deny `Sync` regardless of the Rust type |
/// | `override_copy = bool` | claim or deny copy semantics regardless of the Rust type |
/// | `meta = "..."` | `Meta` source attached to the type |
/// | `debug` | prints the generated code during compilation |
///
/// On a field: `name = "..."`, `ignore` to leave it out of the description,
/// and `meta = "..."`. An ignored field still exists in Rust, scripts just
/// cannot see it.
///
/// The three `override_*` attributes expand to `unsafe` calls. Use them only
/// when you know the claim holds, since the runtime trusts them.
///
/// # Panics
///
/// Expanding panics on a tuple struct, because fields need names. The
/// generated `define_struct` panics when a field type is not in the registry.
#[proc_macro_derive(IntuicioStruct, attributes(intuicio))]
pub fn intuicio_struct(input: TokenStream) -> TokenStream {
    crate::struct_type::intuicio_struct(input)
}

/// Describes a `#[repr(u8)]` `enum` to the registry as a native type.
///
/// Implements `IntuicioEnum::define_enum`. Each variant is described with
/// its discriminant and its fields, again at compiler-chosen offsets.
///
/// ```ignore
/// #[derive(IntuicioEnum)]
/// #[repr(u8)]
/// #[intuicio(name = "Shape")]
/// enum Shape {
///     Empty,
///     Circle { radius: f32 },
/// }
/// ```
///
/// # Attributes
///
/// On the enum, inside `#[intuicio(...)]`: `name`, `module_name`,
/// `override_send`, `override_sync`, `override_copy`, `meta` and `debug`,
/// all as on [`macro@IntuicioStruct`].
///
/// On a variant: `name = "..."`, `ignore`, `meta = "..."`, and `default` to
/// mark the variant a default value starts in. On a variant field:
/// `name = "..."`, `ignore` and `meta = "..."`.
///
/// Discriminants are counted from `0` upwards in declaration order. An
/// explicit `= N` literal resets the count from there. A variant marked
/// `ignore` still takes up its discriminant, so the ones after it keep the
/// values Rust gave them.
///
/// # Panics
///
/// Expanding panics without `#[repr(u8)]`, on a non-literal or non-integer
/// discriminant, or on a named field without a name. The generated
/// `define_enum` panics when a field type is not in the registry.
#[proc_macro_derive(IntuicioEnum, attributes(intuicio))]
pub fn intuicio_enum(input: TokenStream) -> TokenStream {
    crate::enum_type::intuicio_enum(input)
}

/// Wraps the methods of an inherent `impl` so scripts can call them.
///
/// Only methods carrying [`macro@intuicio_method`] are exposed. The rest of the
/// block is left alone. For a method `foo` it adds `foo__intuicio_function`,
/// `foo__define_signature` and `foo__define_function` to the same type.
///
/// ```ignore
/// #[intuicio_methods(module_name = "bytes")]
/// impl Bytes {
///     #[intuicio_method(use_registry)]
///     pub fn new(registry: &Registry) -> Reference { /* ... */ }
/// }
///
/// registry.add_function(Bytes::new__define_function(&registry));
/// ```
///
/// A `self` receiver becomes the first parameter, named `this`. The type the
/// `impl` is for is attached to every signature, so methods stay grouped
/// under it in the registry.
///
/// # Attributes
///
/// `module_name = "..."` and `transformer = "..."`. The transformer applies
/// to every method in the block unless a method names its own.
///
/// # Panics
///
/// Expanding panics on a trait `impl`. Only inherent ones are supported.
#[proc_macro_attribute]
pub fn intuicio_methods(attributes: TokenStream, input: TokenStream) -> TokenStream {
    crate::methods::intuicio_methods(attributes, input)
}

/// Marks a method inside an [`macro@intuicio_methods`] block for exposure.
///
/// Expands to the method unchanged. It exists so that the surrounding
/// attribute can read its arguments.
///
/// # Attributes
///
/// `name`, `use_registry`, `use_context`, `transformer`, `dependency`,
/// `meta`, `args_meta(...)` and `debug`, all as on
/// [`macro@intuicio_function`]. `dependency = "this"` is the usual way to tie
/// a returned reference to the receiver it came from.
#[proc_macro_attribute]
pub fn intuicio_method(_: TokenStream, input: TokenStream) -> TokenStream {
    input
}
