//! Type-erased data primitives that the rest of Intuicio is built on.
//!
//! Intuicio moves values around without knowing their Rust types at compile
//! time, so every value is handled as raw bytes plus a [`type_hash::TypeHash`]
//! and a drop function. This crate provides the containers that do that:
//!
//! - [`data_stack`] - the stack and register storage used to pass values
//!   between function calls.
//! - [`lifetime`] - runtime borrow checking for values that outlive Rust's
//!   static lifetimes.
//! - [`managed`] - reference counted and garbage collected value boxes.
//! - [`shared`] - thin `Rc<RefCell>` / `Arc<RwLock>` wrappers.
//! - [`type_hash`] - cheap runtime type identity.
//!
//! # Features
//!
//! - `alloc-backtrace` - print a backtrace on every allocation made through
//!   the `non_zero_*` helpers.
//! - `typehash_debug_name` - keep the type name inside [`type_hash::TypeHash`]
//!   for readable diagnostics.
pub mod data_stack;
pub mod lifetime;
pub mod managed;
pub mod shared;
pub mod type_hash;

/// Writes a default value through a raw pointer.
///
/// Type-erased containers cannot call [`Default::default`], so they call
/// [`Initialize::initialize_raw`] through a function pointer. Implemented for
/// every [`Default`] type.
pub trait Initialize: Sized {
    /// Returns the initial value of this type.
    fn initialize() -> Self;

    /// Writes the initial value into already allocated memory.
    ///
    /// # Safety
    ///
    /// `data` must be non-null, writable and aligned for `Self`, and must not
    /// hold an initialized value already (the old value is overwritten without
    /// being dropped).
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

/// Drops a value through a raw pointer.
///
/// Type-erased containers keep [`Finalize::finalize_raw`] as a function
/// pointer and call it when the value goes away. Implemented for every type.
pub trait Finalize: Sized {
    /// Drops the value stored at `data` in place.
    ///
    /// # Safety
    ///
    /// `data` must point at an initialized `Self` that nothing else reads
    /// afterwards. The value is read out unaligned and dropped, so calling this
    /// twice on the same pointer is a double free.
    unsafe fn finalize_raw(data: *mut ()) {
        unsafe { data.cast::<Self>().read_unaligned() };
    }
}

impl<T> Finalize for T {}

/// Drops a value whose type has no Rust counterpart.
///
/// A Rust type drops itself through [`Finalize::finalize_raw`], which is a
/// plain function pointer. A runtime type has no such function, because the
/// type is a list of fields that is known only at runtime. The object that
/// holds the field list implements this trait instead.
///
/// `intuicio-core` implements it on `Type`, which drops each field in turn.
pub trait Destructor: Send + Sync {
    /// Drops the value at `pointer` in place.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Finalize::finalize_raw`]: `pointer` must hold an
    /// initialized value of the described type that nothing reads afterwards,
    /// and calling this twice on one pointer is a double free.
    unsafe fn destroy(&self, pointer: *mut ());
}

/// How an owning box drops the value it holds.
///
/// A Rust type gives a function pointer. A runtime type needs its field list,
/// so it keeps a [`Destructor`] alive for as long as a value of that type
/// exists.
///
/// An `unsafe fn(*mut ())` converts into [`Finalizer::Native`].
#[derive(Clone)]
pub enum Finalizer {
    /// A Rust type's own destructor.
    Native(unsafe fn(*mut ())),
    /// A runtime type's field walker, kept alive by the value that needs it.
    Runtime(std::sync::Arc<dyn Destructor>),
}

impl Finalizer {
    /// The finalizer for a Rust type.
    pub fn of<T: Finalize>() -> Self {
        Self::Native(T::finalize_raw)
    }

    /// Drops the value at `pointer` in place.
    ///
    /// # Safety
    ///
    /// Same conditions as [`Finalize::finalize_raw`].
    pub unsafe fn finalize(&self, pointer: *mut ()) {
        match self {
            Self::Native(function) => unsafe { function(pointer) },
            Self::Runtime(destructor) => unsafe { destructor.destroy(pointer) },
        }
    }

    /// Whether this drops the value through a [`Destructor`] instead of a
    /// function pointer.
    pub fn is_runtime(&self) -> bool {
        matches!(self, Self::Runtime(_))
    }

    /// The plain function pointer, when there is one.
    ///
    /// Returns [`None`] for a runtime type, which has no such function pointer.
    pub fn as_native(&self) -> Option<unsafe fn(*mut ())> {
        match self {
            Self::Native(function) => Some(*function),
            Self::Runtime(_) => None,
        }
    }
}

impl From<unsafe fn(*mut ())> for Finalizer {
    fn from(value: unsafe fn(*mut ())) -> Self {
        Self::Native(value)
    }
}

impl From<std::sync::Arc<dyn Destructor>> for Finalizer {
    fn from(value: std::sync::Arc<dyn Destructor>) -> Self {
        Self::Runtime(value)
    }
}

impl std::fmt::Debug for Finalizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native(_) => f.write_str("Finalizer::Native"),
            Self::Runtime(_) => f.write_str("Finalizer::Runtime"),
        }
    }
}

/// Returns how many bytes must be skipped after `pointer` to reach an address
/// aligned to `alignment`.
///
/// Returns `0` when the pointer is already aligned.
#[inline]
pub fn pointer_alignment_padding(pointer: *const u8, alignment: usize) -> usize {
    let mut result = (pointer as usize) % alignment;
    if result > 0 {
        result = alignment - result;
    }
    result
}

/// [`std::alloc::alloc`] that accepts zero-sized layouts.
///
/// A zero-sized layout is bumped to one byte, because the standard allocator
/// refuses zero-sized allocations. Intuicio stores zero-sized types often.
///
/// # Safety
///
/// Same as [`std::alloc::alloc`]. The returned pointer must be freed with
/// [`non_zero_dealloc`] using the same layout.
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

/// [`std::alloc::dealloc`] counterpart of [`non_zero_alloc`].
///
/// # Safety
///
/// Same as [`std::alloc::dealloc`]. `layout` must be the layout the pointer
/// was allocated with, before the zero-size bump is applied.
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

/// [`std::alloc::realloc`] counterpart of [`non_zero_alloc`].
///
/// # Safety
///
/// Same as [`std::alloc::realloc`]. `layout` must be the current layout of
/// `ptr` and `new_size` must be non-zero.
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

/// [`std::alloc::alloc_zeroed`] that accepts zero-sized layouts.
///
/// # Safety
///
/// Same as [`std::alloc::alloc_zeroed`]. The returned pointer must be freed
/// with [`non_zero_dealloc`] using the same layout.
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
