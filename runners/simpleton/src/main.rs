use clap::Parser;
use intuicio_runner_simpleton::*;

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
}

fn main() {
    let cli = Cli::parse();
    if cli.show_cli {
        println!("CLI parameters: {:#?}", cli);
    }
    std::process::exit(execute(
        &cli.entry,
        Config {
            name: cli.name,
            module_name: cli.module_name,
            stack_capacity: cli.stack_capacity,
            registers_capacity: cli.registers_capacity,
            heap_page_capacity: cli.heap_page_capacity,
            into_code: cli.into_code,
            into_intuicio: cli.into_intuicio,
        },
        cli.args.into_iter(),
    ));
}
