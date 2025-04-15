use intuicio_core::{IntuicioVersion, crate_version, registry::Registry};
use libloading::Library;
use std::{cell::RefCell, collections::HashMap};

thread_local! {
    static LIBRARIES: RefCell<HashMap<String, Library>> = Default::default();
}

#[derive(Debug, Copy, Clone)]
pub struct IncompatibleVersionsError {
    pub host: IntuicioVersion,
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

pub fn plugins_version() -> IntuicioVersion {
    crate_version!()
}
