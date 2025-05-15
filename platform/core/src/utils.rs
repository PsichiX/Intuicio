use crate::{object::Object, registry::Registry, types::TypeQuery};
use intuicio_data::{data_stack::DataStack, non_zero_dealloc};

pub fn object_push_to_stack(object: Object, data_stack: &mut DataStack) -> bool {
    unsafe {
        let (handle, memory) = object.into_inner();
        if memory.is_null() {
            return false;
        }
        let bytes = std::slice::from_raw_parts(memory, handle.layout().size());
        let result = data_stack.push_raw(
            *handle.layout(),
            handle.type_hash(),
            handle.finalizer(),
            bytes,
        );
        non_zero_dealloc(memory, *handle.layout());
        result
    }
}

pub fn object_pop_from_stack(data_stack: &mut DataStack, registry: &Registry) -> Option<Object> {
    unsafe {
        let (layout, type_hash, finalizer, data) = data_stack.pop_raw()?;
        if let Some(handle) = registry.find_type(TypeQuery {
            type_hash: Some(type_hash),
            ..Default::default()
        }) {
            Object::from_bytes(handle, &data)
        } else {
            data_stack.push_raw(layout, type_hash, finalizer, &data);
            None
        }
    }
}
