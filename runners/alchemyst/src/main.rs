use clap::Parser;
use intuicio_core::prelude::*;
use intuicio_frontend_simpleton::prelude::*;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Entry script file path.
    #[arg(value_name = "PATH")]
    entry: String,

    /// Additional script arguments.
    #[arg(value_name = "PATH")]
    args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let mut content_provider = ExtensionContentProvider::<SimpletonModule>::default()
        .extension(
            "simp",
            FileContentProvider::new("simp", SimpletonContentParser),
        )
        .extension("plugin", IgnoreContentProvider)
        .default_extension("simp");
    alchemyst::execute(&cli.entry, cli.args, &mut content_provider);
}
