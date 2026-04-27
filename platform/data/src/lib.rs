pub mod data_stack;
pub mod lifetime;
pub mod managed;
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
