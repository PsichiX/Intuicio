use glow::{
    COLOR_BUFFER_BIT, Context as GlowContext, DEPTH_BUFFER_BIT, HasContext, STENCIL_BUFFER_BIT,
};
use glutin::{
    ContextBuilder, ContextWrapper, PossiblyCurrent,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Fullscreen, Window as GlutinWindow, WindowBuilder},
};
use intuicio_core::{
    IntuicioStruct, IntuicioVersion, context::Context, core_version, define_native_struct,
    registry::Registry,
};
use intuicio_data::managed::{Managed, ManagedRef, ManagedRefMut};
use intuicio_derive::{IntuicioStruct, intuicio_function, intuicio_method, intuicio_methods};
use intuicio_frontend_simpleton::{
    Boolean, Integer, Real, Reference, Text, library::event::Event as SimpletonEvent,
};
use std::time::Instant;

pub type Gl = Option<ManagedRef<GlowContext>>;
pub type WindowInterface = Option<ManagedRefMut<WindowInterfaceState>>;

struct WindowState {
    event_loop: EventLoop<()>,
    context_wrapper: ContextWrapper<PossiblyCurrent, GlutinWindow>,
    gl: Managed<GlowContext>,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "MouseInput", module_name = "window")]
pub struct MouseInput {
    pub state: Reference,
    pub button: Reference,
    pub x: Reference,
    pub y: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "KeyboardInput", module_name = "window")]
pub struct KeyboardInput {
    pub state: Reference,
    pub scancode: Reference,
    pub keycode: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "WindowConfig", module_name = "window")]
pub struct WindowConfig {
    pub title: Reference,
    pub width: Reference,
    pub height: Reference,
    pub fullscreen: Reference,
    pub vsync: Reference,
    pub fps: Reference,
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Window", module_name = "window")]
pub struct Window {
    #[intuicio(ignore)]
    state: Option<Box<WindowState>>,
    #[intuicio(ignore)]
    redraw_event: Reference,
    #[intuicio(ignore)]
    input_event: Reference,
    #[intuicio(ignore)]
    running: bool,
    /// (interval, accumulator)?
    #[intuicio(ignore)]
    redraw_interval: Option<(Real, Real)>,
}

#[intuicio_methods(module_name = "window")]
impl Window {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, config: Reference) -> Reference {
        let (title, width, height, fullscreen, vsync, fps) =
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
                    .unwrap_or(576);
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
                let fps = config.fps.read::<Integer>().map(|value| *value);
                (title, width, height, fullscreen, vsync, fps)
            } else {
                ("Simpleton Window".to_owned(), 1024, 576, None, false, None)
            };
        let event_loop = EventLoop::new();
        let window_builder = WindowBuilder::new()
            .with_title(title.as_str())
            .with_inner_size(LogicalSize::new(width as u32, height as u32))
            .with_fullscreen(fullscreen);
        let context_wrapper = unsafe {
            ContextBuilder::new()
                .with_vsync(vsync)
                .with_depth_buffer(24)
                .with_stencil_buffer(8)
                .with_double_buffer(Some(true))
                .with_hardware_acceleration(Some(true))
                .build_windowed(window_builder, &event_loop)
                .expect("Could not build windowed context wrapper!")
                .make_current()
                .expect("Could not make windowed context wrapper a current one!")
        };
        let gl = unsafe {
            GlowContext::from_loader_function(|name| {
                context_wrapper.get_proc_address(name) as *const _
            })
        };
        Reference::new(
            Window {
                state: Some(Box::new(WindowState {
                    event_loop,
                    context_wrapper,
                    gl: Managed::new(gl),
                })),
                redraw_event: Reference::new(SimpletonEvent::default(), registry),
                input_event: Reference::new(SimpletonEvent::default(), registry),
                running: false,
                redraw_interval: fps.map(|value| (1.0 / value as Real, 0.0)),
            },
            registry,
        )
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn run(context: &mut Context, registry: &Registry, mut window: Reference) -> Reference {
        let mut window = window.write::<Window>().expect("`window` is not a Window!");
        let mut state = match window.state.take() {
            Some(state) => state,
            None => return Reference::null(),
        };
        window.running = true;
        let mut timer = Instant::now();
        let mut redraw_timer = Instant::now();
        let mut mouse_position = PhysicalPosition { x: 0.0, y: 0.0 };
        let size = state.context_wrapper.window().inner_size();
        let mut interface = Managed::new(WindowInterfaceState {
            width: size.width as _,
            height: size.height as _,
            running: window.running,
            gl: state.gl.borrow(),
        });
        while window.running {
            state.event_loop.run_return(|event, _, control_flow| {
                *control_flow = ControlFlow::Poll;
                match event {
                    Event::MainEventsCleared => {
                        let redraw = if let Some((interval, accumulator)) =
                            window.redraw_interval.as_mut()
                        {
                            let delta_time = timer.elapsed().as_secs_f64();
                            timer = Instant::now();
                            *accumulator += delta_time;
                            if *accumulator >= *interval {
                                *accumulator %= *interval;
                                true
                            } else {
                                false
                            }
                        } else {
                            true
                        };
                        if redraw {
                            unsafe {
                                let size = state.context_wrapper.window().inner_size();
                                let gl = state.gl.read().unwrap();
                                gl.viewport(0, 0, size.width as _, size.height as _);
                                gl.clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT);
                            }
                            let interface = Reference::new(interface.borrow_mut(), registry);
                            let delta_time =
                                Reference::new_real(redraw_timer.elapsed().as_secs_f64(), registry);
                            redraw_timer = Instant::now();
                            SimpletonEvent::dispatch(
                                context,
                                registry,
                                window.redraw_event.clone(),
                                Reference::new_array(vec![interface, delta_time], registry),
                            );
                            state.context_wrapper.swap_buffers().unwrap();
                        }
                        *control_flow = ControlFlow::Exit;
                    }
                    Event::WindowEvent { ref event, .. } => match event {
                        WindowEvent::Resized(physical_size) => {
                            state.context_wrapper.resize(*physical_size);
                            let size = state.context_wrapper.window().inner_size();
                            let mut interface = interface
                                .write()
                                .expect("Could not write to window interface!");
                            interface.width = size.width as _;
                            interface.height = size.height as _;
                        }
                        WindowEvent::CloseRequested => {
                            window.running = false;
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            mouse_position = *position;
                            let interface = Reference::new(interface.borrow_mut(), registry);
                            let input = Reference::new(
                                MouseInput {
                                    state: Reference::null(),
                                    button: Reference::null(),
                                    x: Reference::new_integer(
                                        mouse_position.x as Integer,
                                        registry,
                                    ),
                                    y: Reference::new_integer(
                                        mouse_position.y as Integer,
                                        registry,
                                    ),
                                },
                                registry,
                            );
                            SimpletonEvent::dispatch(
                                context,
                                registry,
                                window.input_event.clone(),
                                Reference::new_array(vec![interface, input], registry),
                            );
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            let interface = Reference::new(interface.borrow_mut(), registry);
                            let input = Reference::new(
                                MouseInput {
                                    state: Reference::new_boolean(
                                        match state {
                                            ElementState::Pressed => true,
                                            ElementState::Released => false,
                                        },
                                        registry,
                                    ),
                                    button: Reference::new_text(format!("{button:?}"), registry),
                                    x: Reference::new_integer(
                                        mouse_position.x as Integer,
                                        registry,
                                    ),
                                    y: Reference::new_integer(
                                        mouse_position.y as Integer,
                                        registry,
                                    ),
                                },
                                registry,
                            );
                            SimpletonEvent::dispatch(
                                context,
                                registry,
                                window.input_event.clone(),
                                Reference::new_array(vec![interface, input], registry),
                            );
                        }
                        WindowEvent::KeyboardInput { input, .. } => {
                            let interface = Reference::new(interface.borrow_mut(), registry);
                            let input = Reference::new(
                                KeyboardInput {
                                    state: Reference::new_boolean(
                                        match input.state {
                                            ElementState::Pressed => true,
                                            ElementState::Released => false,
                                        },
                                        registry,
                                    ),
                                    scancode: Reference::new_integer(
                                        input.scancode as Integer,
                                        registry,
                                    ),
                                    keycode: Reference::new_text(
                                        input
                                            .virtual_keycode
                                            .map(|code| format!("{code:?}"))
                                            .unwrap_or_default(),
                                        registry,
                                    ),
                                },
                                registry,
                            );
                            SimpletonEvent::dispatch(
                                context,
                                registry,
                                window.input_event.clone(),
                                Reference::new_array(vec![interface, input], registry),
                            );
                        }
                        _ => (),
                    },
                    _ => (),
                }
            });
            if window.running {
                window.running = interface
                    .write()
                    .expect("Could not write to window interface!")
                    .running;
            }
        }
        window.state = Some(state);
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn gl(registry: &Registry, window: Reference) -> Reference {
        let window = window.read::<Window>().expect("`window` is not a Window!");
        window
            .state
            .as_ref()
            .map(|state| Reference::new(state.gl.borrow(), registry))
            .unwrap_or_default()
    }

    #[intuicio_method()]
    pub fn redraw_event(window: Reference) -> Reference {
        window
            .read::<Window>()
            .expect("`window` is not a Window!")
            .redraw_event
            .clone()
    }

    #[intuicio_method()]
    pub fn input_event(window: Reference) -> Reference {
        window
            .read::<Window>()
            .expect("`window` is not a Window!")
            .input_event
            .clone()
    }
}

#[derive(Default)]
pub struct WindowInterfaceState {
    width: Integer,
    height: Integer,
    running: bool,
    gl: Gl,
}

#[intuicio_function(module_name = "window_interface", name = "width", use_registry)]
pub fn window_interface_width(registry: &Registry, interface: Reference) -> Reference {
    let interface = interface
        .read::<WindowInterface>()
        .expect("`interface` is not a WindowInterface!");
    let interface = interface
        .as_ref()
        .expect("`interface` has invalid window interface state!")
        .read()
        .expect("Could not read `interface` state!");
    Reference::new_integer(interface.width, registry)
}

#[intuicio_function(module_name = "window_interface", name = "height", use_registry)]
pub fn window_interface_height(registry: &Registry, interface: Reference) -> Reference {
    let interface = interface
        .read::<WindowInterface>()
        .expect("`interface` is not a WindowInterface!");
    let interface = interface
        .as_ref()
        .expect("`interface` has invalid window interface state!")
        .read()
        .expect("Could not read `interface` state!");
    Reference::new_integer(interface.height, registry)
}

#[intuicio_function(module_name = "window_interface", name = "gl", use_registry)]
pub fn window_interface_gl(registry: &Registry, interface: Reference) -> Reference {
    let interface = interface
        .read::<WindowInterface>()
        .expect("`interface` is not a WindowInterface!");
    let interface = interface
        .as_ref()
        .expect("`interface` has invalid window interface state!")
        .read()
        .expect("Could not read `interface` state!");
    Reference::new(
        interface
            .gl
            .as_ref()
            .expect("`interface` has invalid GL context!")
            .borrow(),
        registry,
    )
}

#[intuicio_function(module_name = "window_interface", name = "exit")]
pub fn window_interface_exit(mut interface: Reference) -> Reference {
    let mut interface = interface
        .write::<WindowInterface>()
        .expect("`interface` is not a WindowInterface!");
    let mut interface = interface
        .as_mut()
        .expect("`interface` has invalid window interface state!")
        .write()
        .expect("Could not write `interface` state!");
    interface.running = false;
    Reference::null()
}

#[unsafe(no_mangle)]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[unsafe(no_mangle)]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_type(define_native_struct! {
        registry => mod gl struct Gl (Gl) {}
    });
    registry.add_type(define_native_struct! {
        registry => mod window_interface struct WindowInterface (WindowInterface) {}
    });
    registry.add_type(MouseInput::define_struct(registry));
    registry.add_type(KeyboardInput::define_struct(registry));
    registry.add_type(WindowConfig::define_struct(registry));
    registry.add_type(Window::define_struct(registry));
    registry.add_function(Window::new__define_function(registry));
    registry.add_function(Window::run__define_function(registry));
    registry.add_function(Window::gl__define_function(registry));
    registry.add_function(Window::redraw_event__define_function(registry));
    registry.add_function(Window::input_event__define_function(registry));
    registry.add_function(window_interface_width::define_function(registry));
    registry.add_function(window_interface_height::define_function(registry));
    registry.add_function(window_interface_gl::define_function(registry));
    registry.add_function(window_interface_exit::define_function(registry));
}
