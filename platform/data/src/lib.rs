pub mod data_stack;
pub mod lifetime;
pub mod managed;
pub mod managed_box;
pub mod shared;
pub mod type_hash;

pub mod prelude {
    pub use crate::{
        data_stack::*, lifetime::*, managed::*, managed_box::*, shared::*, type_hash::*, Finalize,
        Initialize,
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
        data.cast::<Self>().read_unaligned();
    }
}

impl<T> Finalize for T {}

#[inline]
pub fn pointer_alignment_padding(pointer: *const u8, alignment: usize) -> usize {
    let mut result = (pointer as usize) % alignment;
    if result > 0 {
        result = alignment - result;
    }
    result
}

macro_rules! does_implement_trait {
    ($trait:path => $identifier:ident < $type:ident >) => {
        pub fn $identifier<$type>() -> bool {
            struct ImplementsTrait<'a, $type> {
                implements: &'a std::cell::Cell<bool>,
                _marker: std::marker::PhantomData<$type>,
            }

            #[allow(clippy::non_canonical_clone_impl)]
            impl<$type> Clone for ImplementsTrait<'_, $type> {
                fn clone(&self) -> Self {
                    self.implements.set(false);
                    ImplementsTrait {
                        implements: self.implements,
                        _marker: std::marker::PhantomData,
                    }
                }
            }

            impl<$type: $trait> Copy for ImplementsTrait<'_, $type> {}

            let implements = std::cell::Cell::new(true);
            let _ = [ImplementsTrait::<$type> {
                implements: &implements,
                _marker: std::marker::PhantomData,
            }]
            .clone();
            implements.get()
        }
    };
}

does_implement_trait!(Send => is_send<T>);
does_implement_trait!(Sync => is_sync<T>);
does_implement_trait!(Copy => is_copy<T>);
does_implement_trait!(Clone => is_clone<T>);
does_implement_trait!(Sized => is_sized<T>);
does_implement_trait!(Unpin => is_unpin<T>);
does_implement_trait!(ToString => is_to_string<T>);
