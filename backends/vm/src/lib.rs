//! A backend that runs script data as it is, without compiling it first.
//!
//! A backend turns the operations a frontend produced into a callable function
//! body. This one keeps the operations and walks them one at a time, which
//! makes it a virtual machine rather than a compiler.
//!
//! [`scope::VmScope`] is the interpreter, and it is also the
//! `ScriptFunctionGenerator` for this backend, so a whole package installs with
//! it:
//!
//! ```ignore
//! package.install::<VmScope<MyExpression>>(&mut registry, None);
//! ```
//!
//! The `None` is what the generator takes as input, which here is an optional
//! debugger. See [`debugger`].
//!
//! # Suspending
//!
//! A script can stop in the middle with the `Suspend` operation.
//! [`scope::VmScope::run_until_suspended`] gives control back at that point, and
//! [`scope::VmScopeFuture`] wraps the same stepping in a [`Future`], so a script
//! can act as a coroutine.
//!
//! [`Future`]: std::future::Future
pub mod debugger;
pub mod scope;

use intuicio_core::{IntuicioVersion, crate_version};

/// Returns the version of this crate, for plugins to check against.
pub fn backend_vm_version() -> IntuicioVersion {
    crate_version!()
}
