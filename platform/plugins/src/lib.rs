//! Loading Intuicio plugins from dynamic libraries.
//!
//! A plugin is a `cdylib` that exports two C functions:
//!
//! ```ignore
//! #[unsafe(no_mangle)]
//! pub extern "C" fn version() -> IntuicioVersion { core_version() }
//!
//! #[unsafe(no_mangle)]
//! pub extern "C" fn install(registry: &mut Registry) { /* register here */ }
//! ```
//!
//! [`install_plugin`] loads the library, compares the two versions and calls
//! `install`. A loaded library is kept alive for the rest of the thread, since
//! the registry now holds pointers into it.
use intuicio_core::{IntuicioVersion, crate_version, registry::Registry};
use libloading::Library;
use std::{cell::RefCell, collections::HashMap};

thread_local! {
    static LIBRARIES: RefCell<HashMap<String, Library>> = Default::default();
}

/// A plugin was built against an incompatible version of the platform.
#[derive(Debug, Copy, Clone)]
pub struct IncompatibleVersionsError {
    /// Version the host reported.
    pub host: IntuicioVersion,
    /// Version the plugin reported.
    pub plugin: IntuicioVersion,
}

impl std::fmt::Display for IncompatibleVersionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Incompatible host ({}) and plugin ({}) versions!",
            self.host, self.plugin
        )
    }
}

impl std::error::Error for IncompatibleVersionsError {}

/// Loads the plugin at `path` and lets it register its types and functions.
///
/// `host_version` defaults to [`plugins_version`]. The plugin is rejected when
/// its major and minor numbers differ from the host ones.
///
/// The library stays loaded for the life of the calling thread, keyed by
/// `path`. Loading the same path twice replaces the entry, so the second load
/// drops the first library.
///
/// # Errors
///
/// Fails when the library cannot be loaded, when it exports no `version` or
/// `install` symbol, or with [`IncompatibleVersionsError`] on a version
/// mismatch.
pub fn install_plugin(
    path: &str,
    registry: &mut Registry,
    host_version: Option<IntuicioVersion>,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let host_version = host_version.unwrap_or_else(plugins_version);
        let library = Library::new(path)?;
        let version = library.get::<unsafe extern "C" fn() -> IntuicioVersion>(b"version\0")?;
        let plugin_version = version();
        if !host_version.is_compatible(&plugin_version) {
            return Err(Box::new(IncompatibleVersionsError {
                host: host_version,
                plugin: plugin_version,
            }));
        }
        let install = library.get::<unsafe extern "C" fn(&mut Registry)>(b"install\0")?;
        install(registry);
        LIBRARIES.with(|map| map.borrow_mut().insert(path.to_owned(), library));
        Ok(())
    }
}

/// Returns the version of this crate, which plugins are checked against.
pub fn plugins_version() -> IntuicioVersion {
    crate_version!()
}
