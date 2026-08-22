//! Helpers for moving [`Object`] values on and off a data stack.
use crate::{object::Object, registry::Registry, types::TypeQuery};
use intuicio_data::{data_stack::DataStack, non_zero_dealloc};

/// Moves an object onto the stack as a plain value of its type.
///
/// The object's own allocation is freed, since the stack takes the bytes.
/// Returns `false` when the value does not fit.
///
/// **Native types only.** A stack slot keeps its destructor as a plain function
/// pointer. A runtime type drops itself by a field walk, which is not a
/// function pointer. Such a value would be dropped without its fields, and
/// every allocation they own would leak, so this refuses instead. Put a runtime
/// value in a `DynamicManaged` and push that.
pub fn object_push_to_stack(object: Object, data_stack: &mut DataStack) -> bool {
    unsafe {
        let (handle, memory) = object.into_inner();
        if memory.is_null() {
            return false;
        }
        let Some(finalizer) = handle.finalizer().as_native() else {
            return false;
        };
        let bytes = std::slice::from_raw_parts(memory, handle.layout().size());
        let result = data_stack.push_raw(*handle.layout(), handle.type_hash(), finalizer, bytes);
        non_zero_dealloc(memory, *handle.layout());
        result
    }
}

/// Moves the top stack value into an [`Object`], looking its type up in the
/// registry.
///
/// Puts the value back and returns [`None`] when the type is not registered.
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
