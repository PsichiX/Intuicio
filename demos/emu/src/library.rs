use crate::{Memory, Tilemap};
use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use rand::Rng;

macro_rules! impl_type {
    ($type:ty, $registry:expr, $memory:expr) => {
        let memory_ = $memory.clone();
        $registry.add_function(Function::new(
            function_signature! {
                $registry => type ($type) fn read_ram(address: i16) -> (result: $type)
            },
            FunctionBody::closure(move |context, _| {
                let address = context.stack().pop::<i16>().unwrap();
                let result = if let Some(memory) = memory_.read() {
                    if address as usize > memory.ram.len() - std::mem::size_of::<$type>() {
                        panic!(
                            "Trying to read address outside of RAM boundaries: {}",
                            address
                        );
                    }
                    unsafe {
                        memory
                            .ram
                            .as_ptr()
                            .add(address as usize)
                            .cast::<$type>()
                            .read()
                    }
                } else {
                    0
                };
                context.stack().push(result);
            }),
        ));
        let memory_ = $memory.clone();
        $registry.add_function(Function::new(
            function_signature! {
                $registry => type ($type) fn write_ram(address: i16, value: $type) -> ()
            },
            FunctionBody::closure(move |context, _| {
                let address = context.stack().pop::<i16>().unwrap();
                let value = context.stack().pop::<$type>().unwrap();
                if let Some(mut memory) = memory_.write() {
                    if address as usize > memory.ram.len() - std::mem::size_of::<$type>() {
                        panic!(
                            "Trying to write address outside of RAM boundaries: {}",
                            address
                        );
                    }
                    unsafe {
                        memory
                            .ram
                            .as_mut_ptr()
                            .add(address as usize)
                            .cast::<$type>()
                            .write(value);
                    }
                }
            }),
        ));
        $registry.add_function(define_function! {
            $registry => type ($type) fn clone(value: $type) -> (original: $type, cloned: $type) {
                (value, value)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn swap(a: $type, b: $type) -> (b: $type, a: $type) {
                (b, a)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn negate(value: $type) -> (result: $type) {
                (-value,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn add(a: $type, b: $type) -> (result: $type) {
                (a + b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn sub(a: $type, b: $type) -> (result: $type) {
                (a - b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn mul(a: $type, b: $type) -> (result: $type) {
                (a * b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn div(a: $type, b: $type) -> (result: $type) {
                (a / b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn abs(value: $type) -> (result: $type) {
                (value.abs(),)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn increment(value: $type) -> (result: $type) {
                (value + 1,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn decrement(value: $type) -> (result: $type) {
                (value - 1,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn modulo(a: $type, b: $type) -> (result: $type) {
                (a % b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn shl(a: $type, b: $type) -> (result: $type) {
                (a << b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn shr(a: $type, b: $type) -> (result: $type) {
                (a >> b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn and(a: $type, b: $type) -> (result: $type) {
                (a & b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn or(a: $type, b: $type) -> (result: $type) {
                (a | b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn xor(a: $type, b: $type) -> (result: $type) {
                (a ^ b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn eq(a: $type, b: $type) -> (result: bool) {
                (a == b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn neq(a: $type, b: $type) -> (result: bool) {
                (a != b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn lt(a: $type, b: $type) -> (result: bool) {
                (a < b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn lte(a: $type, b: $type) -> (result: bool) {
                (a <= b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn gt(a: $type, b: $type) -> (result: bool) {
                (a > b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn gte(a: $type, b: $type) -> (result: bool) {
                (a >= b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn random() -> (result: $type) {
                (rand::thread_rng().gen::<$type>(),)
            }
        });
        $registry.add_function(define_function! {
            $registry => type ($type) fn debug(value: $type) -> (result: $type) {
                println!("* {}: {}", std::any::type_name::<$type>(), value);
                (value,)
            }
        });
    };
}

pub fn install(registry: &mut Registry, memory: AsyncShared<Memory>) {
    registry.add_type(NativeStructBuilder::new::<bool>().build());
    registry.add_type(NativeStructBuilder::new::<i8>().build());
    registry.add_type(NativeStructBuilder::new::<i16>().build());

    registry.add_function(define_function! {
        registry => type (bool) fn clone(value: bool) -> (original: bool, cloned: bool) {
            (value, value)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn swap(a: bool, b: bool) -> (b: bool, a: bool) {
            (b, a)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn and(a: bool, b: bool) -> (result: bool) {
            (a && b,)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn or(a: bool, b: bool) -> (result: bool) {
            (a || b,)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn to_i8(value: bool) -> (result: i8) {
            (value as i8,)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn to_i16(value: bool) -> (result: i16) {
            (value as i16,)
        }
    });
    registry.add_function(define_function! {
        registry => type (bool) fn debug(value: bool) -> (result: bool) {
            println!("* bool: {}", value);
            (value,)
        }
    });

    impl_type!(i8, registry, memory);
    registry.add_function(define_function! {
        registry => type (i8) fn to_i16(low: i8, high: i8) -> (result: i16) {
            let low = low as i16;
            let high = (high as i16) << 8;
            (low | high,)
        }
    });

    impl_type!(i16, registry, memory);
    registry.add_function(define_function! {
        registry => type (i16) fn to_i8(value: i16) -> (low: i8, high: i8) {
            let low = value & 0x00FF;
            #[allow(overflowing_literals)]
            let high = (value & 0xFF00) >> 8;
            (low as i8, high as i8)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_visibility(object: i16) -> (result: bool) {
            (memory_.read().map(|memory| memory.objects[object as usize].visible).unwrap_or_default(),)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_show(object: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                memory.objects[object as usize].visible = true;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_hide(object: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                memory.objects[object as usize].visible = false;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_position(object: i16) -> (x: i16, y: i16) {
            memory_.read().map(|memory| {
                let object = &memory.objects[object as usize];
                (object.x, object.y)
            }).unwrap_or_default()
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_set_position(object: i16, x: i16, y: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                let object = &mut memory.objects[object as usize];
                object.x = x;
                object.y = y;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_scale(object: i16) -> (x: i16, y: i16) {
            memory_.read().map(|memory| {
                let object = &memory.objects[object as usize];
                (object.scale_x, object.scale_y)
            }).unwrap_or_default()
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_set_scale(object: i16, x: i16, y: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                let object = &mut memory.objects[object as usize];
                object.scale_x = x;
                object.scale_y = y;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_sprite(object: i16) -> (result: i16) {
            (memory_.read().map(|memory| memory.objects[object as usize].sprite).unwrap_or_default(),)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_set_sprite(object: i16, sprite: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                memory.objects[object as usize].sprite = sprite as usize;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_exists() -> (result: bool) {
            (memory_.read().map(|memory| memory.tilemap.is_some()).unwrap_or_default(),)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_create(tileset: i16, cols: i16, rows: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                memory.tilemap = Some(Tilemap {
                    cols: cols as usize,
                    tiles: vec![0; (cols * rows) as usize],
                    tileset: tileset as usize,
                    mesh: None,
                });
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_destroy() -> () {
            if let Some(mut memory) = memory_.write() {
                memory.tilemap = None;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_tile(col: i16, row: i16) -> (result: i16) {
            let result = memory_
                .read()
                .map(|memory| {
                    memory
                        .tilemap
                        .as_ref()
                        .map(|tilemap| tilemap.tiles[row as usize * tilemap.cols + col as usize])
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            (result,)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_set_tile(col: i16, row: i16, tile: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                if let Some(tilemap) = memory.tilemap.as_mut() {
                    tilemap.tiles[row as usize * tilemap.cols + col as usize] = tile;
                    tilemap.mesh = None;
                }
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_fill_tile(tile: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                if let Some(tilemap) = memory.tilemap.as_mut() {
                    for item in &mut tilemap.tiles {
                        *item = tile;
                    }
                    tilemap.mesh = None;
                }
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_region_tile(col: i16, row: i16, cols: i16, rows: i16, tile: i16) -> () {
            if let Some(mut memory) = memory_.write() {
                if let Some(tilemap) = memory.tilemap.as_mut() {
                    for x in col..(col + cols) {
                        for y in row..(row + rows) {
                            tilemap.tiles[y as usize * tilemap.cols + x as usize] = tile;
                        }
                    }
                    tilemap.mesh = None;
                }
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn input_flags() -> (flags: i8) {
            (memory_.read().map(|memory| memory.input_flags).unwrap_or_default(),)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn camera_offset() -> (x: i16, y: i16) {
            memory_.read().map(|memory| memory.camera_offset).unwrap_or_default()
        }
    });

    registry.add_function(define_function! {
        registry => fn set_camera_offset(x: i16, y: i16) -> () {
            if let Some(mut memory) = memory.write() {
                memory.camera_offset = (x, y);
            }
        }
    });
}
