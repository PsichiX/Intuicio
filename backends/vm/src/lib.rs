pub mod debugger;
pub mod scope;

pub mod prelude {
    pub use crate::{debugger::*, scope::*};
}

use intuicio_core::{crate_version, IntuicioVersion};

pub fn backend_vm_version() -> IntuicioVersion {
    crate_version!()
}
