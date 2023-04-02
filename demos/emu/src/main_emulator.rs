mod cartridge;
mod library;

use crate::cartridge::Cartridge;
use clap::Parser;
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use intuicio_frontend_assembler::*;
use std::path::PathBuf;
use tetra::{
    graphics::{
        self,
        mesh::{IndexBuffer, Mesh, Vertex, VertexBuffer},
        Color, DrawParams, ImageData, Texture, TextureFormat,
    },
    math::Vec2,
    Context as TetraContext, ContextBuilder, State,
};

struct Tileset {
    cell_width: usize,
    cell_height: usize,
    cols: usize,
    rows: usize,
    texture: Texture,
}

struct Tilemap {
    cols: usize,
    tiles: Vec<i16>,
    tileset: usize,
    mesh: Option<Mesh>,
}

impl Tilemap {
    fn build_mesh(&self, tilesets: &[Tileset], ctx: &mut TetraContext) -> Mesh {
        let mut vertices = vec![];
        let mut indices = vec![];
        let tileset = &tilesets[self.tileset];
        let tile_width = tileset.cell_width as f32;
        let tile_height = tileset.cell_height as f32;
        let uv_width = 1.0 / (tileset.cols * tileset.cell_width) as f32;
        let uv_height = 1.0 / (tileset.rows * tileset.cell_height) as f32;
        for (map_index, tile_index) in self
            .tiles
            .iter()
            .copied()
            .filter_map(|tile_index| tile_index.checked_sub(1))
            .map(|tile_index| tile_index as usize)
            .enumerate()
        {
            let x = (map_index % self.cols) * tileset.cell_width;
            let y = (map_index / self.cols) * tileset.cell_height;
            let col = tile_index % tileset.cols;
            let row = tile_index / tileset.cols;
            let offset = vertices.len() as u32;
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32,
                    y: y as f32,
                },
                uv: Vec2 {
                    x: col as f32 * uv_width,
                    y: row as f32 * uv_height,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32 + tile_width,
                    y: y as f32,
                },
                uv: Vec2 {
                    x: (col + 1) as f32 * uv_width,
                    y: row as f32 * uv_height,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32 + tile_width,
                    y: y as f32 + tile_height,
                },
                uv: Vec2 {
                    x: (col + 1) as f32 * uv_width,
                    y: (row + 1) as f32 * uv_height,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32,
                    y: y as f32 + tile_height,
                },
                uv: Vec2 {
                    x: col as f32 * uv_width,
                    y: (row + 1) as f32 * uv_height,
                },
                color: Color::WHITE,
            });
            indices.push(offset);
            indices.push(offset + 1);
            indices.push(offset + 2);
            indices.push(offset + 2);
            indices.push(offset + 3);
            indices.push(offset);
        }
        let mut result = Mesh::indexed(
            VertexBuffer::new(ctx, &vertices).expect("Could not create vertex buffer!"),
            IndexBuffer::new(ctx, &indices).expect("Could not create index buffer!"),
        );
        result.set_texture(tileset.texture.clone());
        result
    }
}

#[derive(Default, Clone)]
struct Object {
    visible: bool,
    x: i16,
    y: i16,
    sprite: usize,
}

pub struct Memory {
    sprites: Vec<Texture>,
    tilesets: Vec<Tileset>,
    objects: Vec<Object>,
    tilemap: Option<Tilemap>,
}

struct GameState {
    module_name: String,
    host: Host,
    memory: Shared<Memory>,
}

impl State for GameState {
    fn update(&mut self, _: &mut TetraContext) -> tetra::Result {
        if let Some(call) = self
            .host
            .call_function::<(), ()>("tick", &self.module_name, None)
        {
            call.run(());
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut TetraContext) -> tetra::Result {
        graphics::clear(ctx, Color::rgb(0.0, 0.0, 0.0));
        if let Some(mut memory) = self.memory.write() {
            let mesh = memory
                .tilemap
                .as_ref()
                .filter(|tilemap| tilemap.mesh.is_none())
                .map(|tilemap| tilemap.build_mesh(&memory.tilesets, ctx));
            if let Some(mesh) = mesh {
                memory.tilemap.as_mut().unwrap().mesh = Some(mesh);
            }
            if let Some(mesh) = memory
                .tilemap
                .as_ref()
                .and_then(|tilemap| tilemap.mesh.as_ref())
            {
                mesh.draw(ctx, DrawParams::default())
            }
            for object in memory.objects.iter().filter(|object| object.visible) {
                if let Some(texture) = object
                    .sprite
                    .checked_sub(1)
                    .and_then(|sprite| memory.sprites.get(sprite))
                {
                    texture.draw(
                        ctx,
                        DrawParams {
                            position: Vec2::new(object.x as _, object.y as _),
                            ..Default::default()
                        },
                    )
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input cartridge file path.
    #[arg(value_name = "PATH")]
    input: PathBuf,
}

fn main() -> tetra::Result {
    let cli = Cli::parse();
    let cartridge = std::fs::read(cli.input).expect("Could not read cartridge file!");
    let cartridge = Cartridge::from_bytes(&cartridge).expect("Could not parse cartridge file!");
    ContextBuilder::new(&cartridge.title, 1024, 768)
        .fullscreen(false)
        .show_mouse(false)
        .quit_on_escape(true)
        .build()?
        .run(|ctx| {
            let memory = Shared::new(Memory {
                sprites: cartridge
                    .sprites
                    .iter()
                    .map(|sprite| {
                        let data = ImageData::from_data(
                            sprite.width as _,
                            sprite.height as _,
                            TextureFormat::Rgba8,
                            sprite.bytes.to_owned(),
                        )
                        .expect("Could not create image data!");
                        Texture::from_image_data(ctx, &data).expect("Could not create texture!")
                    })
                    .collect(),
                tilesets: cartridge
                    .tilesets
                    .iter()
                    .map(|tileset| {
                        let data = ImageData::from_data(
                            (tileset.cell_width * tileset.cols) as _,
                            (tileset.cell_height * tileset.rows) as _,
                            TextureFormat::Rgba8,
                            tileset.bytes.to_owned(),
                        )
                        .expect("Could not create image data!");
                        Tileset {
                            cell_width: tileset.cell_width,
                            cell_height: tileset.cell_height,
                            cols: tileset.cols,
                            rows: tileset.rows,
                            texture: Texture::from_image_data(ctx, &data)
                                .expect("Could not create texture!"),
                        }
                    })
                    .collect(),
                objects: vec![Object::default(); cartridge.objects],
                tilemap: None,
            });
            let mut registry = Registry::default();
            crate::library::install(&mut registry, memory.clone());
            cartridge
                .assembly
                .into_package()
                .compile()
                .install::<VmScope<AsmExpression>>(&mut registry, None);
            let mut context = Context::new(10240, 10240, 1024);
            context.heap().pages_count_limit = Some(10);
            let mut host = Host::new(context, registry.into());
            if let Some(call) =
                host.call_function::<(), ()>("bootload", &cartridge.module_name, None)
            {
                call.run(());
            }
            Ok(GameState {
                module_name: cartridge.module_name.to_owned(),
                host,
                memory,
            })
        })
}
