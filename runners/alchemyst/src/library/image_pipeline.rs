use super::{color::Color, image::Image};
use image::{Rgba, Rgba32FImage};
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_simpleton::prelude::{closure::Closure, jobs::Jobs, *};
use std::{
    collections::HashMap,
    sync::Arc,
    thread::{available_parallelism, spawn},
};

#[derive(Default, Clone)]
enum Primitive {
    #[default]
    Null,
    Integer(Integer),
    Real(Real),
    Array(Vec<Primitive>),
    Map(HashMap<String, Primitive>),
}

impl Primitive {
    fn from_value(value: &Reference) -> Self {
        if let Some(value) = value.read::<Integer>() {
            Self::Integer(*value)
        } else if let Some(value) = value.read::<Real>() {
            Self::Real(*value)
        } else if let Some(value) = value.read::<Array>() {
            Self::Array(value.iter().map(|value| Self::from_value(value)).collect())
        } else if let Some(value) = value.read::<Map>() {
            Self::Map(
                value
                    .iter()
                    .map(|(key, value)| (key.to_owned(), Self::from_value(value)))
                    .collect(),
            )
        } else {
            Self::Null
        }
    }

    fn to_value(&self, registry: &Registry) -> Reference {
        match self {
            Self::Null => Reference::null(),
            Self::Integer(value) => Reference::new_integer(*value, registry),
            Self::Real(value) => Reference::new_real(*value, registry),
            Self::Array(value) => Reference::new_array(
                value.iter().map(|value| value.to_value(registry)).collect(),
                registry,
            ),
            Self::Map(value) => Reference::new_map(
                value
                    .iter()
                    .map(|(key, value)| (key.to_owned(), value.to_value(registry)))
                    .collect(),
                registry,
            ),
        }
    }
}

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(
    name = "Fragment",
    module_name = "image_pipeline",
    override_send = true
)]
pub struct Fragment {
    pub index: Reference,
    pub col: Reference,
    pub row: Reference,
    pub u: Reference,
    pub v: Reference,
    pub width: Reference,
    pub height: Reference,
    pub start: Reference,
    pub end: Reference,
}

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(
    name = "Pipeline",
    module_name = "image_pipeline",
    override_send = true
)]
pub struct Pipeline {
    pub width: Reference,
    pub height: Reference,
    pub samplers: Reference,
}

#[intuicio_methods(module_name = "image_pipeline")]
impl Pipeline {
    #[intuicio_method(use_context, use_registry)]
    pub fn process_single_thread(
        context: &mut Context,
        registry: &Registry,
        pipeline: Reference,
        closure: Reference,
    ) -> Reference {
        let pipeline = pipeline.read::<Pipeline>().unwrap();
        let width = *pipeline.width.read::<Integer>().unwrap() as u32;
        let height = *pipeline.height.read::<Integer>().unwrap() as u32;
        {
            let samplers = pipeline.samplers.read::<Map>().unwrap();
            for sampler in samplers.values() {
                if !sampler.type_of().unwrap().is::<Sampler>() {
                    panic!("All items of `samplers` must be of Sampler type!");
                }
            }
        }
        let closure = closure.read::<Closure>().unwrap();
        let pixels_count = width * height;
        let output_width = width.saturating_sub(1) as Real;
        let output_height = height.saturating_sub(1) as Real;
        let result = (0..pixels_count)
            .map(|index| {
                let x = (index % width) as Integer;
                let y = (index / width) as Integer;
                let u = x as Real / output_width;
                let v = y as Real / output_height;
                let fragment = Reference::new(
                    Fragment {
                        index: Reference::new_integer(index as Integer, registry),
                        col: Reference::new_integer(x, registry),
                        row: Reference::new_integer(y, registry),
                        u: Reference::new_real(u, registry),
                        v: Reference::new_real(v, registry),
                        width: Reference::new_integer(width as Integer, registry),
                        height: Reference::new_integer(height as Integer, registry),
                        start: Reference::new_integer(0, registry),
                        end: Reference::new_integer(pixels_count as Integer, registry),
                    },
                    registry,
                );
                let args = [fragment, pipeline.samplers.clone()];
                closure
                    .invoke(context, registry, &args)
                    .read::<Color>()
                    .map(|color| color.to_pixel())
                    .unwrap_or_else(|| Rgba([0.0, 0.0, 0.0, 0.0]))
            })
            .flat_map(|pixel| pixel.0)
            .collect::<Vec<_>>();
        Rgba32FImage::from_vec(width, height, result)
            .map(|buffer| Reference::new(Image { buffer }, registry))
            .unwrap_or_default()
    }

    #[intuicio_method(use_context, use_registry)]
    pub fn process_multi_thread(
        context: &mut Context,
        registry: &Registry,
        pipeline: Reference,
        closure: Reference,
    ) -> Reference {
        let threads_count = available_parallelism()
            .ok()
            .map(|count| count.get() as u32)
            .unwrap_or_default();
        if threads_count <= 1 {
            return Self::process_single_thread(context, registry, pipeline, closure);
        }
        let closure = closure.read::<Closure>().unwrap();
        let captures = closure
            .captured
            .iter()
            .map(|value| Primitive::from_value(value))
            .collect::<Vec<_>>();
        let host_producer = match context.custom::<HostProducer>(Jobs::HOST_PRODUCER_CUSTOM) {
            Some(host_producer) => host_producer.clone(),
            None => return Reference::null(),
        };
        let pipeline = pipeline.read::<Pipeline>().unwrap();
        let width = *pipeline.width.read::<Integer>().unwrap() as u32;
        let height = *pipeline.height.read::<Integer>().unwrap() as u32;
        let samplers = pipeline.samplers.read::<Map>().unwrap();
        let signature = closure.function.handle().unwrap().signature();
        let function_name = signature.name.to_owned();
        let function_module_name = signature.module_name.to_owned();
        let pixels_count = width * height;
        let pixels_per_thread = if pixels_count % threads_count == 0 {
            pixels_count / threads_count
        } else {
            1 + pixels_count / threads_count
        };
        let output_width = width.saturating_sub(1) as Real;
        let output_height = height.saturating_sub(1) as Real;
        let mut offset = 0;
        let function_name_ = &function_name;
        let function_module_name_ = &function_module_name;
        let captures_ = captures.as_slice();
        let threads = (0..threads_count)
            .map(move |_| {
                let start = offset;
                offset += pixels_per_thread;
                let end = offset.min(pixels_count);
                let samplers = samplers
                    .iter()
                    .map(|(key, value)| (key.to_owned(), value.read::<Sampler>().unwrap().clone()))
                    .collect::<HashMap<_, _>>();
                let host_producer = host_producer.clone();
                let function_name = function_name_.to_owned();
                let function_module_name = function_module_name_.to_owned();
                let captures = captures_.to_owned();
                spawn(move || {
                    let mut host = host_producer.produce();
                    host.context()
                        .set_custom(Jobs::HOST_PRODUCER_CUSTOM, host_producer);
                    let (context, registry) = host.context_and_registry();
                    let samplers = samplers
                        .into_iter()
                        .map(|(key, value)| (key, Reference::new(value, registry)))
                        .collect::<HashMap<_, _>>();
                    let samplers = Reference::new_map(samplers, registry);
                    let captures = captures
                        .iter()
                        .map(|primitive| primitive.to_value(registry))
                        .collect::<Vec<_>>();
                    let size = (end - start) as usize;
                    if let Some(function) = registry.find_function(FunctionQuery {
                        name: Some(function_name.into()),
                        module_name: function_module_name.map(|name| name.into()),
                        ..Default::default()
                    }) {
                        (start..end)
                            .map(|index| {
                                let x = (index % width) as Integer;
                                let y = (index / width) as Integer;
                                let u = x as Real / output_width;
                                let v = y as Real / output_height;
                                let fragment = Reference::new(
                                    Fragment {
                                        index: Reference::new_integer(index as Integer, registry),
                                        col: Reference::new_integer(x, registry),
                                        row: Reference::new_integer(y, registry),
                                        u: Reference::new_real(u, registry),
                                        v: Reference::new_real(v, registry),
                                        width: Reference::new_integer(width as Integer, registry),
                                        height: Reference::new_integer(height as Integer, registry),
                                        start: Reference::new_integer(start as Integer, registry),
                                        end: Reference::new_integer(end as Integer, registry),
                                    },
                                    registry,
                                );
                                context.stack().push(samplers.clone());
                                context.stack().push(fragment);
                                for value in captures.iter().rev() {
                                    context.stack().push(value.clone());
                                }
                                function.invoke(context, registry);
                                context
                                    .stack()
                                    .pop::<Reference>()
                                    .unwrap()
                                    .read::<Color>()
                                    .map(|color| color.to_pixel())
                                    .unwrap_or_else(|| Rgba([0.0, 0.0, 0.0, 0.0]))
                            })
                            .collect()
                    } else {
                        vec![Rgba([0.0, 0.0, 0.0, 0.0]); size]
                    }
                })
            })
            .collect::<Vec<_>>();
        let result = threads
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .flat_map(|pixel| pixel.0)
            .collect::<Vec<_>>();
        Rgba32FImage::from_vec(width, height, result)
            .map(|buffer| Reference::new(Image { buffer }, registry))
            .unwrap_or_default()
    }
}

#[derive(IntuicioStruct, Default, Clone)]
#[intuicio(name = "Sampler", module_name = "image_sampler")]
pub struct Sampler {
    #[intuicio(ignore)]
    image: Arc<Image>,
}

#[intuicio_methods(module_name = "image_sampler")]
impl Sampler {
    #[intuicio_method(use_registry)]
    pub fn new(registry: &Registry, image: Reference) -> Reference {
        let image = image.read::<Image>().unwrap();
        Reference::new(
            Sampler {
                image: Arc::new(image.clone()),
            },
            registry,
        )
    }

    #[intuicio_method(use_registry)]
    pub fn clone(registry: &Registry, sampler: Reference) -> Reference {
        Reference::new(sampler.read::<Sampler>().unwrap().clone(), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn sample(
        registry: &Registry,
        sampler: Reference,
        u: Reference,
        v: Reference,
        interpolate: Reference,
        wrap: Reference,
    ) -> Reference {
        let sampler = sampler.read::<Sampler>().unwrap();
        let u = *u.read::<Real>().unwrap() as f32;
        let v = *v.read::<Real>().unwrap() as f32;
        let wrap = *wrap.read::<Boolean>().unwrap();
        let interpolate = *interpolate.read::<Boolean>().unwrap();
        let result = sampler.image.sample_inner(u, v, wrap, interpolate);
        Reference::new(Color::from_pixel(&result, registry), registry)
    }

    #[intuicio_method(use_registry)]
    pub fn fetch(
        registry: &Registry,
        sampler: Reference,
        col: Reference,
        row: Reference,
    ) -> Reference {
        let sampler = sampler.read::<Sampler>().unwrap();
        let col = *col.read::<Integer>().unwrap() as u32;
        let row = *row.read::<Integer>().unwrap() as u32;
        let result = sampler.image.get_pixel_inner(col, row);
        Reference::new(Color::from_pixel(&result, registry), registry)
    }
}

pub fn install(registry: &mut Registry) {
    registry.add_struct(Fragment::define_struct(registry));
    registry.add_struct(Sampler::define_struct(registry));
    registry.add_struct(Pipeline::define_struct(registry));
    registry.add_function(Pipeline::process_single_thread__define_function(registry));
    registry.add_function(Pipeline::process_multi_thread__define_function(registry));
    registry.add_function(Sampler::new__define_function(registry));
    registry.add_function(Sampler::clone__define_function(registry));
    registry.add_function(Sampler::sample__define_function(registry));
    registry.add_function(Sampler::fetch__define_function(registry));
}
