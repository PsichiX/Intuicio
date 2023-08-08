use crate::{object::Object, prelude::StructQuery, registry::Registry};
use intuicio_data::data_stack::DataStack;

pub fn object_push_to_stack(object: Object, data_stack: &mut DataStack) -> bool {
    unsafe {
        let (handle, memory) = object.into_inner();
        data_stack.push_raw(
            *handle.layout(),
            handle.type_hash(),
            handle.finalizer(),
            &memory,
        )
    }
}

pub fn object_pop_from_stack(data_stack: &mut DataStack, registry: &Registry) -> Option<Object> {
    unsafe {
        let (layout, type_hash, finalizer, data) = data_stack.pop_raw()?;
        if let Some(handle) = registry.find_struct(StructQuery {
            type_hash: Some(type_hash),
            ..Default::default()
        }) {
            Object::new_raw(handle, data)
        } else {
            data_stack.push_raw(layout, type_hash, finalizer, &data);
            None
        }
    }
}