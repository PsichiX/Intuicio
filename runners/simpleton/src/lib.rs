mod library;

use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_simpleton::{
    library::jobs::Jobs,
    script::{SimpletonContentParser, SimpletonPackage, SimpletonScriptExpression},
    Integer, Reference,
};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub name: Option<String>,
    pub module_name: Option<String>,
    pub stack_capacity: Option<usize>,
    pub registers_capacity: Option<usize>,
    pub heap_page_capacity: Option<usize>,
    pub into_code: Option<String>,
    pub into_intuicio: Option<String>,
}

pub fn execute(entry: &str, config: Config, args: impl IntoIterator<Item = String>) -> i32 {
    let entry = if Path::new(entry).is_dir() {
        format!("{}/main.simp", entry)
    } else {
        entry.to_owned()
    };
    let mut content_provider = FileContentProvider::new("simp", SimpletonContentParser);
    let package = SimpletonPackage::new(&entry, &mut content_provider).unwrap();
    if let Some(path) = &config.into_code {
        std::fs::write(path, format!("{:#?}", package)).unwrap();
    }
    if let Some(path) = &config.into_intuicio {
        std::fs::write(path, format!("{:#?}", package.compile())).unwrap();
    }
    if config.into_code.is_some() || config.into_intuicio.is_some() {
        return 0;
    }
    let stack_capacity = config.stack_capacity.unwrap_or(1024);
    let registers_capacity = config.registers_capacity.unwrap_or(1024);
    let heap_page_capacity = config.heap_page_capacity.unwrap_or(1024);
    let host_producer = HostProducer::new(move || {
        let mut registry = Registry::default();
        intuicio_frontend_simpleton::library::install(&mut registry);
        crate::library::install(&mut registry);
        let package = package.compile();
        package.install::<VmScope<SimpletonScriptExpression>>(&mut registry, None);
        let context = Context::new(stack_capacity, registers_capacity, heap_page_capacity);
        Host::new(context, registry.into())
    });
    let mut host = host_producer.produce();
    host.context()
        .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
    let args = Reference::new_array(
        args.into_iter()
            .map(|arg| Reference::new_text(arg, host.registry()))
            .collect(),
        host.registry(),
    );
    host.call_function::<(Reference,), _>(
        &config.name.unwrap_or_else(|| "main".to_owned()),
        &config.module_name.unwrap_or_else(|| "main".to_owned()),
        None,
    )
    .unwrap()
    .run((args,))
    .0
    .read::<Integer>()
    .map(|result| *result as i32)
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_runner() {
        assert_eq!(
            execute("./resources/examples", Config::default(), vec![]),
            0
        );
    }
}
