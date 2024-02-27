use crate::{
    managed::{ManagedLazy, ManagedRef, ManagedRefMut},
    prelude::{Lifetime, ValueReadAccess, ValueWriteAccess},
    type_hash::TypeHash,
    Finalize,
};
use std::{
    alloc::{alloc, dealloc, Layout},
    cell::RefCell,
    collections::HashMap,
    ptr::NonNull,
};

const MEMORY_CHUNK_SIZE: usize = 128;
const MEMORY_PAGE_SIZE: usize = MEMORY_CHUNK_SIZE * u128::BITS as usize;

thread_local! {
    static STORAGE: RefCell<ManagedStorage> = Default::default();
}

pub fn managed_storage_stats() -> ManagedStorageStats {
    STORAGE.with_borrow(|storage| storage.stats())
}

enum ManagedObjectHeader {
    Occupied {
        id: usize,
        type_hash: TypeHash,
        lifetime: Lifetime,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
        instances_count: usize,
    },
    Free,
}

#[derive(Debug, Default, Clone, Copy)]
struct OccupancyMap {
    // each bit represents single memory chunk occupancy.
    map: u128,
}

impl OccupancyMap {
    fn occuppy(&mut self, range: OccupancyRange) {
        self.map |= range.mask;
    }

    fn free(&mut self, range: OccupancyRange) {
        self.map &= !range.mask;
    }

    fn is_free(&self, range: OccupancyRange) -> bool {
        self.map & range.mask == 0
    }

    fn find_free_space(
        &self,
        object_with_header_size: usize,
        range: OccupancyRange,
    ) -> Option<OccupancyRange> {
        if object_with_header_size > range.byte_size() {
            return None;
        }
        if self.is_free(range) {
            return range.cut(object_with_header_size);
        }
        let (left, right) = range.split()?;
        let left = self.find_free_space(object_with_header_size, left);
        let right = self.find_free_space(object_with_header_size, right);
        match (left, right) {
            (None, None) => None,
            (None, Some(right)) => Some(right),
            (Some(left), None) => Some(left),
            (Some(left), Some(right)) => {
                if right.byte_size() < left.byte_size() {
                    Some(right)
                } else {
                    Some(left)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct OccupancyRange {
    bits_start_inclusive: usize,
    bits_end_exclusive: usize,
    mask: u128,
}

impl Default for OccupancyRange {
    fn default() -> Self {
        Self {
            bits_start_inclusive: 0,
            bits_end_exclusive: u128::BITS as _,
            mask: u128::MAX,
        }
    }
}

impl OccupancyRange {
    fn byte_offset(&self) -> usize {
        self.bits_start_inclusive * MEMORY_CHUNK_SIZE
    }

    fn byte_size(&self) -> usize {
        (self.bits_end_exclusive - self.bits_start_inclusive) * MEMORY_CHUNK_SIZE
    }

    fn update_mask(mut self) -> Self {
        let size = self.bits_end_exclusive - self.bits_start_inclusive;
        self.mask = if size == u128::BITS as _ {
            u128::MAX
        } else {
            (!u128::MAX.wrapping_shl(size as _)).wrapping_shl(self.bits_start_inclusive as _)
        };
        self
    }

    fn cut(&self, object_with_header_size: usize) -> Option<Self> {
        let size = object_with_header_size.next_power_of_two() / MEMORY_CHUNK_SIZE;
        if size <= self.byte_size() {
            Some(
                Self {
                    bits_start_inclusive: self.bits_start_inclusive,
                    bits_end_exclusive: self.bits_start_inclusive + size,
                    mask: 0,
                }
                .update_mask(),
            )
        } else {
            None
        }
    }

    fn split(&self) -> Option<(Self, Self)> {
        let half_size = (self.bits_end_exclusive - self.bits_start_inclusive) / 2;
        if half_size == 0 {
            return None;
        }
        let start = self.bits_start_inclusive;
        let mid = self.bits_start_inclusive + half_size;
        let end = self.bits_end_exclusive;
        Some((
            Self {
                bits_start_inclusive: start,
                bits_end_exclusive: mid,
                mask: 0,
            }
            .update_mask(),
            Self {
                bits_start_inclusive: mid,
                bits_end_exclusive: end,
                mask: 0,
            }
            .update_mask(),
        ))
    }

    fn from_pointer_size(memory: NonNull<u8>, pointer: NonNull<u8>, size: usize) -> Self {
        let offset = pointer.as_ptr() as usize - memory.as_ptr() as usize;
        let from = offset / MEMORY_CHUNK_SIZE;
        let to = from + size.next_power_of_two() / MEMORY_CHUNK_SIZE;
        Self {
            bits_start_inclusive: from,
            bits_end_exclusive: to,
            mask: 0,
        }
        .update_mask()
    }
}

enum ManagedMemoryPage {
    Chunked {
        memory: NonNull<u8>,
        layout: Layout,
        occupancy: OccupancyMap,
    },
    Exclusive {
        memory: NonNull<u8>,
        layout: Layout,
    },
}

impl Drop for ManagedMemoryPage {
    fn drop(&mut self) {
        unsafe {
            match self {
                ManagedMemoryPage::Chunked { memory, layout, .. } => {
                    dealloc(memory.as_ptr(), *layout);
                }
                ManagedMemoryPage::Exclusive { memory, layout } => {
                    dealloc(memory.as_ptr(), *layout);
                }
            }
        }
    }
}

impl ManagedMemoryPage {
    fn new_chunked() -> Self {
        let layout = Layout::from_size_align(MEMORY_PAGE_SIZE, 1).unwrap();
        unsafe {
            let memory = NonNull::new_unchecked(alloc(layout));
            memory
                .as_ptr()
                .cast::<ManagedObjectHeader>()
                .write(ManagedObjectHeader::Free);
            Self::Chunked {
                memory,
                layout,
                occupancy: Default::default(),
            }
        }
    }

    fn new_exclusive(size: usize) -> Self {
        unsafe {
            let header_layout = Layout::new::<ManagedObjectHeader>().pad_to_align();
            let layout = Layout::from_size_align_unchecked(header_layout.size() + size, 1);
            let memory = NonNull::new_unchecked(alloc(layout));
            memory
                .as_ptr()
                .cast::<ManagedObjectHeader>()
                .write(ManagedObjectHeader::Free);
            Self::Exclusive { layout, memory }
        }
    }

    fn alloc_uninitialized(
        &mut self,
        id: usize,
        page: usize,
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Option<DynamicManagedBox> {
        let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
        match self {
            ManagedMemoryPage::Chunked {
                memory, occupancy, ..
            } => unsafe {
                let range = occupancy
                    .find_free_space(header_size + layout.size(), OccupancyRange::default())?;
                occupancy.occuppy(range);
                let memory = memory.as_ptr().add(range.byte_offset());
                memory
                    .cast::<ManagedObjectHeader>()
                    .write(ManagedObjectHeader::Occupied {
                        id,
                        type_hash,
                        lifetime: Default::default(),
                        layout,
                        finalizer,
                        instances_count: 1,
                    });
                Some(DynamicManagedBox {
                    memory: NonNull::new_unchecked(memory.add(header_size)),
                    id,
                    page,
                    drop: true,
                })
            },
            ManagedMemoryPage::Exclusive { memory, .. } => unsafe {
                memory.as_ptr().cast::<ManagedObjectHeader>().write(
                    ManagedObjectHeader::Occupied {
                        id,
                        type_hash,
                        lifetime: Default::default(),
                        layout,
                        finalizer,
                        instances_count: 1,
                    },
                );
                Some(DynamicManagedBox {
                    memory: NonNull::new_unchecked(memory.as_ptr().add(header_size)),
                    id,
                    page,
                    drop: true,
                })
            },
        }
    }

    fn owns_pointer(&self, pointer: NonNull<u8>) -> bool {
        let (from, to) = unsafe {
            match self {
                ManagedMemoryPage::Chunked { memory, layout, .. } => {
                    (memory.as_ptr(), memory.as_ptr().add(layout.size()))
                }
                ManagedMemoryPage::Exclusive { memory, layout } => {
                    (memory.as_ptr(), memory.as_ptr().add(layout.size()))
                }
            }
        };
        pointer.as_ptr() >= from && pointer.as_ptr() < to
    }

    fn total_size(&self) -> usize {
        match self {
            ManagedMemoryPage::Chunked { layout, .. }
            | ManagedMemoryPage::Exclusive { layout, .. } => layout.size(),
        }
    }

    fn occupied_size(&self) -> usize {
        match self {
            ManagedMemoryPage::Chunked { occupancy, .. } => {
                occupancy.map.count_ones() as usize * MEMORY_CHUNK_SIZE
            }
            ManagedMemoryPage::Exclusive { layout, .. } => layout.size(),
        }
    }

    fn free_size(&self) -> usize {
        match self {
            ManagedMemoryPage::Chunked { occupancy, .. } => {
                occupancy.map.count_zeros() as usize * MEMORY_CHUNK_SIZE
            }
            ManagedMemoryPage::Exclusive { .. } => 0,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ManagedStorageStats {
    pub pages_count: usize,
    pub chunked_pages_count: usize,
    pub exclusive_pages_count: usize,
    pub total_size: usize,
    pub occupied_size: usize,
    pub free_size: usize,
}

#[derive(Default)]
struct ManagedStorage {
    object_id_generator: usize,
    page_id_generator: usize,
    pages: HashMap<usize, ManagedMemoryPage>,
}

impl ManagedStorage {
    fn stats(&self) -> ManagedStorageStats {
        ManagedStorageStats {
            pages_count: self.pages.len(),
            chunked_pages_count: self
                .pages
                .values()
                .filter(|page| matches!(page, ManagedMemoryPage::Chunked { .. }))
                .count(),
            exclusive_pages_count: self
                .pages
                .values()
                .filter(|page| matches!(page, ManagedMemoryPage::Exclusive { .. }))
                .count(),
            total_size: self.pages.values().map(|page| page.total_size()).sum(),
            occupied_size: self.pages.values().map(|page| page.occupied_size()).sum(),
            free_size: self.pages.values().map(|page| page.free_size()).sum(),
        }
    }

    fn generate_object_id(&mut self) -> usize {
        let result = self.object_id_generator;
        self.object_id_generator = self.object_id_generator.wrapping_add(1);
        result
    }

    fn generate_page_id(&mut self) -> usize {
        let result = self.page_id_generator;
        self.page_id_generator = self.page_id_generator.wrapping_add(1);
        result
    }

    fn alloc_uninitialized(
        &mut self,
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> DynamicManagedBox {
        let id = self.generate_object_id();
        let size = layout.size() + Layout::new::<ManagedObjectHeader>().size();
        if size > MEMORY_PAGE_SIZE {
            let page_id = self.generate_page_id();
            let mut page = ManagedMemoryPage::new_exclusive(size);
            let object = page
                .alloc_uninitialized(id, page_id, type_hash, layout, finalizer)
                .unwrap();
            self.pages.insert(page_id, page);
            object
        } else {
            for (page_id, page) in &mut self.pages {
                if let Some(object) =
                    page.alloc_uninitialized(id, *page_id, type_hash, layout, finalizer)
                {
                    return object;
                }
            }
            let page_id = self.generate_page_id();
            let mut page = ManagedMemoryPage::new_chunked();
            let object = page
                .alloc_uninitialized(id, page_id, type_hash, layout, finalizer)
                .unwrap();
            self.pages.insert(page_id, page);
            object
        }
    }

    fn increment(&mut self, object_id: usize, page_id: usize, pointer: NonNull<u8>) {
        if let Some(page) = self.pages.get(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                unsafe {
                    let header = pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap();
                    if let ManagedObjectHeader::Occupied {
                        id,
                        instances_count,
                        ..
                    } = header
                    {
                        if object_id == *id {
                            *instances_count += 1;
                        }
                    }
                }
            }
        }
    }

    fn decrement(&mut self, object_id: usize, page_id: usize, pointer: NonNull<u8>) {
        if let Some(page) = self.pages.get_mut(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                unsafe {
                    let header = pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap();
                    if let ManagedObjectHeader::Occupied {
                        id,
                        lifetime,
                        layout,
                        finalizer,
                        instances_count,
                        ..
                    } = header
                    {
                        if object_id == *id && *instances_count > 0 {
                            *instances_count -= 1;
                            if *instances_count == 0 {
                                (finalizer)(pointer.as_ptr().cast::<()>());
                                std::mem::take(lifetime);
                                match page {
                                    ManagedMemoryPage::Chunked {
                                        memory, occupancy, ..
                                    } => {
                                        let range = OccupancyRange::from_pointer_size(
                                            *memory,
                                            pointer,
                                            header_size + layout.size(),
                                        );
                                        occupancy.free(range);
                                        if occupancy.is_free(OccupancyRange::default()) {
                                            self.pages.remove(&page_id);
                                        }
                                    }
                                    ManagedMemoryPage::Exclusive { .. } => {
                                        self.pages.remove(&page_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn access_object_lifetime<T>(
        &self,
        pointer: NonNull<u8>,
        object_id: usize,
        page_id: usize,
    ) -> Option<(NonNull<T>, NonNull<Lifetime>)> {
        if let Some(page) = self.pages.get(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                let header = unsafe {
                    pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap()
                };
                if let ManagedObjectHeader::Occupied {
                    id,
                    type_hash,
                    lifetime,
                    instances_count,
                    ..
                } = header
                {
                    if object_id == *id && *instances_count > 0 && *type_hash == TypeHash::of::<T>()
                    {
                        return unsafe {
                            Some((pointer.cast::<T>(), NonNull::new_unchecked(lifetime)))
                        };
                    }
                }
            }
        }
        None
    }

    fn object_type_hash(
        &self,
        pointer: NonNull<u8>,
        object_id: usize,
        page_id: usize,
    ) -> Option<TypeHash> {
        if let Some(page) = self.pages.get(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                let header = unsafe {
                    pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap()
                };
                if let ManagedObjectHeader::Occupied {
                    id,
                    type_hash,
                    instances_count,
                    ..
                } = header
                {
                    if object_id == *id && *instances_count > 0 {
                        return Some(*type_hash);
                    }
                }
            }
        }
        None
    }

    fn object_layout(
        &self,
        pointer: NonNull<u8>,
        object_id: usize,
        page_id: usize,
    ) -> Option<Layout> {
        if let Some(page) = self.pages.get(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                let header = unsafe {
                    pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap()
                };
                if let ManagedObjectHeader::Occupied {
                    id,
                    layout,
                    instances_count,
                    ..
                } = header
                {
                    if object_id == *id && *instances_count > 0 {
                        return Some(*layout);
                    }
                }
            }
        }
        None
    }

    fn object_instances_count(
        &self,
        pointer: NonNull<u8>,
        object_id: usize,
        page_id: usize,
    ) -> usize {
        if let Some(page) = self.pages.get(&page_id) {
            if page.owns_pointer(pointer) {
                let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
                let header = unsafe {
                    pointer
                        .as_ptr()
                        .sub(header_size)
                        .cast::<ManagedObjectHeader>()
                        .as_mut()
                        .unwrap()
                };
                if let ManagedObjectHeader::Occupied {
                    id,
                    instances_count,
                    ..
                } = header
                {
                    if object_id == *id {
                        return *instances_count;
                    }
                }
            }
        }
        0
    }
}

pub struct ManagedBox<T> {
    memory: NonNull<T>,
    id: usize,
    page: usize,
    drop: bool,
}

impl<T: Default> Default for ManagedBox<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Drop for ManagedBox<T> {
    fn drop(&mut self) {
        if self.drop {
            STORAGE.with_borrow_mut(|storage| {
                storage.decrement(self.id, self.page, self.memory.cast());
            })
        }
    }
}

impl<T> ManagedBox<T> {
    pub fn new(value: T) -> Self
    where
        T: Finalize,
    {
        unsafe {
            STORAGE.with_borrow_mut(|storage| {
                let mut result = storage.alloc_uninitialized(
                    TypeHash::of::<T>(),
                    Layout::new::<T>().pad_to_align(),
                    T::finalize_raw,
                );
                result.drop = false;
                let result = Self {
                    memory: result.memory.cast(),
                    id: result.id,
                    page: result.page,
                    drop: true,
                };
                result.memory.as_ptr().write(value);
                result
            })
        }
    }

    pub fn into_dynamic(mut self) -> DynamicManagedBox {
        self.drop = false;
        DynamicManagedBox {
            memory: self.memory.cast(),
            id: self.id,
            page: self.page,
            drop: true,
        }
    }

    pub fn instances_count(&self) -> usize {
        STORAGE.with_borrow(|storage| {
            storage.object_instances_count(self.memory.cast(), self.id, self.page)
        })
    }

    pub fn type_hash(&self) -> TypeHash {
        STORAGE.with_borrow(|storage| {
            storage
                .object_type_hash(self.memory.cast(), self.id, self.page)
                .unwrap()
        })
    }

    pub fn does_share_reference(&self, other: &Self) -> bool {
        self.id == other.id && self.page == other.page && self.memory == other.memory
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .unwrap()
        });
        unsafe {
            Some(ManagedRef::new(
                pointer.as_ref(),
                lifetime.as_ref().borrow()?,
            ))
        }
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        let (mut pointer, mut lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .unwrap()
        });
        unsafe {
            Some(ManagedRefMut::new(
                pointer.as_mut(),
                lifetime.as_mut().borrow_mut()?,
            ))
        }
    }

    pub fn lazy(&self) -> ManagedLazy<T> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .unwrap()
        });
        unsafe { ManagedLazy::new(pointer.as_ref(), lifetime.as_ref().lazy()) }
    }

    pub fn read(&self) -> Option<ValueReadAccess<T>> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .unwrap()
        });
        unsafe { lifetime.as_ref().read(pointer.as_ref()) }
    }

    pub fn write(&mut self) -> Option<ValueWriteAccess<T>> {
        let (mut pointer, mut lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .unwrap()
        });
        unsafe { lifetime.as_mut().write(pointer.as_mut()) }
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        STORAGE.with_borrow(|storage| {
            if storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .is_some()
            {
                Some(self.memory.as_ptr().cast_const())
            } else {
                None
            }
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_mut(&mut self) -> Option<*mut T> {
        STORAGE.with_borrow(|storage| {
            if storage
                .access_object_lifetime::<T>(self.memory.cast(), self.id, self.page)
                .is_some()
            {
                Some(self.memory.as_ptr())
            } else {
                None
            }
        })
    }
}

impl<T> Clone for ManagedBox<T> {
    fn clone(&self) -> Self {
        STORAGE.with_borrow_mut(|storage| {
            storage.increment(self.id, self.page, self.memory.cast());
            Self {
                memory: self.memory,
                id: self.id,
                page: self.page,
                drop: true,
            }
        })
    }
}

pub struct DynamicManagedBox {
    memory: NonNull<u8>,
    id: usize,
    page: usize,
    drop: bool,
}

impl Drop for DynamicManagedBox {
    fn drop(&mut self) {
        if self.drop {
            STORAGE.with_borrow_mut(|storage| {
                storage.decrement(self.id, self.page, self.memory);
            })
        }
    }
}

impl DynamicManagedBox {
    pub fn new<T: Finalize>(value: T) -> Self {
        unsafe {
            let result =
                Self::new_uninitialized(TypeHash::of::<T>(), Layout::new::<T>(), T::finalize_raw);
            result.memory.as_ptr().cast::<T>().write(value);
            result
        }
    }

    pub fn new_uninitialized(
        type_hash: TypeHash,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
    ) -> Self {
        STORAGE.with_borrow_mut(|storage| {
            storage.alloc_uninitialized(type_hash, layout.pad_to_align(), finalizer)
        })
    }

    pub fn into_typed<T>(mut self) -> Result<ManagedBox<T>, Self> {
        if self.is::<T>() {
            self.drop = false;
            Ok(ManagedBox {
                memory: self.memory.cast(),
                id: self.id,
                page: self.page,
                drop: true,
            })
        } else {
            Err(self)
        }
    }

    pub fn instances_count(&self) -> usize {
        STORAGE
            .with_borrow(|storage| storage.object_instances_count(self.memory, self.id, self.page))
    }

    pub fn type_hash(&self) -> TypeHash {
        STORAGE.with_borrow(|storage| {
            storage
                .object_type_hash(self.memory, self.id, self.page)
                .unwrap()
        })
    }

    pub fn does_share_reference(&self, other: &Self) -> bool {
        self.id == other.id && self.page == other.page && self.memory == other.memory
    }

    pub fn is<T>(&self) -> bool {
        STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .is_some()
        })
    }

    pub fn borrow<T>(&self) -> Option<ManagedRef<T>> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .unwrap()
        });
        unsafe {
            Some(ManagedRef::new(
                pointer.as_ref(),
                lifetime.as_ref().borrow()?,
            ))
        }
    }

    pub fn borrow_mut<T>(&mut self) -> Option<ManagedRefMut<T>> {
        let (mut pointer, mut lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .unwrap()
        });
        unsafe {
            Some(ManagedRefMut::new(
                pointer.as_mut(),
                lifetime.as_mut().borrow_mut()?,
            ))
        }
    }

    pub fn lazy<T>(&self) -> ManagedLazy<T> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .unwrap()
        });
        unsafe { ManagedLazy::new(pointer.as_ref(), lifetime.as_ref().lazy()) }
    }

    pub fn read<T>(&self) -> Option<ValueReadAccess<T>> {
        let (pointer, lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .unwrap()
        });
        unsafe { lifetime.as_ref().read(pointer.as_ref()) }
    }

    pub fn write<T>(&mut self) -> Option<ValueWriteAccess<T>> {
        let (mut pointer, mut lifetime) = STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .unwrap()
        });
        unsafe { lifetime.as_mut().write(pointer.as_mut()) }
    }

    /// # Safety
    pub unsafe fn memory(&self) -> Option<&[u8]> {
        STORAGE.with_borrow(|storage| {
            storage
                .object_layout(self.memory, self.id, self.page)
                .map(|layout| std::slice::from_raw_parts(self.memory.as_ptr(), layout.size()))
        })
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> Option<&mut [u8]> {
        STORAGE.with_borrow(|storage| {
            if let Some(layout) = storage.object_layout(self.memory, self.id, self.page) {
                Some(std::slice::from_raw_parts_mut(
                    self.memory.as_ptr(),
                    layout.size(),
                ))
            } else {
                None
            }
        })
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        STORAGE.with_borrow(|storage| {
            if storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .is_some()
            {
                Some(self.memory.as_ptr().cast_const().cast())
            } else {
                None
            }
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_mut<T>(&mut self) -> Option<*mut T> {
        STORAGE.with_borrow(|storage| {
            if storage
                .access_object_lifetime::<T>(self.memory, self.id, self.page)
                .is_some()
            {
                Some(self.memory.as_ptr().cast())
            } else {
                None
            }
        })
    }
}

impl Clone for DynamicManagedBox {
    fn clone(&self) -> Self {
        STORAGE.with_borrow_mut(|storage| {
            storage.increment(self.id, self.page, self.memory);
            Self {
                memory: self.memory,
                id: self.id,
                page: self.page,
                drop: true,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_occupancy_range() {
        let v = OccupancyRange::default();
        assert_eq!(v.mask, 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 0..128);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE);
        let v = v.split().unwrap().0;
        assert_eq!(v.mask, 0x0000000000000000FFFFFFFFFFFFFFFF);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 0..64);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 2);
        let v = v.split().unwrap().1;
        assert_eq!(v.mask, 0x0000000000000000FFFFFFFF00000000);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 32..64);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 4);
        let v = v.split().unwrap().0;
        assert_eq!(v.mask, 0x00000000000000000000FFFF00000000);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 32..48);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 8);
        let v = v.split().unwrap().1;
        assert_eq!(v.mask, 0x00000000000000000000FF0000000000);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 40..48);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 16);
        let v = v.split().unwrap().0;
        assert_eq!(v.mask, 0x000000000000000000000F0000000000);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 40..44);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 32);
    }

    #[test]
    fn test_occupancy_map() {
        let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
        let mut map = OccupancyMap::default();

        let range = map
            .find_free_space(
                std::mem::size_of::<f32>() + header_size,
                OccupancyRange::default(),
            )
            .unwrap();
        map.occuppy(range);
        assert_eq!(range.bits_start_inclusive..range.bits_end_exclusive, 0..1);

        let range = map
            .find_free_space(
                std::mem::size_of::<u8>() + header_size,
                OccupancyRange::default(),
            )
            .unwrap();
        map.occuppy(range);
        assert_eq!(range.bits_start_inclusive..range.bits_end_exclusive, 1..2);
    }

    #[test]
    fn test_managed_box() {
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
        let a = ManagedBox::new(42usize);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert_eq!(*a.read().unwrap(), 42);
        assert_eq!(a.instances_count(), 1);
        let mut b = a.clone();
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert_eq!(a.instances_count(), 2);
        assert_eq!(b.instances_count(), 2);
        assert!(a.does_share_reference(&b));
        assert_eq!(*b.read().unwrap(), 42);
        *b.write().unwrap() = 10;
        assert_eq!(*a.read().unwrap(), 10);
        assert_eq!(*b.read().unwrap(), 10);
        drop(a);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert_eq!(b.instances_count(), 1);
        drop(b);
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
    }

    #[test]
    fn test_dynamic_managed_box() {
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
        let a = DynamicManagedBox::new(42usize);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert!(a.is::<usize>());
        assert_eq!(*a.read::<usize>().unwrap(), 42);
        assert_eq!(a.instances_count(), 1);
        let mut b = a.clone();
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert!(b.is::<usize>());
        assert_eq!(a.instances_count(), 2);
        assert_eq!(b.instances_count(), 2);
        assert!(a.does_share_reference(&b));
        assert_eq!(*b.read::<usize>().unwrap(), 42);
        *b.write::<usize>().unwrap() = 10;
        assert_eq!(*a.read::<usize>().unwrap(), 10);
        assert_eq!(*b.read::<usize>().unwrap(), 10);
        drop(a);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 128,
                free_size: 16256,
                ..Default::default()
            }
        );
        assert_eq!(b.instances_count(), 1);
        drop(b);
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
    }

    #[test]
    fn test_growing_allocations() {
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
        let a = ManagedBox::<[u64; 10]>::new(std::array::from_fn(|index| index as _));
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 256,
                free_size: 16128,
                ..Default::default()
            }
        );
        let b = ManagedBox::<[u64; 100]>::new(std::array::from_fn(|index| index as _));
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 1280,
                free_size: 15104,
                ..Default::default()
            }
        );
        let c = ManagedBox::<[u64; 1000]>::new(std::array::from_fn(|index| index as _));
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 1,
                total_size: 16384,
                occupied_size: 9472,
                free_size: 6912,
                ..Default::default()
            }
        );
        let d = ManagedBox::<[u64; 10000]>::new(std::array::from_fn(|index| index as _));
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 2,
                chunked_pages_count: 1,
                exclusive_pages_count: 1,
                total_size: 96560,
                occupied_size: 89648,
                free_size: 6912,
                ..Default::default()
            }
        );
        drop(a);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 2,
                chunked_pages_count: 1,
                exclusive_pages_count: 1,
                total_size: 96560,
                occupied_size: 89392,
                free_size: 7168,
                ..Default::default()
            }
        );
        drop(b);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 2,
                chunked_pages_count: 1,
                exclusive_pages_count: 1,
                total_size: 96560,
                occupied_size: 88368,
                free_size: 8192,
                ..Default::default()
            }
        );
        drop(c);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 0,
                exclusive_pages_count: 1,
                total_size: 80176,
                occupied_size: 80176,
                free_size: 0,
                ..Default::default()
            }
        );
        drop(d);
        assert_eq!(managed_storage_stats(), ManagedStorageStats::default());
    }

    #[test]
    fn test_managed_box_borrows() {
        let v = ManagedBox::new(42usize);
        let r = v.borrow().unwrap();
        drop(v);
        assert!(r.read().is_none());
    }
}
