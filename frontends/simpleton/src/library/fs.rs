use crate::{library::bytes::Bytes, Array, Reference, Text};
use intuicio_core::registry::Registry;
use intuicio_derive::intuicio_function;
use std::path::{Path, PathBuf};

#[intuicio_function(module_name = "fs", use_registry)]
pub fn exists(registry: &Registry, path: Reference) -> Reference {
    Reference::new_boolean(
        Path::new(path.read::<Text>().unwrap().as_str()).exists(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn is_file(registry: &Registry, path: Reference) -> Reference {
    Reference::new_boolean(
        Path::new(path.read::<Text>().unwrap().as_str()).is_file(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn is_dir(registry: &Registry, path: Reference) -> Reference {
    Reference::new_boolean(
        Path::new(path.read::<Text>().unwrap().as_str()).is_dir(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn make_dir(registry: &Registry, path: Reference) -> Reference {
    Reference::new_boolean(
        std::fs::create_dir_all(path.read::<Text>().unwrap().as_str()).is_ok(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn scan_dir(registry: &Registry, path: Reference) -> Reference {
    let result = std::fs::read_dir(path.read::<Text>().unwrap().as_str())
        .unwrap()
        .map(|entry| {
            Reference::new_text(
                entry.unwrap().path().to_string_lossy().as_ref().to_owned(),
                registry,
            )
        })
        .collect::<Array>();
    Reference::new_array(result, registry)
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn read_file(registry: &Registry, path: Reference) -> Reference {
    Reference::new(
        Bytes::new_raw(std::fs::read(path.read::<Text>().unwrap().as_str()).unwrap()),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn write_file(registry: &Registry, path: Reference, bytes: Reference) -> Reference {
    Reference::new_boolean(
        std::fs::write(
            path.read::<Text>().unwrap().as_str(),
            bytes.read::<Bytes>().unwrap().get_ref(),
        )
        .is_ok(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn delete(registry: &Registry, path: Reference) -> Reference {
    let path = PathBuf::from(path.read::<Text>().unwrap().as_str());
    if path.is_file() {
        Reference::new_boolean(std::fs::remove_file(path).is_ok(), registry)
    } else if path.is_dir() {
        Reference::new_boolean(std::fs::remove_dir_all(path).is_ok(), registry)
    } else {
        Reference::new_boolean(false, registry)
    }
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn get_current_dir(registry: &Registry) -> Reference {
    Reference::new_text(
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        registry,
    )
}

#[intuicio_function(module_name = "fs", use_registry)]
pub fn set_current_dir(registry: &Registry, path: Reference) -> Reference {
    Reference::new_boolean(
        std::env::set_current_dir(path.read::<Text>().unwrap().as_str()).is_ok(),
        registry,
    )
}

pub fn install(registry: &mut Registry) {
    registry.add_function(exists::define_function(registry));
    registry.add_function(is_file::define_function(registry));
    registry.add_function(is_dir::define_function(registry));
    registry.add_function(make_dir::define_function(registry));
    registry.add_function(scan_dir::define_function(registry));
    registry.add_function(read_file::define_function(registry));
    registry.add_function(write_file::define_function(registry));
    registry.add_function(delete::define_function(registry));
    registry.add_function(get_current_dir::define_function(registry));
    registry.add_function(set_current_dir::define_function(registry));
}
