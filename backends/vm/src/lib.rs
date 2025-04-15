pub mod debugger;
pub mod scope;

pub mod prelude {
    pub use crate::{debugger::*, scope::*};
}

use intuicio_core::{IntuicioVersion, crate_version};

pub fn backend_vm_version() -> IntuicioVersion {
    crate_version!()
}
