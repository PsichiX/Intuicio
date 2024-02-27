use crate::{lifetime::Lifetime, managed::ManagedLazy, Finalize};
use std::alloc::Layout;

const MINIMUM_SIZE: usize = 1024;

struct ManagedArenaObjectHeader {
    layout: Layout,
    finalizer: unsafe fn(*mut ()),
    #[allow(dead_code)]
    lifetime: Lifetime,
}

pub struct ManagedArenaPage {
    memory: Vec<u8>,
    position: usize,
}

impl Drop for ManagedArenaPage {
    fn drop(&mut self) {
        self.clear();
    }
}

impl ManagedArenaPage {
    pub fn new(mut capacity: usize) -> Self {
        capacity = capacity.max(MINIMUM_SIZE).next_power_of_two();
        let memory = vec![0; capacity];
        Self {
            memory,
            position: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.memory.len()
    }

    pub fn size(&self) -> usize {
        self.position
    }

    pub fn available_size(&self) -> usize {
        self.capacity() - self.size()
    }

    pub fn alloc<T: Finalize>(&mut self, value: T) -> Option<ManagedLazy<T>> {
        unsafe {
            let result = self.alloc_uninitialized::<T>()?;
            result.as_mut_ptr().unwrap().write(value);
            Some(result)
        }
    }

    /// # Safety
    pub unsafe fn alloc_uninitialized<T: Finalize>(&mut self) -> Option<ManagedLazy<T>> {
        let header_layout = Layout::new::<ManagedArenaObjectHeader>().pad_to_align();
        let layout = Layout::new::<T>().pad_to_align();
        if header_layout.size() + layout.size() > self.available_size() {
            return None;
        }
        let lifetime = Lifetime::default();
        let lifetime_lazy = lifetime.lazy();
        let header = ManagedArenaObjectHeader {
            layout,
            finalizer: T::finalize_raw,
            lifetime,
        };
        self.memory
            .as_mut_ptr()
            .add(self.position)
            .cast::<ManagedArenaObjectHeader>()
            .write(header);
        self.position += header_layout.size();
        let data = self.memory.as_mut_ptr().add(self.position).cast::<T>();
        self.position += layout.size();
        Some(ManagedLazy::new_raw(data, lifetime_lazy))
    }

    pub fn clear(&mut self) {
        let header_layout = Layout::new::<ManagedArenaObjectHeader>().pad_to_align();
        let mut position = 0;
        while position < self.position {
            unsafe {
                let header = self
                    .memory
                    .as_ptr()
                    .add(position)
                    .cast::<ManagedArenaObjectHeader>()
                    .read();
                position += header_layout.size();
                let data = self.memory.as_mut_ptr().add(position).cast::<()>();
                (header.finalizer)(data);
                position += header.layout.size();
            }
        }
        self.position = 0;
    }
}

pub struct ManagedArena {
    pages: Vec<ManagedArenaPage>,
    page_capacity: usize,
}

impl Default for ManagedArena {
    fn default() -> Self {
        Self {
            pages: Default::default(),
            page_capacity: MINIMUM_SIZE,
        }
    }
}

impl ManagedArena {
    pub fn new(page_capacity: usize) -> Self {
        Self {
            pages: Default::default(),
            page_capacity: page_capacity.max(MINIMUM_SIZE),
        }
    }

    pub fn page_capacity(&self) -> usize {
        self.page_capacity
    }

    pub fn pages_count(&self) -> usize {
        self.pages.len()
    }

    pub fn capacity(&self) -> usize {
        self.pages.iter().map(|page| page.capacity()).sum()
    }

    pub fn size(&self) -> usize {
        self.pages.iter().map(|page| page.size()).sum()
    }

    pub fn available_size(&self) -> usize {
        self.pages.iter().map(|page| page.available_size()).sum()
    }

    pub fn alloc<T: Finalize>(&mut self, value: T) -> ManagedLazy<T> {
        unsafe {
            let result = self.alloc_uninitialized::<T>();
            result.as_mut_ptr().unwrap().write(value);
            result
        }
    }

    /// # Safety
    pub unsafe fn alloc_uninitialized<T: Finalize>(&mut self) -> ManagedLazy<T> {
        for page in self.pages.iter_mut() {
            if let Some(result) = page.alloc_uninitialized::<T>() {
                return result;
            }
        }
        let header_size = Layout::new::<ManagedArenaObjectHeader>()
            .pad_to_align()
            .size();
        let size = Layout::new::<T>().pad_to_align().size();
        let mut page = ManagedArenaPage::new(self.page_capacity.max(header_size + size));
        let result = page.alloc_uninitialized::<T>().unwrap();
        self.pages.push(page);
        result
    }

    pub fn clear(&mut self) {
        self.pages.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::{managed::ManagedLazy, managed_arena::ManagedArena};
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_managed_arena() {
        #[derive(Default)]
        struct Droppable(Arc<RwLock<bool>>);

        impl Drop for Droppable {
            fn drop(&mut self) {
                *self
                    .0
                    .write()
                    .expect("Could not get write access to arena memory page!") = true;
            }
        }

        struct Nested(ManagedLazy<u32>);

        let droppable = Droppable::default();
        let droppable_inner = droppable.0.clone();
        let mut arena = ManagedArena::new(4);
        assert_eq!(arena.page_capacity(), 1024);
        assert_eq!(arena.pages_count(), 0);
        assert_eq!(arena.capacity(), 0);
        assert_eq!(arena.size(), 0);
        assert_eq!(arena.available_size(), 0);
        let object = arena.alloc(42u32);
        assert_eq!(arena.pages_count(), 1);
        assert_eq!(arena.capacity(), 1024);
        assert_eq!(arena.size(), 68);
        assert_eq!(arena.available_size(), 956);
        assert_eq!(*object.read().unwrap(), 42);
        *object.write().unwrap() = 100;
        assert_eq!(*object.read().unwrap(), 100);
        let _ = arena.alloc([42u8; 2048]);
        assert_eq!(arena.pages_count(), 2);
        assert_eq!(arena.capacity(), 5120);
        assert_eq!(arena.size(), 2180);
        assert_eq!(arena.available_size(), 2940);
        let _ = arena.alloc(droppable);
        assert_eq!(arena.pages_count(), 2);
        assert_eq!(arena.capacity(), 5120);
        assert_eq!(arena.size(), 2252);
        assert_eq!(arena.available_size(), 2868);
        assert!(!*droppable_inner.read().unwrap());
        let nested = arena.alloc(Nested(object.clone()));
        assert_eq!(arena.pages_count(), 2);
        assert_eq!(arena.capacity(), 5120);
        assert_eq!(arena.size(), 2364);
        assert_eq!(arena.available_size(), 2756);
        assert_eq!(*nested.read().unwrap().0.read().unwrap(), 100);
        assert_eq!(*object.read().unwrap(), 100);
        *nested.write().unwrap().0.write().unwrap() = 42;
        assert_eq!(*nested.read().unwrap().0.read().unwrap(), 42);
        assert_eq!(*object.read().unwrap(), 42);
        arena.clear();
        assert!(*droppable_inner.read().unwrap());
        assert_eq!(arena.pages_count(), 0);
        assert_eq!(arena.capacity(), 0);
        assert_eq!(arena.size(), 0);
        assert_eq!(arena.available_size(), 0);
        assert!(object.read().is_none());
    }
}
