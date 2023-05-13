use glutin::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Fullscreen, Window as GlutinWindow, WindowBuilder},
    ContextBuilder, ContextWrapper, PossiblyCurrent,
};
use intuicio_core::{core_version, registry::Registry, IntuicioStruct, IntuicioVersion};
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};
use intuicio_frontend_simpleton::{Boolean, Integer, Reference, Text};

struct WindowState {
    event_loop: EventLoop<()>,
    context_wrapper: ContextWrapper<PossiblyCurrent, GlutinWindow>,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "WindowConfig", module_name = "window")]
pub struct WindowConfig {
    pub title: Reference,
    pub width: Reference,
    pub height: Reference,
    pub fullscreen: Reference,
    pub vsync: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Window", module_name = "window")]
pub struct Window {
    #[intuicio(ignore)]
    state: Option<WindowState>,
}

#[intuicio_methods(module_name = "window")]
impl Window {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, config: Reference) -> Reference {
        let (title, width, height, fullscreen, vsync) =
            if let Some(config) = config.read::<WindowConfig>() {
                let title = config
                    .title
                    .read::<Text>()
                    .map(|value| value.to_owned())
                    .unwrap_or("Simpleton Window".to_owned());
                let width = config
                    .width
                    .read::<Integer>()
                    .map(|value| *value)
                    .unwrap_or(1024);
                let height = config
                    .height
                    .read::<Integer>()
                    .map(|value| *value)
                    .unwrap_or(1024);
                let fullscreen = config
                    .fullscreen
                    .read::<Boolean>()
                    .map(|value| {
                        if *value {
                            Some(Fullscreen::Borderless(None))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                let vsync = config
                    .vsync
                    .read::<Boolean>()
                    .map(|value| *value)
                    .unwrap_or(false);
                (title, width, height, fullscreen, vsync)
            } else {
                ("Simpleton Window".to_owned(), 1024, 768, None, false)
            };
        let event_loop = EventLoop::new();
        let window_builder = WindowBuilder::new()
            .with_title(title.as_str())
            .with_inner_size(LogicalSize::new(width as u32, height as u32))
            .with_fullscreen(fullscreen);
        let context_wrapper = unsafe {
            ContextBuilder::new()
                .with_vsync(vsync)
                .build_windowed(window_builder, &event_loop)
                .unwrap()
                .make_current()
                .unwrap()
        };
        Reference::new(
            Window {
                state: Some(WindowState {
                    event_loop,
                    context_wrapper,
                }),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn maintain(registry: &Registry, mut window: Reference) -> Reference {
        let mut window = window.write::<Window>().expect("`window` is not a Window!");
        let state = window
            .state
            .as_mut()
            .expect("`window` doesn't have valid state!");
        let mut result = true;
        state.event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    result = false;
                }
                Event::MainEventsCleared => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => (),
            }
        });
        Reference::new_boolean(result, registry)
    }

    #[intuicio_method(use_registry)]
    pub fn get_proc_address(registry: &Registry, window: Reference, name: Reference) -> Reference {
        let window = window.read::<Window>().expect("`window` is not a Window!");
        let name = name.read::<Text>().expect("`name` is not a Text!");
        let state = window
            .state
            .as_ref()
            .expect("`window` doesn't have valid state!");
        Reference::new_integer(
            state.context_wrapper.get_proc_address(name.as_str()) as Integer,
            registry,
        )
    }
}

#[no_mangle]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[no_mangle]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_struct(WindowConfig::define_struct(registry));
    registry.add_struct(Window::define_struct(registry));
    registry.add_function(Window::new__define_function(registry));
    registry.add_function(Window::maintain__define_function(registry));
    registry.add_function(Window::get_proc_address__define_function(registry));
}
