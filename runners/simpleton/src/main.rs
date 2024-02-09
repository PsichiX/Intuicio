mod library;

use clap::{Parser, Subcommand};
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_frontend_simpleton::prelude::{jobs::Jobs, *};
use std::path::{Path, PathBuf};

const ENTRY_DIR: &str = "entry-dir";

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

    /// Registry indexing capacity.
    #[arg(long, value_name = "COUNT")]
    indexing_capacity: Option<usize>,

    /// Registry use indexing threshold.
    #[arg(long, value_name = "COUNT")]
    use_indexing_threshold: Option<usize>,

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
    #[arg(short, long)]
    show_cli: bool,

    /// Additional root path to binary plugins.
    #[arg(short, long, value_name = "PATH")]
    plugins: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Writes generated simpleton code into specified file.
    Code {
        /// Path to file.
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Writes generated intuicio code into specified file.
    Intuicio {
        /// Path to file.
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Writes generated binary code into specified file.
    Binary {
        /// Path to file.
        #[arg(value_name = "PATH")]
        path: String,
    },
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
        .extension("bimp", SimpletonBinaryFileContentProvider::new("bimp"))
        .extension("plugin", IgnoreContentProvider)
        .default_extension("simp");
    let package = SimpletonPackage::new(&entry, &mut content_provider).unwrap();
    if let Some(command) = cli.command {
        match command {
            Commands::Code { path } => {
                std::fs::write(path, format!("{:#?}", package)).unwrap();
            }
            Commands::Intuicio { path } => {
                std::fs::write(path, format!("{:#?}", package.compile())).unwrap();
            }
            Commands::Binary { path } => {
                let bytes = SimpletonBinary::archive(package, |path| {
                    path.ends_with(".plugin") || path.ends_with(".bimp")
                })
                .unwrap();
                std::fs::write(path, bytes).unwrap();
            }
        }
        return;
    }
    let indexing_capacity = cli.indexing_capacity.unwrap_or(256);
    let use_indexing_threshold = cli.use_indexing_threshold.unwrap_or(256);
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
        let mut registry = Registry::default()
            .with_index_capacity(indexing_capacity)
            .with_use_indexing_threshold(use_indexing_threshold);
        intuicio_frontend_simpleton::library::install(&mut registry);
        crate::library::install(&mut registry);
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
    host.context().set_custom(ENTRY_DIR, {
        let mut result = PathBuf::from(entry);
        result.pop();
        result.to_string_lossy().to_string()
    });
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
