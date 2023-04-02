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
        mesh::{IndexBuffer, Mesh, Vertex, VertexBuffer, VertexWinding},
        Color, DrawParams, Texture,
    },
    input::{self, Key},
    math::Vec2,
    time::Timestep,
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
        for (map_index, tile_index) in self
            .tiles
            .iter()
            .copied()
            .filter_map(|tile_index| (tile_index as usize).checked_sub(1))
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
                    x: col as f32 / tileset.cols as f32,
                    y: row as f32 / tileset.rows as f32,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32 + tile_width,
                    y: y as f32,
                },
                uv: Vec2 {
                    x: (col + 1) as f32 / tileset.cols as f32,
                    y: row as f32 / tileset.rows as f32,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32 + tile_width,
                    y: y as f32 + tile_height,
                },
                uv: Vec2 {
                    x: (col + 1) as f32 / tileset.cols as f32,
                    y: (row + 1) as f32 / tileset.rows as f32,
                },
                color: Color::WHITE,
            });
            vertices.push(Vertex {
                position: Vec2 {
                    x: x as f32,
                    y: y as f32 + tile_height,
                },
                uv: Vec2 {
                    x: col as f32 / tileset.cols as f32,
                    y: (row + 1) as f32 / tileset.rows as f32,
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
        result.set_front_face_winding(VertexWinding::Clockwise);
        result
    }
}

#[derive(Clone)]
struct Object {
    visible: bool,
    x: i16,
    y: i16,
    scale_x: i16,
    scale_y: i16,
    sprite: usize,
}

impl Default for Object {
    fn default() -> Self {
        Self {
            visible: false,
            x: 0,
            y: 0,
            scale_x: 1,
            scale_y: 1,
            sprite: 0,
        }
    }
}

pub struct Memory {
    sprites: Vec<Texture>,
    tilesets: Vec<Tileset>,
    objects: Vec<Object>,
    tilemap: Option<Tilemap>,
    input_flags: i8,
    camera_offset: (i16, i16),
}

struct GameState {
    module_name: String,
    host: Host,
    memory: Shared<Memory>,
}

impl State for GameState {
    fn update(&mut self, ctx: &mut TetraContext) -> tetra::Result {
        if let Some(mut memory) = self.memory.write() {
            memory.input_flags = 0;
            if input::is_key_down(ctx, Key::W) || input::is_key_down(ctx, Key::Up) {
                memory.input_flags |= 1 << 0;
            }
            if input::is_key_down(ctx, Key::S) || input::is_key_down(ctx, Key::Down) {
                memory.input_flags |= 1 << 1;
            }
            if input::is_key_down(ctx, Key::A) || input::is_key_down(ctx, Key::Left) {
                memory.input_flags |= 1 << 2;
            }
            if input::is_key_down(ctx, Key::D) || input::is_key_down(ctx, Key::Right) {
                memory.input_flags |= 1 << 3;
            }
            if input::is_key_down(ctx, Key::Space) || input::is_key_down(ctx, Key::Enter) {
                memory.input_flags |= 1 << 4;
            }
            if input::is_key_down(ctx, Key::LeftShift) || input::is_key_down(ctx, Key::Backspace) {
                memory.input_flags |= 1 << 5;
            }
        }
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
            let camera_offset =
                Vec2::new(memory.camera_offset.0 as f32, memory.camera_offset.1 as f32);
            if let Some(mesh) = memory
                .tilemap
                .as_ref()
                .and_then(|tilemap| tilemap.mesh.as_ref())
            {
                mesh.draw(
                    ctx,
                    DrawParams {
                        position: -camera_offset,
                        ..Default::default()
                    },
                )
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
                            position: Vec2::new(object.x as _, object.y as _) - camera_offset,
                            scale: Vec2::new(object.scale_x as _, object.scale_y as _),
                            origin: Vec2::new(
                                (texture.width() / 2) as f32,
                                (texture.height() / 2) as f32,
                            ),
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
        .timestep(Timestep::Fixed(30.0))
        .build()?
        .run(|ctx| {
            let memory = Shared::new(Memory {
                sprites: cartridge
                    .sprites
                    .iter()
                    .map(|sprite| {
                        Texture::from_encoded(ctx, &sprite.bytes)
                            .expect("Could not create texture!")
                    })
                    .collect(),
                tilesets: cartridge
                    .tilesets
                    .iter()
                    .map(|tileset| Tileset {
                        cell_width: tileset.cell_width,
                        cell_height: tileset.cell_height,
                        cols: tileset.cols,
                        rows: tileset.rows,
                        texture: Texture::from_encoded(ctx, &tileset.bytes)
                            .expect("Could not create texture!"),
                    })
                    .collect(),
                objects: vec![Object::default(); cartridge.objects],
                tilemap: None,
                input_flags: 0,
                camera_offset: (0, 0),
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
