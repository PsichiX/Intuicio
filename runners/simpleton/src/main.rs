use clap::Parser;
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_simpleton::prelude::{jobs::Jobs, *};
use std::path::Path;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Entry script file path.
    #[arg(value_name = "PATH")]
    entry: String,

    /// Additional script arguments.
    #[arg(value_name = "PATH")]
    args: Vec<String>,

    /// Function name to run.
    #[arg(short, long, value_name = "NAME")]
    name: Option<String>,

    /// Function module name to run.
    #[arg(short, long, value_name = "NAME")]
    module_name: Option<String>,

    /// VM stack capacity in bytes.
    #[arg(long, value_name = "BYTES")]
    stack_capacity: Option<usize>,

    /// VM registers capacity in bytes.
    #[arg(long, value_name = "BYTES")]
    registers_capacity: Option<usize>,

    /// VM heap page capacity in bytes.
    #[arg(long, value_name = "BYTES")]
    heap_page_capacity: Option<usize>,

    /// Prints CLI parameters.
    #[arg(short, long, value_name = "BYTES")]
    show_cli: bool,

    /// Writes generated simpleton code into specified file.
    #[arg(short, long, value_name = "PATH")]
    into_code: Option<String>,

    /// Writes generated intuicio code into specified file.
    #[arg(short, long, value_name = "PATH")]
    into_intuicio: Option<String>,

    /// Additional root path to binary plugins.
    #[arg(short, long, value_name = "PATH")]
    plugins: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if cli.show_cli {
        println!("CLI parameters: {:#?}", cli);
    }

    let entry = if Path::new(&cli.entry).is_dir() {
        format!("{}/main.simp", &cli.entry)
    } else {
        cli.entry.to_owned()
    };
    let mut content_provider = ExtensionContentProvider::<SimpletonModule>::default()
        .extension(
            "simp",
            FileContentProvider::new("simp", SimpletonContentParser),
        )
        .extension("plugin", IgnoreContentProvider)
        .default_extension("simp");
    let package = SimpletonPackage::new(&entry, &mut content_provider).unwrap();
    if let Some(path) = &cli.into_code {
        std::fs::write(path, format!("{:#?}", package)).unwrap();
    }
    if let Some(path) = &cli.into_intuicio {
        std::fs::write(path, format!("{:#?}", package.compile())).unwrap();
    }
    if cli.into_code.is_some() || cli.into_intuicio.is_some() {
        std::process::exit(0);
    }
    let stack_capacity = cli.stack_capacity.unwrap_or(1024);
    let registers_capacity = cli.registers_capacity.unwrap_or(1024);
    let heap_page_capacity = cli.heap_page_capacity.unwrap_or(1024);
    let host_producer = HostProducer::new(move || {
        let packages_dir = dirs::data_dir()
            .unwrap()
            .join(".simpleton")
            .join("packages")
            .to_string_lossy()
            .to_string();
        let mut plugin_search_paths = vec!["./", packages_dir.as_str()];
        if let Some(path) = &cli.plugins {
            plugin_search_paths.push(path.as_str());
        }
        let mut registry = Registry::default();
        intuicio_frontend_simpleton::library::install(&mut registry);
        package.install_plugins(&mut registry, &plugin_search_paths);
        package
            .compile()
            .install::<VmScope<SimpletonScriptExpression>>(&mut registry, None);
        let context = Context::new(stack_capacity, registers_capacity, heap_page_capacity);
        Host::new(context, registry.into())
    });
    let mut host = host_producer.produce();
    host.context()
        .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
    let args = Reference::new_array(
        cli.args
            .into_iter()
            .map(|arg| Reference::new_text(arg, host.registry()))
            .collect(),
        host.registry(),
    );
    let result = host
        .call_function::<(Reference,), _>(
            &cli.name.unwrap_or_else(|| "main".to_owned()),
            &cli.module_name.unwrap_or_else(|| "main".to_owned()),
            None,
        )
        .unwrap()
        .run((args,))
        .0
        .read::<Integer>()
        .map(|result| *result as i32)
        .unwrap_or(0);
    std::process::exit(result);
}
