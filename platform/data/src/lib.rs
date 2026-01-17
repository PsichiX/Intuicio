pub mod data_stack;
pub mod lifetime;
pub mod managed;
pub mod managed_gc;
pub mod shared;
pub mod type_hash;

pub trait Initialize: Sized {
    fn initialize() -> Self;

    /// # Safety
    unsafe fn initialize_raw(data: *mut ()) {
        unsafe { data.cast::<Self>().write(Self::initialize()) };
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
        unsafe { data.cast::<Self>().read_unaligned() };
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

/// # Safety
pub unsafe fn non_zero_alloc(mut layout: std::alloc::Layout) -> *mut u8 {
    unsafe {
        if layout.size() == 0 {
            layout = std::alloc::Layout::from_size_align_unchecked(1, layout.align());
        }
        let result = std::alloc::alloc(layout);
        #[cfg(feature = "alloc-backtrace")]
        println!(
            "* Alloc {:p} ({:?}):\n{}",
            result,
            layout,
            std::backtrace::Backtrace::force_capture()
        );
        result
    }
}

/// # Safety
pub unsafe fn non_zero_dealloc(ptr: *mut u8, mut layout: std::alloc::Layout) {
    unsafe {
        if layout.size() == 0 {
            layout = std::alloc::Layout::from_size_align_unchecked(1, layout.align());
        }
        #[cfg(feature = "alloc-backtrace")]
        println!(
            "* Dealloc {:p} ({:?}):\n{}",
            ptr,
            layout,
            std::backtrace::Backtrace::force_capture()
        );
        std::alloc::dealloc(ptr, layout);
    }
}

/// # Safety
pub unsafe fn non_zero_realloc(
    ptr: *mut u8,
    mut layout: std::alloc::Layout,
    new_size: usize,
) -> *mut u8 {
    unsafe {
        if layout.size() == 0 {
            layout = std::alloc::Layout::from_size_align_unchecked(1, layout.align());
        }
        let result = std::alloc::realloc(ptr, layout, new_size);
        #[cfg(feature = "alloc-backtrace")]
        println!(
            "* Realloc {:p} -> {:p} ({:?}):\n{}",
            ptr,
            result,
            layout,
            std::backtrace::Backtrace::force_capture()
        );
        result
    }
}

/// # Safety
pub unsafe fn non_zero_alloc_zeroed(mut layout: std::alloc::Layout) -> *mut u8 {
    unsafe {
        if layout.size() == 0 {
            layout = std::alloc::Layout::from_size_align_unchecked(1, layout.align());
        }
        let result = std::alloc::alloc_zeroed(layout);
        #[cfg(feature = "alloc-backtrace")]
        println!(
            "* Alloc zeroed {:p} ({:?}):\n{}",
            result,
            layout,
            std::backtrace::Backtrace::force_capture()
        );
        result
    }
}

#[macro_export]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{marker::PhantomPinned, rc::Rc};

    struct Foo;

    #[test]
    fn test_does_implement_trait() {
        assert!(is_send::<i32>());
        assert!(!is_send::<Rc<i32>>());

        assert!(is_sync::<i32>());
        assert!(!is_sync::<Rc<i32>>());

        assert!(is_copy::<i32>());
        assert!(!is_copy::<Foo>());

        assert!(is_clone::<i32>());
        assert!(!is_clone::<Foo>());

        assert!(is_sized::<[i32; 1]>());

        assert!(is_unpin::<&i32>());
        assert!(!is_unpin::<PhantomPinned>());

        assert!(is_to_string::<i32>());
        assert!(!is_to_string::<Foo>());
    }
}
