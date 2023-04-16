use intuicio_frontend_assembler::*;
use pot::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Assembly {
    pub files: Vec<AsmFile>,
}

impl Assembly {
    #[allow(dead_code)]
    pub fn from_package(package: AsmPackage) -> Self {
        Self {
            files: package.files.into_values().collect(),
        }
    }

    #[allow(dead_code)]
    pub fn into_package(self) -> AsmPackage {
        AsmPackage {
            files: self
                .files
                .into_iter()
                .enumerate()
                .map(|(index, file)| (index.to_string(), file))
                .collect(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cartridge {
    pub title: String,
    pub assembly: Assembly,
    pub module_name: String,
    pub objects: usize,
    pub sprites: Vec<CartridgeSprite>,
    pub tilesets: Vec<CartridgeTileset>,
}

#[allow(dead_code)]
impl Cartridge {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        pot::from_slice(bytes)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        pot::to_vec(self)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CartridgeSprite {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CartridgeTileset {
    pub cell_width: usize,
    pub cell_height: usize,
    pub cols: usize,
    pub rows: usize,
    pub bytes: Vec<u8>,
}
