pub mod data_heap;
pub mod data_stack;
pub mod lifetime;
pub mod managed;
pub mod shared;
pub mod type_hash;

pub mod prelude {
    pub use crate::{
        data_heap::*, data_stack::*, lifetime::*, managed::*, shared::*, type_hash::*, Finalize,
        Initialize,
    };
}

pub trait Initialize: Sized {
    fn initialize(&mut self);

    /// # Safety
    unsafe fn initialize_raw(data: *mut ()) {
        Self::initialize(data.cast::<Self>().as_mut().unwrap());
    }
}

impl<T> Initialize for T
where
    T: Default,
{
    fn initialize(&mut self) {
        *self = Self::default();
    }
}

pub trait Finalize: Sized {
    /// # Safety
    unsafe fn finalize_raw(data: *mut ()) {
        std::ptr::drop_in_place(data.cast::<Self>());
    }
}

impl<T> Finalize for T {}
