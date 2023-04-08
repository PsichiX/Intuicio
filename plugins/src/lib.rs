use intuicio_core::registry::Registry;
use libloading::Library;
use std::{cell::RefCell, collections::HashMap};

thread_local! {
    static LIBRARIES: RefCell<HashMap<String, Library>> = Default::default();
}

pub fn install_plugin(
    path: &str,
    registry: &mut Registry,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let library = Library::new(path)?;
        let functor = library.get::<unsafe extern "C" fn(&mut Registry)>(b"install\0")?;
        functor(registry);
        LIBRARIES.with(|map| map.borrow_mut().insert(path.to_owned(), library));
        Ok(())
    }
}
