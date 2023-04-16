use super::{
    color::Color,
    engine::Engine,
    font::Font,
    image::Image,
    rect::{Rect, TetraRect},
    vec2::{TetraVec2, Vec2},
};
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::{
    library::{closure::Closure, event::Event, promise::Promise, reflect},
    Function, *,
};
use tetra::{
    graphics::{text::Text as TetraText, DrawParams, NineSlice as TetraNineSlice},
    input::get_mouse_position,
    window,
};

macro_rules! read {
    ($( $name:ident : $type:ty ),*) => {
        #[allow(unused_mut)]
        let ( $( mut $name , )* ) = ( $( $name.read::<$type>().unwrap() , )* );
    };
}

macro_rules! write {
    ($( $name:ident : $type:ty ),*) => {
        #[allow(unused_mut)]
        let ( $( mut $name , )* ) = ( $( $name.write::<$type>().unwrap() , )* );
    };
}

macro_rules! read_deref {
    ($( $name:ident : $type:ty $(as $type_cast:ty)? ),*) => {
        #[allow(unused_mut)]
        let ( $( mut $name , )* ) = ( $( *$name.read::<$type>().unwrap() $(as $type_cast)? , )* );
    };
}

macro_rules! read_tetra {
    ($( $name:ident : $type:ty ),*) => {
        #[allow(unused_mut)]
        let ( $( mut $name , )* ) = ( $( $name.read::<$type>().unwrap().to_tetra() , )* );
    };
}

macro_rules! execute_gui {
    ($layout:expr, $input_consumed:expr, $executor:expr, $context:expr, $registry:expr) => {{
        let result = Reference::new(Self { layout: $layout }, $registry);
        if $executor.read::<Function>().is_some() {
            reflect::call(
                $context,
                $registry,
                $executor.clone(),
                Reference::new_array(vec![result.clone()], $registry),
            );
        } else if let Some(executor) = $executor.read::<Closure>() {
            executor.invoke($context, $registry, &[result.clone()]);
        } else if $executor.read::<Promise>().is_some() {
            Promise::resolve($context, $registry, $executor.clone(), result.clone());
        } else if $executor.read::<Event>().is_some() {
            Event::dispatch(
                $context,
                $registry,
                $executor.clone(),
                Reference::new_array(vec![result.clone()], $registry),
            );
        }
        result
    }};
}

macro_rules! execute_gui_item {
    ($item:expr, $layout:expr, $input_consumed:expr, $executor:expr, $context:expr, $registry:expr) => {{
        let result = Reference::new(Self { layout: $layout }, $registry);
        if $executor.read::<Function>().is_some() {
            reflect::call(
                $context,
                $registry,
                $executor.clone(),
                Reference::new_array(vec![result.clone(), $item], $registry),
            );
        } else if let Some(executor) = $executor.read::<Closure>() {
            executor.invoke($context, $registry, &[result.clone(), $item]);
        } else if $executor.read::<Promise>().is_some() {
            Promise::resolve(
                $context,
                $registry,
                $executor.clone(),
                Reference::new_map(map! {gui: result.clone(), item: $item}, $registry),
            );
        } else if $executor.read::<Event>().is_some() {
            Event::dispatch(
                $context,
                $registry,
                $executor.clone(),
                Reference::new_array(vec![result.clone(), $item], $registry),
            );
        }
        result
    }};
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "NineSlice", module_name = "gui", override_send = true)]
pub struct NineSlice {
    pub region: Reference,
    pub left: Reference,
    pub right: Reference,
    pub top: Reference,
    pub bottom: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Gui", module_name = "gui")]
pub struct Gui {
    #[intuicio(ignore)]
    layout: TetraRect,
}

#[intuicio_methods(module_name = "gui")]
impl Gui {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, layout: Reference) -> Reference {
        Reference::new(
            Self {
                layout: layout.read::<Rect>().unwrap().to_tetra(),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn screen(registry: &Registry, engine: Reference) -> Reference {
        let engine = engine.read::<Engine>().unwrap();
        let ctx = engine.tetra_context.as_ref().unwrap();
        let ctx = ctx.read().unwrap();
        Reference::new(
            Self {
                layout: TetraRect {
                    x: 0.0,
                    y: 0.0,
                    width: window::get_width(&ctx) as f32,
                    height: window::get_height(&ctx) as f32,
                },
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn clone(registry: &Registry, gui: Reference) -> Reference {
        read!(gui: Gui);
        Reference::new(Self { layout: gui.layout }, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn layout(registry: &Registry, gui: Reference) -> Reference {
        Reference::new(
            Rect::from_tetra(gui.read::<Gui>().unwrap().layout, registry),
            registry,
        )
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn freeform_aligned(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        size: Reference,
        alignment: Reference,
        pivot: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_tetra!(size: Vec2, alignment: Vec2, pivot: Vec2);
        let layout = TetraRect {
            x: gui.layout.x + gui.layout.width * pivot.x - size.x * pivot.x,
            y: gui.layout.y + gui.layout.height * alignment.y - size.y * pivot.y,
            width: size.x,
            height: size.y,
        };
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[allow(clippy::too_many_arguments)]
    #[intuicio_method(use_context, use_registry)]
    pub fn freeform_at(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        size: Reference,
        position: Reference,
        pivot: Reference,
        relative: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_tetra!(size: Vec2, position: Vec2, pivot: Vec2);
        let mut layout = TetraRect {
            x: position.x - size.x * pivot.x,
            y: position.y - size.y * pivot.y,
            width: size.x,
            height: size.y,
        };
        if *relative.read::<Boolean>().unwrap() {
            layout.x += gui.layout.x;
            layout.y += gui.layout.y;
        }
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[allow(clippy::too_many_arguments)]
    #[intuicio_method(use_context, use_registry)]
    pub fn margin(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        left: Reference,
        right: Reference,
        top: Reference,
        bottom: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_deref!(
            left: Real as f32,
            right: Real as f32,
            top: Real as f32,
            bottom: Real as f32
        );
        let mut horizontal = left + right;
        if horizontal > gui.layout.width {
            left = gui.layout.width * 0.5;
            horizontal = gui.layout.width;
        }
        let mut vertical = top + bottom;
        if vertical > gui.layout.height {
            top = gui.layout.height * 0.5;
            vertical = gui.layout.height;
        }
        let layout = TetraRect {
            x: gui.layout.x + left,
            y: gui.layout.y + top,
            width: gui.layout.width - horizontal,
            height: gui.layout.height - vertical,
        };
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn offset(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        value: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_tetra!(value: Vec2);
        let layout = TetraRect {
            x: gui.layout.x + value.x,
            y: gui.layout.y + value.y,
            width: gui.layout.width,
            height: gui.layout.height,
        };
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn scale(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        value: Reference,
        alignment: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_tetra!(value: Vec2, alignment: Vec2);
        let width = gui.layout.width * value.x;
        let height = gui.layout.height * value.y;
        let horizontal_space = gui.layout.width - width;
        let vertical_space = gui.layout.height - height;
        let layout = TetraRect {
            x: gui.layout.x + horizontal_space * alignment.x,
            y: gui.layout.y + vertical_space * alignment.y,
            width,
            height,
        };
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn section(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        relative: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self);
        read_tetra!(relative: Rect);
        let layout = TetraRect {
            x: gui.layout.x + relative.x,
            y: gui.layout.y + relative.y,
            width: relative.width,
            height: relative.height,
        };
        execute_gui!(layout, gui.input_consumed, executor, context, registry)
    }

    #[allow(clippy::too_many_arguments)]
    #[intuicio_method(use_context, use_registry)]
    pub fn vertical_list(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        array: Reference,
        item_height: Reference,
        separation: Reference,
        alignment: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self, array: Array);
        read_deref!(
            item_height: Real as f32,
            separation: Real as f32,
            alignment: Real as f32
        );
        let length =
            item_height * array.len() as f32 + separation * array.len().saturating_sub(1) as f32;
        let mut y = gui.layout.y - length * alignment;
        for item in array.iter() {
            let layout = TetraRect {
                x: gui.layout.x,
                y,
                width: gui.layout.width,
                height: item_height,
            };
            execute_gui_item!(
                item.clone(),
                layout,
                gui.input_consumed,
                executor,
                context,
                registry
            );
            y += item_height + separation;
        }
        Reference::new(Self { layout: gui.layout }, registry)
    }

    #[allow(clippy::too_many_arguments)]
    #[intuicio_method(use_context, use_registry)]
    pub fn horizontal_list(
        context: &mut Context,
        registry: &Registry,
        gui: Reference,
        array: Reference,
        item_width: Reference,
        separation: Reference,
        alignment: Reference,
        executor: Reference,
    ) -> Reference {
        read!(gui: Self, array: Array);
        read_deref!(
            item_width: Real as f32,
            separation: Real as f32,
            alignment: Real as f32
        );
        let length =
            item_width * array.len() as f32 + separation * array.len().saturating_sub(1) as f32;
        let mut x = gui.layout.x - length * alignment;
        for item in array.iter() {
            let layout = TetraRect {
                x,
                y: gui.layout.y,
                width: item_width,
                height: gui.layout.height,
            };
            execute_gui_item!(
                item.clone(),
                layout,
                gui.input_consumed,
                executor,
                context,
                registry
            );
            x += item_width + separation;
        }
        Reference::new(Self { layout: gui.layout }, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn cut_left(
        context: &mut Context,
        registry: &Registry,
        mut gui: Reference,
        value: Reference,
        executor: Reference,
    ) -> Reference {
        let layout = {
            write!(gui: Self);
            read_deref!(value: Real as f32);
            let layout = TetraRect {
                x: gui.layout.x,
                y: gui.layout.y,
                width: value,
                height: gui.layout.height,
            };
            gui.layout.x += value;
            layout
        };
        execute_gui!(layout, input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn cut_right(
        context: &mut Context,
        registry: &Registry,
        mut gui: Reference,
        value: Reference,
        executor: Reference,
    ) -> Reference {
        let layout = {
            write!(gui: Self);
            read_deref!(value: Real as f32);
            let layout = TetraRect {
                x: gui.layout.x + gui.layout.width - value,
                y: gui.layout.y,
                width: value,
                height: gui.layout.height,
            };
            gui.layout.width -= value;
            layout
        };
        execute_gui!(layout, input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn cut_top(
        context: &mut Context,
        registry: &Registry,
        mut gui: Reference,
        value: Reference,
        executor: Reference,
    ) -> Reference {
        let layout = {
            write!(gui: Self);
            read_deref!(value: Real as f32);
            let layout = TetraRect {
                x: gui.layout.x,
                y: gui.layout.y,
                width: gui.layout.width,
                height: value,
            };
            gui.layout.y += value;
            layout
        };
        execute_gui!(layout, input_consumed, executor, context, registry)
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn cut_bottom(
        context: &mut Context,
        registry: &Registry,
        mut gui: Reference,
        value: Reference,
        executor: Reference,
    ) -> Reference {
        let layout = {
            write!(gui: Self);
            read_deref!(value: Real as f32);
            let layout = TetraRect {
                x: gui.layout.x,
                y: gui.layout.y + gui.layout.height - value,
                width: gui.layout.width,
                height: value,
            };
            gui.layout.height -= value;
            layout
        };
        execute_gui!(layout, input_consumed, executor, context, registry)
    }

    #[intuicio_method()]
    pub fn image(
        mut engine: Reference,
        gui: Reference,
        image: Reference,
        color: Reference,
        nineslice: Reference,
    ) -> Reference {
        write!(engine: Engine);
        read!(gui: Self, image: Image);
        read_tetra!(color: Color);
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let position = gui.layout.top_left();
        let size = image.texture.as_ref().unwrap().size();
        if let Some(nineslice) = nineslice.read::<NineSlice>() {
            image.texture.as_ref().unwrap().draw_nine_slice(
                &mut ctx,
                &TetraNineSlice {
                    region: nineslice
                        .region
                        .read::<Rect>()
                        .map(|rect| rect.to_tetra())
                        .unwrap_or_else(|| TetraRect {
                            x: 0.0,
                            y: 0.0,
                            width: size.0 as f32,
                            height: size.0 as f32,
                        }),
                    left: nineslice
                        .left
                        .read::<Real>()
                        .map(|value| *value as f32)
                        .unwrap_or_default(),
                    right: nineslice
                        .right
                        .read::<Real>()
                        .map(|value| *value as f32)
                        .unwrap_or_default(),
                    top: nineslice
                        .top
                        .read::<Real>()
                        .map(|value| *value as f32)
                        .unwrap_or_default(),
                    bottom: nineslice
                        .bottom
                        .read::<Real>()
                        .map(|value| *value as f32)
                        .unwrap_or_default(),
                },
                gui.layout.width,
                gui.layout.height,
                DrawParams {
                    position,
                    color,
                    ..Default::default()
                },
            );
        } else {
            image.texture.as_ref().unwrap().draw(
                &mut ctx,
                DrawParams {
                    position,
                    scale: TetraVec2::new(gui.layout.width, gui.layout.height)
                        / TetraVec2::new(size.0 as f32, size.1 as f32),
                    color,
                    ..Default::default()
                },
            );
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn text(
        mut engine: Reference,
        gui: Reference,
        font: Reference,
        color: Reference,
        alignment: Reference,
        text: Reference,
    ) -> Reference {
        write!(engine: Engine);
        read!(gui: Self, font: Font, text: Text);
        read_tetra!(color: Color, alignment: Vec2);
        let ctx = engine.tetra_context.as_mut().unwrap();
        let mut ctx = ctx.write().unwrap();
        let position = gui.layout.top_left();
        let container_size = TetraVec2::new(gui.layout.width, gui.layout.height);
        let mut renderable = TetraText::new(text.as_str(), font.font.as_ref().unwrap().clone());
        renderable.set_max_width(Some(gui.layout.width));
        let size = renderable
            .get_bounds(&mut ctx)
            .map(|rect| TetraVec2::new(rect.width, rect.height))
            .unwrap_or_default();
        let position = TetraVec2::lerp(position, position + container_size - size, alignment);
        renderable.draw(
            &mut ctx,
            DrawParams {
                position,
                color,
                ..Default::default()
            },
        );
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn hover(registry: &Registry, engine: Reference, gui: Reference) -> Reference {
        read!(engine: Engine, gui: Self);
        let ctx = engine.tetra_context.as_ref().unwrap();
        let ctx = ctx.read().unwrap();
        let point = get_mouse_position(&ctx);
        Reference::new_boolean(gui.layout.contains_point(point), registry)
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Gui::define_struct(registry));
    registry.add_struct(NineSlice::define_struct(registry));
    registry.add_function(Gui::new__define_function(registry));
    registry.add_function(Gui::screen__define_function(registry));
    registry.add_function(Gui::clone__define_function(registry));
    registry.add_function(Gui::layout__define_function(registry));
    registry.add_function(Gui::freeform_aligned__define_function(registry));
    registry.add_function(Gui::freeform_at__define_function(registry));
    registry.add_function(Gui::margin__define_function(registry));
    registry.add_function(Gui::offset__define_function(registry));
    registry.add_function(Gui::scale__define_function(registry));
    registry.add_function(Gui::section__define_function(registry));
    registry.add_function(Gui::vertical_list__define_function(registry));
    registry.add_function(Gui::horizontal_list__define_function(registry));
    registry.add_function(Gui::cut_left__define_function(registry));
    registry.add_function(Gui::cut_right__define_function(registry));
    registry.add_function(Gui::cut_top__define_function(registry));
    registry.add_function(Gui::cut_bottom__define_function(registry));
    registry.add_function(Gui::image__define_function(registry));
    registry.add_function(Gui::text__define_function(registry));
    registry.add_function(Gui::hover__define_function(registry));
}
