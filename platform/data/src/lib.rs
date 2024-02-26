pub mod data_heap;
pub mod data_stack;
pub mod lifetime;
pub mod managed;
pub mod managed_arena;
pub mod managed_box;
pub mod shared;
pub mod type_hash;

pub mod prelude {
    pub use crate::{
        data_heap::*, data_stack::*, lifetime::*, managed::*, managed_arena::*, managed_box::*,
        shared::*, type_hash::*, Finalize, Initialize,
    };
}

pub trait Initialize: Sized {
    fn initialize() -> Self;

    /// # Safety
    unsafe fn initialize_raw(data: *mut ()) {
        data.cast::<Self>().write(Self::initialize());
    }
}

impl<T> Initialize for T
where
    T: Default,
{
    fn initialize() -> Self {
        Self::default()
    }
}

pub trait Finalize: Sized {
    /// # Safety
    unsafe fn finalize_raw(data: *mut ()) {
        data.cast::<Self>().drop_in_place();
    }
}

impl<T> Finalize for T {}
