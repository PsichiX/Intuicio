use glow::{
    Buffer, Context as GlowContext, HasContext, Program, Texture, UniformLocation, VertexArray,
    ARRAY_BUFFER, BLEND, CLAMP_TO_EDGE, COLOR_BUFFER_BIT, ELEMENT_ARRAY_BUFFER, FLOAT,
    FRAGMENT_SHADER, LINEAR, NEAREST, ONE_MINUS_SRC_ALPHA, RGBA, SRC_ALPHA, STATIC_DRAW, TEXTURE0,
    TEXTURE_2D, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER, TEXTURE_WRAP_S, TEXTURE_WRAP_T, TRIANGLES,
    UNSIGNED_BYTE, UNSIGNED_INT, VERTEX_SHADER,
};
use image::ImageReader;
use intuicio_core::{core_version, prelude::*};
use intuicio_data::prelude::*;
use intuicio_derive::{intuicio_method, intuicio_methods, IntuicioStruct};
use intuicio_frontend_simpleton::prelude::{bytes::Bytes, *};
use std::{collections::HashMap, io::Cursor};
use vek::{FrustumPlanes, Mat4, Quaternion, Transform as VekTransform, Vec3};

pub type Gl = Option<ManagedRef<GlowContext>>;

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Renderer", module_name = "renderer")]
pub struct Renderer {
    #[intuicio(ignore)]
    gl: Gl,
    #[intuicio(ignore)]
    shaders: HashMap<Integer, (Program, HashMap<String, UniformLocation>)>,
    #[intuicio(ignore)]
    textures: HashMap<Integer, Texture>,
    /// {handle: (vertex array, vertex buffer, index buffer)}
    #[intuicio(ignore)]
    meshes: HashMap<Integer, (VertexArray, Buffer, Buffer)>,
    #[intuicio(ignore)]
    handle_generator: Integer,
    #[intuicio(ignore)]
    shader_version: String,
}

#[intuicio_methods(module_name = "renderer")]
impl Renderer {
    #[allow(clippy::new_ret_no_self)]
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, gl: Reference, shader_version: Reference) -> Reference {
        let gl = gl.read::<Gl>().expect("`gl` is not a GL context!");
        let gl = gl.as_ref().expect("`gl` does not have valid GL context!");
        unsafe {
            let gl = gl.read().expect("Could not read `gl` GL context!");
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(COLOR_BUFFER_BIT);
        }
        let shader_version = shader_version
            .read::<Text>()
            .map(|version| version.to_owned())
            .unwrap_or_else(|| "330".to_owned());
        Reference::new(
            Renderer {
                gl: gl.borrow(),
                shaders: Default::default(),
                textures: Default::default(),
                meshes: Default::default(),
                handle_generator: 0,
                shader_version,
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn create_shader(
        registry: &Registry,
        mut renderer: Reference,
        vertex_content: Reference,
        fragment_content: Reference,
        uniforms: Reference,
        layout: Reference,
    ) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = renderer.generate_handle();
        let vertex_content = vertex_content
            .read::<Text>()
            .expect("`vertex_content` is not a Text!");
        let vertex_content = format!("#version {}\n{}", renderer.shader_version, vertex_content);
        let fragment_content = fragment_content
            .read::<Text>()
            .expect("`fragment_content` is not a Text!");
        let fragment_content =
            format!("#version {}\n{}", renderer.shader_version, fragment_content);
        let uniforms = uniforms
            .read::<Array>()
            .expect("`uniforms` is not an Array!");
        let layout = layout.read::<Array>().expect("`layout` is not an Array!");
        if layout.is_empty() {
            panic!("`layout` array is empty!");
        }
        let (program, uniforms) = unsafe {
            let gl = renderer
                .gl
                .as_ref()
                .expect("`renderer` has invalid GL context!");
            let gl = gl.read().unwrap();
            let vertex_shader = gl
                .create_shader(VERTEX_SHADER)
                .expect("Could not create vertex shader!");
            gl.shader_source(vertex_shader, &vertex_content);
            gl.compile_shader(vertex_shader);
            if !gl.get_shader_compile_status(vertex_shader) {
                panic!(
                    "Could not compile vertex shader: {}",
                    gl.get_shader_info_log(vertex_shader)
                );
            }
            let fragment_shader = gl
                .create_shader(FRAGMENT_SHADER)
                .expect("Could not create fragment shader!");
            gl.shader_source(fragment_shader, &fragment_content);
            gl.compile_shader(fragment_shader);
            if !gl.get_shader_compile_status(fragment_shader) {
                panic!(
                    "Could not compile fragment shader: {}",
                    gl.get_shader_info_log(fragment_shader)
                );
            }
            let program = gl
                .create_program()
                .expect("Could not create program object!");
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            for (index, chunk) in layout.chunks(2).enumerate() {
                let name = chunk[0]
                    .read::<Text>()
                    .expect("`layout` array item is not a Text!");
                gl.bind_attrib_location(program, index as _, &name);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!(
                    "Could not link shader: {}",
                    gl.get_program_info_log(program)
                );
            }
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);
            let uniforms = uniforms
                .iter()
                .filter_map(|item| {
                    let item = item.read::<Text>().expect("`uniforms` item is not a Text!");
                    let location = gl.get_uniform_location(program, item.as_str())?;
                    Some((item.to_owned(), location))
                })
                .collect();
            (program, uniforms)
        };
        renderer.shaders.insert(handle, (program, uniforms));
        Reference::new_integer(handle, registry)
    }

    #[intuicio_method()]
    pub fn destroy_shader(mut renderer: Reference, handle: Reference) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = *handle
            .read::<Integer>()
            .expect("`handle` is not an Integer!");
        if let Some((program, _)) = renderer.shaders.remove(&handle) {
            unsafe {
                renderer
                    .gl
                    .as_ref()
                    .expect("`renderer` has invalid GL context!")
                    .read()
                    .unwrap()
                    .delete_program(program);
            }
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn create_texture(
        registry: &Registry,
        mut renderer: Reference,
        bytes: Reference,
        width: Reference,
        height: Reference,
        interpolated: Reference,
    ) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = renderer.generate_handle();
        let bytes = bytes.read::<Bytes>().expect("`bytes` is not Bytes!");
        let width = width
            .read::<Integer>()
            .expect("`width` is not an Integer!")
            .max(1);
        let height = height
            .read::<Integer>()
            .expect("`height` is not an Integer!")
            .max(1);
        let interpolated = *interpolated
            .read::<Boolean>()
            .expect("`interpolated` is not a Boolean!");
        if (width * height) as usize * std::mem::size_of::<u8>() * 4 != bytes.get_ref().len() {
            panic!("`bytes` buffer size does not match provided `width` and `height`!");
        }
        let texture = unsafe {
            let gl = renderer
                .gl
                .as_ref()
                .expect("`renderer` has invalid GL context!");
            let gl = gl.read().unwrap();
            let texture = gl
                .create_texture()
                .expect("Could not create texture object!");
            gl.bind_texture(TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                TEXTURE_2D,
                0,
                RGBA as _,
                width as _,
                height as _,
                0,
                RGBA,
                UNSIGNED_BYTE,
                Some(bytes.get_ref()),
            );
            let filter = if interpolated { LINEAR } else { NEAREST };
            gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MIN_FILTER, filter as _);
            gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_MAG_FILTER, filter as _);
            gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE as _);
            gl.tex_parameter_i32(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE as _);
            gl.generate_mipmap(TEXTURE_2D);
            gl.bind_texture(TEXTURE_2D, None);
            texture
        };
        renderer.textures.insert(handle, texture);
        Reference::new_integer(handle, registry)
    }

    #[intuicio_method()]
    pub fn destroy_texture(mut renderer: Reference, handle: Reference) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = *handle
            .read::<Integer>()
            .expect("`handle` is not an Integer!");
        if let Some(texture) = renderer.textures.remove(&handle) {
            unsafe {
                renderer
                    .gl
                    .as_ref()
                    .expect("`renderer` has invalid GL context!")
                    .read()
                    .unwrap()
                    .delete_texture(texture);
            }
        }
        Reference::null()
    }

    #[intuicio_method(use_registry)]
    pub fn create_mesh(
        registry: &Registry,
        mut renderer: Reference,
        vertex_bytes: Reference,
        index_bytes: Reference,
        layout: Reference,
    ) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = renderer.generate_handle();
        let vertex_bytes = vertex_bytes
            .read::<Bytes>()
            .expect("`vertex_bytes` is not Bytes!");
        let index_bytes = index_bytes
            .read::<Bytes>()
            .expect("`index_bytes` is not Bytes!");
        let layout = layout.read::<Array>().expect("`layout` is not an Array!");
        if layout.is_empty() {
            panic!("`layout` array is empty!");
        }
        let (vertex_array, vertex_buffer, index_buffer) = unsafe {
            let gl = renderer
                .gl
                .as_ref()
                .expect("`renderer` has invalid GL context!");
            let gl = gl.read().unwrap();
            let vertex_array = gl
                .create_vertex_array()
                .expect("Could not create vertex array!");
            gl.bind_vertex_array(Some(vertex_array));
            let index_buffer = gl.create_buffer().expect("Could not create index buffer!");
            gl.bind_buffer(ELEMENT_ARRAY_BUFFER, Some(index_buffer));
            gl.buffer_data_u8_slice(ELEMENT_ARRAY_BUFFER, index_bytes.get_ref(), STATIC_DRAW);
            let vertex_buffer = gl.create_buffer().expect("Could not create vertex buffer!");
            gl.bind_buffer(ARRAY_BUFFER, Some(vertex_buffer));
            gl.buffer_data_u8_slice(ARRAY_BUFFER, vertex_bytes.get_ref(), STATIC_DRAW);
            let layout = layout
                .chunks(2)
                .map(|chunk| {
                    let name = chunk[0]
                        .read::<Text>()
                        .expect("`layout` array item is not a Text!");
                    let channels = *chunk[1]
                        .read::<Integer>()
                        .expect("`layout` array item is not an Integer!");
                    (name.to_owned(), channels as usize)
                })
                .collect::<Vec<_>>();
            let stride = layout
                .iter()
                .map(|(_, channels)| (*channels * std::mem::size_of::<f32>()) as i32)
                .sum();
            let mut offset = 0;
            for (index, (_, channels)) in layout.into_iter().enumerate() {
                gl.vertex_attrib_pointer_f32(
                    index as _,
                    channels as _,
                    FLOAT,
                    false,
                    stride,
                    offset as _,
                );
                gl.enable_vertex_attrib_array(index as _);
                offset += channels * std::mem::size_of::<f32>();
            }
            gl.bind_vertex_array(None);
            (vertex_array, vertex_buffer, index_buffer)
        };
        renderer
            .meshes
            .insert(handle, (vertex_array, vertex_buffer, index_buffer));
        Reference::new_integer(handle, registry)
    }

    #[intuicio_method()]
    pub fn destroy_mesh(mut renderer: Reference, handle: Reference) -> Reference {
        let mut renderer = renderer
            .write::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let handle = *handle
            .read::<Integer>()
            .expect("`handle` is not an Integer!");
        if let Some((vertex_array, vertex_buffer, index_buffer)) = renderer.meshes.remove(&handle) {
            unsafe {
                let gl = renderer
                    .gl
                    .as_ref()
                    .expect("`renderer` has invalid GL context!");
                let gl = gl.read().unwrap();
                gl.delete_vertex_array(vertex_array);
                gl.delete_buffer(vertex_buffer);
                gl.delete_buffer(index_buffer);
            }
        }
        Reference::null()
    }

    #[intuicio_method()]
    pub fn render(
        renderer: Reference,
        buffer: Reference,
        viewport_width: Reference,
        viewport_height: Reference,
        perspective_fov: Reference,
        camera_transform: Reference,
    ) -> Reference {
        let renderer = renderer
            .read::<Renderer>()
            .expect("`renderer` is not a Renderer!");
        let buffer = buffer
            .read::<RenderBuffer>()
            .expect("`buffer` is not a RenderBuffer!");
        let viewport_width = *viewport_width
            .read::<Integer>()
            .expect("`viewport_width` is not an Integer!") as f32;
        let viewport_height = *viewport_height
            .read::<Integer>()
            .expect("`viewport_height` is not an Integer!") as f32;
        let perspective_fov = perspective_fov
            .read::<Real>()
            .map(|value| *value)
            .unwrap_or(0.0)
            .max(0.0) as f32;
        let camera_transform = camera_transform
            .read::<Transform>()
            .expect("`camera_transform` is not a Transform!")
            .to_matrix();
        let gl = renderer
            .gl
            .as_ref()
            .expect("`renderer` has invalid GL context!");
        let gl = gl.read().unwrap();
        unsafe {
            gl.enable(BLEND);
            gl.blend_func(SRC_ALPHA, ONE_MINUS_SRC_ALPHA);
        }
        let projection = if perspective_fov > 0.0 {
            Mat4::infinite_perspective_rh(
                perspective_fov.to_radians(),
                (viewport_width / viewport_height).abs(),
                1.0,
            )
        } else {
            let viewport_width = viewport_width * 0.5;
            let viewport_height = viewport_height * 0.5;
            Mat4::orthographic_without_depth_planes(FrustumPlanes {
                left: -viewport_width,
                right: viewport_width,
                top: -viewport_height,
                bottom: viewport_height,
                near: -1.0,
                far: 1.0,
            })
        };
        let projection = projection.as_col_slice();
        let view = camera_transform.inverted();
        let view = view.as_col_slice();
        let mut last_shader = -1;
        let mut last_uniform_locations = None;
        let mut last_mesh = -1;
        unsafe {
            for renderable in &buffer.buffer {
                if renderable.shader < 0 || renderable.mesh < 0 {
                    continue;
                }
                if last_shader != renderable.shader {
                    if let Some((program, uniforms)) = renderer.shaders.get(&renderable.shader) {
                        gl.use_program(Some(*program));
                        last_shader = renderable.shader;
                        last_uniform_locations = Some(uniforms);
                    } else {
                        continue;
                    }
                }
                if last_mesh != renderable.mesh {
                    if let Some((vertex_array, _, _)) = renderer.meshes.get(&renderable.mesh) {
                        gl.bind_vertex_array(Some(*vertex_array));
                        last_mesh = renderable.mesh;
                    } else {
                        continue;
                    }
                }
                let mut active_textures = 0;
                if let Some(locations) = last_uniform_locations {
                    let model = renderable.model_transform.as_col_slice();
                    gl.uniform_matrix_4_f32_slice(locations.get("projection"), false, projection);
                    gl.uniform_matrix_4_f32_slice(locations.get("view"), false, view);
                    gl.uniform_matrix_4_f32_slice(locations.get("model"), false, model);
                    for (name, data) in &renderable.uniforms {
                        match data {
                            UniformData::Float(data) => match data.len() {
                                1 => {
                                    gl.uniform_1_f32_slice(locations.get(name), data);
                                }
                                2 => {
                                    gl.uniform_2_f32_slice(locations.get(name), data);
                                }
                                3 => {
                                    gl.uniform_3_f32_slice(locations.get(name), data);
                                }
                                4 => {
                                    gl.uniform_4_f32_slice(locations.get(name), data);
                                }
                                16 => {
                                    gl.uniform_matrix_4_f32_slice(locations.get(name), false, data);
                                }
                                _ => {}
                            },
                            UniformData::Texture(handle) => {
                                if let Some(texture) = renderer.textures.get(handle) {
                                    gl.active_texture(TEXTURE0 + active_textures);
                                    gl.bind_texture(TEXTURE_2D, Some(*texture));
                                    gl.uniform_1_i32(locations.get(name), active_textures as _);
                                    active_textures += 1;
                                }
                            }
                        }
                    }
                }
                gl.draw_elements(
                    TRIANGLES,
                    (renderable.triangles_count * 3) as _,
                    UNSIGNED_INT,
                    (renderable.index_start * std::mem::size_of::<u32>()) as _,
                );
            }
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
        Reference::null()
    }

    fn generate_handle(&mut self) -> Integer {
        let result = self.handle_generator;
        self.handle_generator = self.handle_generator.wrapping_add_unsigned(1);
        result
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "RenderBuffer", module_name = "render_buffer")]
pub struct RenderBuffer {
    #[intuicio(ignore)]
    buffer: Vec<Renderable>,
}

#[intuicio_methods(module_name = "render_buffer")]
impl RenderBuffer {
    #[intuicio_method()]
    pub fn clear(mut buffer: Reference) -> Reference {
        let mut buffer = buffer
            .write::<RenderBuffer>()
            .expect("`buffer` is not a RenderBuffer!");
        buffer.buffer.clear();
        Reference::null()
    }

    #[intuicio_method()]
    pub fn enqueue(
        mut buffer: Reference,
        shader: Reference,
        mesh: Reference,
        model_transform: Reference,
        index_start: Reference,
        triangles_count: Reference,
        uniforms: Reference,
    ) -> Reference {
        let mut buffer = buffer
            .write::<RenderBuffer>()
            .expect("`buffer` is not a RenderBuffer!");
        let shader = *shader
            .read::<Integer>()
            .expect("`shader` is not an Integer!");
        let mesh = *mesh.read::<Integer>().expect("`mesh` is not an Integer!");
        let model_transform = model_transform
            .read::<Transform>()
            .expect("`model_transform` is not a Transform!")
            .to_matrix();
        let index_start = *index_start
            .read::<Integer>()
            .expect("`index_start` is not an Integer!") as _;
        let triangles_count = *triangles_count
            .read::<Integer>()
            .expect("`triangles_count` is not an Integer!") as _;
        let uniforms = uniforms.read::<Map>().expect("`uniforms` is not a Map!");
        let uniforms = uniforms
            .iter()
            .filter_map(|(name, data)| {
                let data = if let Some(data) = data.read::<Array>() {
                    UniformData::Float(
                        data.iter()
                            .map(|item| {
                                *item
                                    .read::<Real>()
                                    .expect("`data` array item is not a Real!")
                                    as f32
                            })
                            .collect(),
                    )
                } else if let Some(data) = data.read::<Integer>() {
                    UniformData::Texture(*data)
                } else {
                    return None;
                };
                Some((name.to_owned(), data))
            })
            .collect();
        buffer.buffer.push(Renderable {
            shader,
            mesh,
            model_transform,
            index_start,
            triangles_count,
            uniforms,
        });
        Reference::null()
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Transform", module_name = "transform")]
pub struct Transform {
    pub px: Reference,
    pub py: Reference,
    pub pz: Reference,
    pub yaw: Reference,
    pub pitch: Reference,
    pub roll: Reference,
    pub sx: Reference,
    pub sy: Reference,
    pub sz: Reference,
}

impl Transform {
    fn to_matrix(&self) -> Mat4<f32> {
        let px = self.px.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let py = self.py.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let pz = self.pz.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let yaw = self.yaw.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let pitch = self.pitch.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let roll = self.roll.read::<Real>().map(|value| *value).unwrap_or(0.0) as f32;
        let sx = self.sx.read::<Real>().map(|value| *value).unwrap_or(1.0) as f32;
        let sy = self.sy.read::<Real>().map(|value| *value).unwrap_or(1.0) as f32;
        let sz = self.sz.read::<Real>().map(|value| *value).unwrap_or(1.0) as f32;
        VekTransform {
            position: Vec3::new(px, py, pz),
            orientation: Quaternion::rotation_x(roll.to_radians())
                * Quaternion::rotation_y(pitch.to_radians())
                * Quaternion::rotation_z(yaw.to_radians()),
            scale: Vec3::new(sx, sy, sz),
        }
        .into()
    }
}

#[derive(IntuicioStruct, Default)]
#[intuicio(name = "Image", module_name = "image")]
pub struct Image {
    pub width: Reference,
    pub height: Reference,
    pub bytes: Reference,
}

#[intuicio_methods(module_name = "image")]
impl Image {
    #[intuicio_method(use_registry)]
    pub fn decode(registry: &Registry, bytes: Reference) -> Reference {
        let bytes = bytes.read::<Bytes>().expect("`bytes` is not Bytes!");
        let bytes = Cursor::new(bytes.get_ref());
        let reader = ImageReader::new(bytes).with_guessed_format().unwrap();
        let buffer = reader
            .decode()
            .expect("Could not decode image from `bytes`!")
            .into_rgba8();
        let bytes = unsafe { buffer.align_to::<u8>().1.to_vec() };
        Reference::new(
            Image {
                width: Reference::new_integer(buffer.width() as _, registry),
                height: Reference::new_integer(buffer.height() as _, registry),
                bytes: Reference::new(Bytes::new_raw(bytes), registry),
            },
            registry,
        )
    }
}

struct Renderable {
    shader: Integer,
    mesh: Integer,
    model_transform: Mat4<f32>,
    index_start: usize,
    triangles_count: usize,
    uniforms: HashMap<String, UniformData>,
}

enum UniformData {
    Float(Vec<f32>),
    Texture(Integer),
}

#[no_mangle]
pub extern "C" fn version() -> IntuicioVersion {
    core_version()
}

#[no_mangle]
pub extern "C" fn install(registry: &mut Registry) {
    registry.add_type(Renderer::define_struct(registry));
    registry.add_type(RenderBuffer::define_struct(registry));
    registry.add_type(Transform::define_struct(registry));
    registry.add_type(Image::define_struct(registry));
    registry.add_function(Renderer::new__define_function(registry));
    registry.add_function(Renderer::create_shader__define_function(registry));
    registry.add_function(Renderer::destroy_shader__define_function(registry));
    registry.add_function(Renderer::create_texture__define_function(registry));
    registry.add_function(Renderer::destroy_texture__define_function(registry));
    registry.add_function(Renderer::create_mesh__define_function(registry));
    registry.add_function(Renderer::destroy_mesh__define_function(registry));
    registry.add_function(Renderer::render__define_function(registry));
    registry.add_function(RenderBuffer::clear__define_function(registry));
    registry.add_function(RenderBuffer::enqueue__define_function(registry));
    registry.add_function(Image::decode__define_function(registry));
}
