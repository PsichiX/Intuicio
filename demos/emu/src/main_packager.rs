mod cartridge;

use cartridge::{Assembly, Cartridge, CartridgeSprite, CartridgeTileset};
use clap::Parser;
use intuicio_core::script::FileContentProvider;
use intuicio_frontend_assembler::{AsmContentParser, AsmPackage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub title: String,
    pub assembly_entry: PathBuf,
    #[serde(default)]
    pub module_name: Option<String>,
    #[serde(default)]
    pub objects: usize,
    #[serde(default)]
    pub sprites: Vec<ManifestSprite>,
    #[serde(default)]
    pub tilesets: Vec<ManifestTileset>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ManifestSprite {
    pub file: PathBuf,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ManifestTileset {
    pub file: PathBuf,
    pub cell_width: usize,
    pub cell_height: usize,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input manifest file path.
    #[arg(value_name = "PATH")]
    input: PathBuf,

    /// Output cartrdge file path.
    #[arg(value_name = "PATH")]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let manifest = std::fs::read_to_string(cli.input).expect("Could not read manifest file!");
    let manifest = toml::from_str::<Manifest>(&manifest).expect("Could not parse manifest file!");
    let mut content_provider = FileContentProvider::new("iasm", AsmContentParser);
    let assembly = Assembly::from_package(
        AsmPackage::new(
            manifest.assembly_entry.to_string_lossy().as_ref(),
            &mut content_provider,
        )
        .expect("Could not compile assembly!"),
    );
    let result = Cartridge {
        title: manifest.title,
        assembly,
        module_name: manifest.module_name.unwrap_or_else(|| "main".to_owned()),
        objects: manifest.objects,
        sprites: manifest
            .sprites
            .into_iter()
            .map(|config| CartridgeSprite {
                width: config.width,
                height: config.height,
                bytes: std::fs::read(config.file).expect("Could not read sprite file!"),
            })
            .collect(),
        tilesets: manifest
            .tilesets
            .into_iter()
            .map(|config| CartridgeTileset {
                cell_width: config.cell_width,
                cell_height: config.cell_height,
                cols: config.cols,
                rows: config.rows,
                bytes: std::fs::read(config.file).expect("Could not read tileset file!"),
            })
            .collect(),
    }
    .into_bytes()
    .expect("Could not package cartridge!");
    std::fs::write(cli.output, result).expect("Could not write cartridge to file!");
}
