mod library;

use clap::Parser;
use intuicio_backend_vm::scope::VmScope;
use intuicio_core::{
    context::Context,
    host::{Host, HostProducer},
    registry::Registry,
    script::{ExtensionContentProvider, FileContentProvider, IgnoreContentProvider},
};
use intuicio_frontend_simpleton::{
    Reference,
    library::jobs::Jobs,
    script::{
        SimpletonBinaryFileContentProvider, SimpletonContentParser, SimpletonModule,
        SimpletonPackage, SimpletonScriptExpression,
    },
};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Entry script file path.
    #[arg(value_name = "PATH")]
    entry: String,

    /// Additional script arguments.
    #[arg(value_name = "PATH")]
    args: Vec<String>,

    /// Additional root path to binary plugins.
    #[arg(short, long, value_name = "PATH")]
    plugins: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let mut content_provider = ExtensionContentProvider::<SimpletonModule>::default()
        .extension(
            "simp",
            FileContentProvider::new("simp", SimpletonContentParser),
        )
        .extension("bimp", SimpletonBinaryFileContentProvider::new("bimp"))
        .extension("plugin", IgnoreContentProvider)
        .default_extension("simp");
    let package = SimpletonPackage::new(&cli.entry, &mut content_provider).unwrap();
    let host_producer = HostProducer::new(move || {
        let simpleton_packages_dir = dirs::data_dir()
            .unwrap()
            .join(".simpleton")
            .join("packages")
            .to_string_lossy()
            .to_string();
        let alchemyst_packages_dir = dirs::data_dir()
            .unwrap()
            .join(".alchemyst")
            .join("packages")
            .to_string_lossy()
            .to_string();
        let mut plugin_search_paths = vec![
            "./",
            simpleton_packages_dir.as_str(),
            alchemyst_packages_dir.as_str(),
        ];
        if let Some(path) = &cli.plugins {
            plugin_search_paths.push(path.as_str());
        }
        let mut registry = Registry::default()
            .with_index_capacity(256)
            .with_use_indexing_threshold(256);
        intuicio_frontend_simpleton::library::install(&mut registry);
        crate::library::install(&mut registry);
        package.install_plugins(&mut registry, &plugin_search_paths);
        package
            .compile()
            .install::<VmScope<SimpletonScriptExpression>>(&mut registry, None);
        let context = Context::new(1024 * 128, 1024 * 128);
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
    host.call_function::<(Reference,), _>("main", "main", None)
        .unwrap()
        .run((args,));
}
