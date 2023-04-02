use crate::{Memory, Tilemap};
use intuicio_core::prelude::*;
use intuicio_data::prelude::*;

macro_rules! impl_type {
    ($registry:expr => $type:ty) => {
        $registry.add_function(define_function! {
            $registry => struct ($type) fn clone(value: $type) -> (original: $type, cloned: $type) {
                (value, value)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn negate(value: $type) -> (result: $type) {
                (-value,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn add(a: $type, b: $type) -> (result: $type) {
                (a + b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn sub(a: $type, b: $type) -> (result: $type) {
                (a - b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn mul(a: $type, b: $type) -> (result: $type) {
                (a * b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn div(a: $type, b: $type) -> (result: $type) {
                (a / b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn shl(a: $type, b: $type) -> (result: $type) {
                (a << b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn shr(a: $type, b: $type) -> (result: $type) {
                (a >> b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn and(a: $type, b: $type) -> (result: $type) {
                (a & b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn or(a: $type, b: $type) -> (result: $type) {
                (a | b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn xor(a: $type, b: $type) -> (result: $type) {
                (a ^ b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn eq(a: $type, b: $type) -> (result: bool) {
                (a == b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn neq(a: $type, b: $type) -> (result: bool) {
                (a != b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn lt(a: $type, b: $type) -> (result: bool) {
                (a < b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn lte(a: $type, b: $type) -> (result: bool) {
                (a <= b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn gt(a: $type, b: $type) -> (result: bool) {
                (a > b,)
            }
        });
        $registry.add_function(define_function! {
            $registry => struct ($type) fn gte(a: $type, b: $type) -> (result: bool) {
                (a >= b,)
            }
        });
    };
}

pub fn install(registry: &mut Registry, memory: Shared<Memory>) {
    registry.add_struct(NativeStructBuilder::new::<bool>().build());
    registry.add_struct(NativeStructBuilder::new::<i8>().build());
    registry.add_struct(NativeStructBuilder::new::<i16>().build());

    impl_type!(registry => i8);
    registry.add_function(define_function! {
        registry => struct (i8) fn to_i16(low: i8, high: i8) -> (result: i16) {
            let low = low as i16;
            let high = (high as i16) << 8;
            (low | high,)
        }
    });

    impl_type!(registry => i16);
    registry.add_function(define_function! {
        registry => struct (i16) fn to_i8(value: i16) -> (low: i8, high: i8) {
            let low = value & 0x00FF;
            #[allow(overflowing_literals)]
            let high = (value & 0xFF00) >> 8;
            (low as i8, high as i8)
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_show(object: i16) -> () {
            if let Some(mut memory) = memory_.write(){
                memory.objects[object as usize].visible = true;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_hide(object: i16) -> () {
            if let Some(mut memory) = memory_.write(){
                memory.objects[object as usize].visible = false;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_set_position(object: i16, x: i16, y: i16) -> () {
            if let Some(mut memory) = memory_.write(){
                let object = &mut memory.objects[object as usize];
                object.x = x;
                object.y = y;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn object_set_sprite(object: i16, sprite: i16) -> () {
            if let Some(mut memory) = memory_.write(){
                memory.objects[object as usize].sprite = sprite as usize;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_create(tileset: i16, cols: i16, rows: i16) -> () {
            if let Some(mut memory) = memory_.write(){
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
            if let Some(mut memory) = memory_.write(){
                memory.tilemap = None;
            }
        }
    });

    let memory_ = memory.clone();
    registry.add_function(define_function! {
        registry => fn tilemap_set_tile(col: i16, row: i16, tile: i16) -> () {
            if let Some(mut memory) = memory_.write(){
                if let Some(tilemap) = memory.tilemap.as_mut() {
                    tilemap.tiles[row as usize * tilemap.cols + col as usize] = tile;
                    tilemap.mesh = None;
                }
            }
        }
    });
}
