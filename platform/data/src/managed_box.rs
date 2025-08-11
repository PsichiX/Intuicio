use crate::{
    Finalize,
    lifetime::{
        Lifetime, LifetimeLazy, LifetimeRef, LifetimeRefMut, ValueReadAccess, ValueWriteAccess,
    },
    managed::{
        DynamicManagedLazy, DynamicManagedRef, DynamicManagedRefMut, ManagedLazy, ManagedRef,
        ManagedRefMut,
    },
    non_zero_alloc, non_zero_dealloc, pointer_alignment_padding,
    type_hash::TypeHash,
};
use std::{
    alloc::Layout, cell::RefCell, collections::HashMap, future::poll_fn, ops::Range, task::Poll,
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
        padding: u8,
    },
    Free,
}

impl std::fmt::Debug for ManagedObjectHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Occupied {
                id,
                type_hash,
                layout,
                finalizer,
                instances_count,
                ..
            } => f
                .debug_struct("Occupied")
                .field("id", id)
                .field("type_hash", type_hash)
                .field("layout", layout)
                .field("finalizer", finalizer)
                .field("instances_count", instances_count)
                .finish_non_exhaustive(),
            Self::Free => write!(f, "Free"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct OccupancyMap {
    // each bit represents single memory chunk occupancy.
    mask: u128,
}

impl OccupancyMap {
    fn occuppy(&mut self, range: OccupancyRange) {
        self.mask |= range.mask;
    }

    fn free(&mut self, range: OccupancyRange) {
        self.mask &= !range.mask;
    }

    fn is_free(&self, range: OccupancyRange) -> bool {
        self.mask & range.mask == 0
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
    fn range(&self) -> Range<usize> {
        self.bits_start_inclusive..self.bits_end_exclusive
    }

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

    fn from_pointer_size(memory: *const u8, pointer: *const u8, size: usize) -> Self {
        let offset = pointer as usize - memory as usize;
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
        memory: *mut u8,
        layout: Layout,
        occupancy: OccupancyMap,
        padding: u8,
    },
    Exclusive {
        memory: *mut u8,
        layout: Layout,
        padding: u8,
    },
}

impl Drop for ManagedMemoryPage {
    fn drop(&mut self) {
        // TODO: if it somehow happen that some objects won't deallocate before page gets destroyed
        // (highly impossible), add consuming headers and finalizing objects.
        unsafe {
            match self {
                ManagedMemoryPage::Chunked { memory, layout, .. } => {
                    if memory.is_null() {
                        return;
                    }
                    non_zero_dealloc(*memory, *layout);
                }
                ManagedMemoryPage::Exclusive { memory, layout, .. } => {
                    if memory.is_null() {
                        return;
                    }
                    non_zero_dealloc(*memory, *layout);
                }
            }
        }
    }
}

impl ManagedMemoryPage {
    fn new_chunked() -> Option<Self> {
        let header_layout = Layout::new::<ManagedObjectHeader>().pad_to_align();
        let layout = Layout::from_size_align(MEMORY_PAGE_SIZE + header_layout.align(), 1).unwrap();
        unsafe {
            let memory = non_zero_alloc(layout);
            if memory.is_null() {
                None
            } else {
                let padding = pointer_alignment_padding(memory, header_layout.align());
                for offset in (0..MEMORY_PAGE_SIZE).step_by(MEMORY_CHUNK_SIZE) {
                    memory
                        .add(padding + offset)
                        .cast::<ManagedObjectHeader>()
                        .write(ManagedObjectHeader::Free);
                }
                Some(Self::Chunked {
                    memory,
                    layout,
                    occupancy: Default::default(),
                    padding: padding as u8,
                })
            }
        }
    }

    fn new_exclusive(size: usize, alignment: usize) -> Option<Self> {
        unsafe {
            let header_layout = Layout::new::<ManagedObjectHeader>().pad_to_align();
            let layout =
                Layout::from_size_align_unchecked(header_layout.size() + size + alignment, 1);
            let memory = non_zero_alloc(layout);
            if memory.is_null() {
                None
            } else {
                let padding = pointer_alignment_padding(memory, header_layout.align());
                memory
                    .add(padding)
                    .cast::<ManagedObjectHeader>()
                    .write(ManagedObjectHeader::Free);
                Some(Self::Exclusive {
                    layout,
                    memory,
                    padding: padding as u8,
                })
            }
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
        let header_layout = Layout::new::<ManagedObjectHeader>().pad_to_align();
        match self {
            ManagedMemoryPage::Chunked {
                memory,
                occupancy,
                padding,
                ..
            } => unsafe {
                let range = occupancy.find_free_space(
                    header_layout.size() + layout.size(),
                    OccupancyRange::default(),
                )?;
                let memory = memory.add(*padding as usize + range.byte_offset());
                let padding = pointer_alignment_padding(memory, layout.align());
                if header_layout.size() + layout.size() - padding > range.byte_size() {
                    return None;
                }
                occupancy.occuppy(range);
                *memory.cast::<ManagedObjectHeader>().as_mut().unwrap() =
                    ManagedObjectHeader::Occupied {
                        id,
                        type_hash,
                        lifetime: Default::default(),
                        layout,
                        finalizer,
                        instances_count: 1,
                        padding: padding as u8,
                    };
                Some(DynamicManagedBox {
                    memory,
                    id,
                    page,
                    drop: true,
                })
            },
            ManagedMemoryPage::Exclusive {
                memory, padding, ..
            } => unsafe {
                let memory = memory.add(*padding as usize);
                let padding = pointer_alignment_padding(memory, layout.align());
                *memory.cast::<ManagedObjectHeader>().as_mut().unwrap() =
                    ManagedObjectHeader::Occupied {
                        id,
                        type_hash,
                        lifetime: Default::default(),
                        layout,
                        finalizer,
                        instances_count: 1,
                        padding: padding as u8,
                    };
                Some(DynamicManagedBox {
                    memory,
                    id,
                    page,
                    drop: true,
                })
            },
        }
    }

    fn owns_pointer(&self, pointer: *const u8) -> bool {
        let (from, to) = unsafe {
            match self {
                ManagedMemoryPage::Chunked { memory, layout, .. }
                | ManagedMemoryPage::Exclusive { memory, layout, .. } => {
                    (*memory, memory.add(layout.size()))
                }
            }
        };
        pointer >= from && pointer < to
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
                occupancy.mask.count_ones() as usize * MEMORY_CHUNK_SIZE
            }
            ManagedMemoryPage::Exclusive { layout, .. } => layout.size(),
        }
    }

    fn free_size(&self) -> usize {
        match self {
            ManagedMemoryPage::Chunked { occupancy, .. } => {
                occupancy.mask.count_zeros() as usize * MEMORY_CHUNK_SIZE
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
            let mut page = ManagedMemoryPage::new_exclusive(size, layout.align()).unwrap();
            let object = page
                .alloc_uninitialized(id, page_id, type_hash, layout, finalizer)
                .unwrap();
            self.pages.insert(page_id, page);
            object
        } else {
            for (page_id, page) in &mut self.pages {
                if matches!(page, ManagedMemoryPage::Chunked { .. })
                    && let Some(object) =
                        page.alloc_uninitialized(id, *page_id, type_hash, layout, finalizer)
                {
                    return object;
                }
            }
            let page_id = self.generate_page_id();
            let mut page = ManagedMemoryPage::new_chunked().unwrap();
            let object = page
                .alloc_uninitialized(id, page_id, type_hash, layout, finalizer)
                .unwrap();
            self.pages.insert(page_id, page);
            object
        }
    }

    fn increment(&mut self, object_id: usize, page_id: usize, pointer: *mut u8) {
        if let Some(page) = self.pages.get(&page_id)
            && page.owns_pointer(pointer)
        {
            unsafe {
                let header = pointer.cast::<ManagedObjectHeader>().as_mut().unwrap();
                if let ManagedObjectHeader::Occupied {
                    id,
                    instances_count,
                    ..
                } = header
                    && object_id == *id
                {
                    *instances_count += 1;
                }
            }
        }
    }

    fn decrement(&mut self, object_id: usize, page_id: usize, pointer: *mut u8) {
        if let Some(page) = self.pages.get_mut(&page_id)
            && page.owns_pointer(pointer)
        {
            let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
            unsafe {
                let header = pointer.cast::<ManagedObjectHeader>().as_mut().unwrap();
                if let ManagedObjectHeader::Occupied {
                    id,
                    layout,
                    finalizer,
                    instances_count,
                    padding,
                    ..
                } = header
                    && object_id == *id
                    && *instances_count > 0
                {
                    *instances_count -= 1;
                    if *instances_count == 0 {
                        (finalizer)(pointer.add(header_size + *padding as usize).cast::<()>());
                        match page {
                            ManagedMemoryPage::Chunked {
                                memory,
                                occupancy,
                                padding,
                                ..
                            } => {
                                let range = OccupancyRange::from_pointer_size(
                                    memory.add(*padding as usize),
                                    pointer,
                                    header_size + layout.size(),
                                );
                                occupancy.free(range);
                                *header = ManagedObjectHeader::Free;
                                for index in range.range().skip(1) {
                                    memory
                                        .add(*padding as usize + index * MEMORY_CHUNK_SIZE)
                                        .cast::<ManagedObjectHeader>()
                                        .write(ManagedObjectHeader::Free);
                                }
                                if occupancy.is_free(OccupancyRange::default()) {
                                    self.pages.remove(&page_id);
                                }
                            }
                            ManagedMemoryPage::Exclusive { .. } => {
                                *header = ManagedObjectHeader::Free;
                                self.pages.remove(&page_id);
                            }
                        }
                    }
                }
            }
        }
    }

    fn access_object_lifetime_type<T>(
        &self,
        pointer: *mut u8,
        object_id: usize,
        page_id: usize,
        type_check: bool,
    ) -> Option<(*mut T, *mut Lifetime, TypeHash)> {
        if let Some(page) = self.pages.get(&page_id)
            && page.owns_pointer(pointer)
        {
            let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
            let header = unsafe { pointer.cast::<ManagedObjectHeader>().as_mut().unwrap() };
            if let ManagedObjectHeader::Occupied {
                id,
                type_hash,
                lifetime,
                instances_count,
                padding,
                ..
            } = header
                && object_id == *id
                && *instances_count > 0
                && (!type_check || *type_hash == TypeHash::of::<T>())
            {
                return Some((
                    unsafe { pointer.add(header_size + *padding as usize).cast::<T>() },
                    lifetime,
                    *type_hash,
                ));
            }
        }
        None
    }

    fn object_type_hash(
        &self,
        pointer: *mut u8,
        object_id: usize,
        page_id: usize,
    ) -> Option<TypeHash> {
        if let Some(page) = self.pages.get(&page_id)
            && page.owns_pointer(pointer)
        {
            let header = unsafe { pointer.cast::<ManagedObjectHeader>().as_mut().unwrap() };
            if let ManagedObjectHeader::Occupied {
                id,
                type_hash,
                instances_count,
                ..
            } = header
                && object_id == *id
                && *instances_count > 0
            {
                return Some(*type_hash);
            }
        }
        None
    }

    fn object_layout_with_offset(
        &self,
        pointer: *mut u8,
        object_id: usize,
        page_id: usize,
    ) -> Option<(Layout, usize)> {
        if let Some(page) = self.pages.get(&page_id)
            && page.owns_pointer(pointer)
        {
            let header_size = Layout::new::<ManagedObjectHeader>().pad_to_align().size();
            let header = unsafe { pointer.cast::<ManagedObjectHeader>().as_mut().unwrap() };
            if let ManagedObjectHeader::Occupied {
                id,
                layout,
                instances_count,
                padding,
                ..
            } = header
                && object_id == *id
                && *instances_count > 0
            {
                return Some((*layout, header_size + *padding as usize));
            }
        }
        None
    }

    fn object_instances_count(&self, pointer: *mut u8, object_id: usize, page_id: usize) -> usize {
        if let Some(page) = self.pages.get(&page_id)
            && page.owns_pointer(pointer)
        {
            let header = unsafe { pointer.cast::<ManagedObjectHeader>().as_mut().unwrap() };
            if let ManagedObjectHeader::Occupied {
                id,
                instances_count,
                ..
            } = header
                && object_id == *id
            {
                return *instances_count;
            }
        }
        0
    }
}

pub struct ManagedBox<T> {
    memory: *mut T,
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
        let mut result = DynamicManagedBox::new(value);
        result.drop = false;
        Self {
            memory: result.memory.cast(),
            id: result.id,
            page: result.page,
            drop: true,
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

    pub fn does_share_reference(&self, other: &Self) -> bool {
        self.id == other.id && self.page == other.page && self.memory == other.memory
    }

    pub fn type_hash(&self) -> Option<TypeHash> {
        STORAGE
            .with_borrow(|storage| storage.object_type_hash(self.memory.cast(), self.id, self.page))
    }

    pub fn lifetime_borrow(&self) -> Option<LifetimeRef> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { lifetime.as_ref()?.borrow() }
        })
    }

    pub fn lifetime_borrow_mut(&self) -> Option<LifetimeRefMut> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { lifetime.as_ref()?.borrow_mut() }
        })
    }

    pub fn lifetime_lazy(&self) -> Option<LifetimeLazy> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { Some(lifetime.as_ref()?.lazy()) }
        })
    }

    pub fn read(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            unsafe { lifetime.as_ref()?.read_ptr(pointer) }
        })
    }

    pub async fn read_async(&'_ self) -> ValueReadAccess<'_, T> {
        loop {
            if let Some(access) = self.read() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<T>>::Pending
            })
            .await;
        }
    }

    pub fn write(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            unsafe { lifetime.as_mut()?.write_ptr(pointer) }
        })
    }

    pub async fn write_async(&'_ mut self) -> ValueWriteAccess<'_, T> {
        loop {
            let result = STORAGE.with_borrow(|storage| {
                let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                    self.memory.cast(),
                    self.id,
                    self.page,
                    true,
                )?;
                unsafe { lifetime.as_mut()?.write_ptr(pointer) }
            });
            if let Some(access) = result {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueWriteAccess<T>>::Pending
            })
            .await;
        }
    }

    pub fn borrow(&self) -> Option<ManagedRef<T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            unsafe { ManagedRef::new_raw(pointer, lifetime.as_ref()?.borrow()?) }
        })
    }

    pub async fn borrow_async(&self) -> ManagedRef<T> {
        loop {
            if let Some(access) = self.borrow() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ManagedRef<T>>::Pending
            })
            .await;
        }
    }

    pub fn borrow_mut(&mut self) -> Option<ManagedRefMut<T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            unsafe { ManagedRefMut::new_raw(pointer, lifetime.as_mut()?.borrow_mut()?) }
        })
    }

    pub async fn borrow_mut_async(&mut self) -> ManagedRefMut<T> {
        loop {
            if let Some(access) = self.borrow_mut() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ManagedRefMut<T>>::Pending
            })
            .await;
        }
    }

    pub fn lazy(&self) -> Option<ManagedLazy<T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            unsafe { ManagedLazy::new_raw(pointer, lifetime.as_mut().unwrap().lazy()) }
        })
    }

    /// # Safety
    pub unsafe fn as_ptr(&self) -> Option<*const T> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            Some(pointer.cast_const())
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_mut(&mut self) -> Option<*mut T> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                true,
            )?;
            Some(pointer)
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            Some(pointer.cast_const().cast())
        })
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> Option<*mut u8> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<T>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            Some(pointer.cast())
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
    memory: *mut u8,
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
            let mut result =
                Self::new_uninitialized(TypeHash::of::<T>(), Layout::new::<T>(), T::finalize_raw);
            result.as_ptr_mut::<T>().unwrap().write(value);
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

    pub fn does_share_reference(&self, other: &Self) -> bool {
        self.id == other.id && self.page == other.page && self.memory == other.memory
    }

    pub fn type_hash(&self) -> Option<TypeHash> {
        STORAGE.with_borrow(|storage| storage.object_type_hash(self.memory, self.id, self.page))
    }

    pub fn lifetime_borrow(&self) -> Option<LifetimeRef> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { lifetime.as_ref()?.borrow() }
        })
    }

    pub fn lifetime_borrow_mut(&self) -> Option<LifetimeRefMut> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { lifetime.as_ref()?.borrow_mut() }
        })
    }

    pub fn lifetime_lazy(&self) -> Option<LifetimeLazy> {
        STORAGE.with_borrow(|storage| {
            let (_, lifetime, _) = storage.access_object_lifetime_type::<u8>(
                self.memory.cast(),
                self.id,
                self.page,
                false,
            )?;
            unsafe { Some(lifetime.as_ref()?.lazy()) }
        })
    }

    pub fn is<T>(&self) -> bool {
        STORAGE.with_borrow(|storage| {
            storage
                .access_object_lifetime_type::<T>(self.memory, self.id, self.page, true)
                .is_some()
        })
    }

    pub fn borrow(&self) -> Option<DynamicManagedRef> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, type_hash) = storage.access_object_lifetime_type::<u8>(
                self.memory,
                self.id,
                self.page,
                false,
            )?;
            unsafe { DynamicManagedRef::new_raw(type_hash, lifetime.as_ref()?.borrow()?, pointer) }
        })
    }

    pub async fn borrow_async(&self) -> DynamicManagedRef {
        loop {
            if let Some(access) = self.borrow() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<DynamicManagedRef>::Pending
            })
            .await;
        }
    }

    pub fn borrow_mut(&mut self) -> Option<DynamicManagedRefMut> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, type_hash) = storage.access_object_lifetime_type::<u8>(
                self.memory,
                self.id,
                self.page,
                false,
            )?;
            unsafe {
                DynamicManagedRefMut::new_raw(type_hash, lifetime.as_mut()?.borrow_mut()?, pointer)
            }
        })
    }

    pub async fn borrow_mut_async(&mut self) -> DynamicManagedRefMut {
        loop {
            if let Some(access) = self.borrow_mut() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<DynamicManagedRefMut>::Pending
            })
            .await;
        }
    }

    pub fn lazy(&self) -> Option<DynamicManagedLazy> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, type_hash) = storage.access_object_lifetime_type::<u8>(
                self.memory,
                self.id,
                self.page,
                false,
            )?;
            unsafe {
                DynamicManagedLazy::new_raw(type_hash, lifetime.as_mut().unwrap().lazy(), pointer)
            }
        })
    }

    pub fn read<T>(&'_ self) -> Option<ValueReadAccess<'_, T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) =
                storage.access_object_lifetime_type::<T>(self.memory, self.id, self.page, true)?;
            unsafe { lifetime.as_ref()?.read_ptr(pointer) }
        })
    }

    pub async fn read_async<'a, T: 'a>(&'a self) -> ValueReadAccess<'a, T> {
        loop {
            if let Some(access) = self.read() {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueReadAccess<T>>::Pending
            })
            .await;
        }
    }

    pub fn write<T>(&'_ mut self) -> Option<ValueWriteAccess<'_, T>> {
        STORAGE.with_borrow(|storage| {
            let (pointer, lifetime, _) =
                storage.access_object_lifetime_type::<T>(self.memory, self.id, self.page, true)?;
            unsafe { lifetime.as_mut()?.write_ptr(pointer) }
        })
    }

    pub async fn write_async<'a, T: 'a>(&'a mut self) -> ValueWriteAccess<'a, T> {
        loop {
            let result = STORAGE.with_borrow(|storage| {
                let (pointer, lifetime, _) = storage.access_object_lifetime_type::<T>(
                    self.memory,
                    self.id,
                    self.page,
                    true,
                )?;
                unsafe { lifetime.as_mut()?.write_ptr(pointer) }
            });
            if let Some(access) = result {
                return access;
            }
            poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::<ValueWriteAccess<T>>::Pending
            })
            .await;
        }
    }

    /// # Safety
    pub unsafe fn memory(&self) -> Option<&[u8]> {
        STORAGE.with_borrow(|storage| {
            storage
                .object_layout_with_offset(self.memory, self.id, self.page)
                .map(|(layout, offset)| unsafe {
                    std::slice::from_raw_parts(self.memory.add(offset), layout.size())
                })
        })
    }

    /// # Safety
    pub unsafe fn memory_mut(&mut self) -> Option<&mut [u8]> {
        STORAGE.with_borrow(|storage| {
            storage
                .object_layout_with_offset(self.memory, self.id, self.page)
                .map(|(layout, offset)| unsafe {
                    std::slice::from_raw_parts_mut(self.memory.add(offset), layout.size())
                })
        })
    }

    /// # Safety
    pub unsafe fn as_ptr<T>(&self) -> Option<*const T> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) =
                storage.access_object_lifetime_type::<T>(self.memory, self.id, self.page, true)?;
            Some(pointer.cast_const().cast())
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_mut<T>(&mut self) -> Option<*mut T> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) =
                storage.access_object_lifetime_type::<T>(self.memory, self.id, self.page, true)?;
            Some(pointer.cast())
        })
    }

    /// # Safety
    pub unsafe fn as_ptr_raw(&self) -> Option<*const u8> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<u8>(
                self.memory,
                self.id,
                self.page,
                false,
            )?;
            Some(pointer.cast_const())
        })
    }

    /// # Safety
    pub unsafe fn as_mut_ptr_raw(&mut self) -> Option<*mut u8> {
        STORAGE.with_borrow(|storage| {
            let (pointer, _, _) = storage.access_object_lifetime_type::<u8>(
                self.memory,
                self.id,
                self.page,
                false,
            )?;
            Some(pointer)
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
        let v = OccupancyRange {
            bits_start_inclusive: 0,
            bits_end_exclusive: 128,
            ..Default::default()
        }
        .update_mask();
        assert_eq!(v.mask, 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF);
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 0..128);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE);

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

        let v = v.split().unwrap().1;
        assert_eq!(
            v.mask,
            0b0000000000000000000011000000000000000000000000000000000000000000
        );
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 42..44);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 64);

        let v = v.split().unwrap().0;
        assert_eq!(
            v.mask,
            0b0000000000000000000001000000000000000000000000000000000000000000
        );
        assert_eq!(v.bits_start_inclusive..v.bits_end_exclusive, 42..43);
        assert_eq!(v.byte_size(), MEMORY_PAGE_SIZE / 128);

        assert!(v.split().is_none());
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 16392,
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
                total_size: 96528,
                occupied_size: 89608,
                free_size: 6912
            }
        );
        drop(a);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 2,
                chunked_pages_count: 1,
                exclusive_pages_count: 1,
                total_size: 96528,
                occupied_size: 89352,
                free_size: 7168
            }
        );
        drop(b);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 2,
                chunked_pages_count: 1,
                exclusive_pages_count: 1,
                total_size: 96528,
                occupied_size: 88328,
                free_size: 8192
            }
        );
        drop(c);
        assert_eq!(
            managed_storage_stats(),
            ManagedStorageStats {
                pages_count: 1,
                chunked_pages_count: 0,
                exclusive_pages_count: 1,
                total_size: 80136,
                occupied_size: 80136,
                free_size: 0
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

    #[test]
    fn test_fuzz_managed_box() {
        let builders = [
            || DynamicManagedBox::new(1u8),
            || DynamicManagedBox::new(2u16),
            || DynamicManagedBox::new(3u32),
            || DynamicManagedBox::new(4u64),
            || DynamicManagedBox::new(5u128),
            || DynamicManagedBox::new([42u8; 1000]),
            || DynamicManagedBox::new([42u8; 10000]),
            || DynamicManagedBox::new([42u8; 100000]),
        ];
        let mut boxes = std::array::from_fn::<_, 50, _>(|_| None);
        for index in 0..100 {
            let source = index % builders.len();
            let target = index % boxes.len();
            boxes[target] = Some((builders[source])());
        }
    }
}
