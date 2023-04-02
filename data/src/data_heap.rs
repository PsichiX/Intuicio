use crate::Finalize;
use std::{
    alloc::Layout,
    cell::RefCell,
    ptr::NonNull,
    rc::{Rc, Weak},
};
use typid::ID;

pub type DataHeapMemoryPageHandle = Rc<RefCell<DataHeapMemoryPage>>;
pub type DataHeapObjectID = ID<DataHeapMemoryPage>;

const MINIMUM_SIZE: usize = 1024;

#[derive(Copy, Clone)]
enum DataHeapObjectHeader {
    Occupied {
        id: DataHeapObjectID,
        layout: Layout,
        finalizer: unsafe fn(*mut ()),
        tail_size: usize,
    },
    Free {
        size: usize,
    },
}

#[derive(Copy, Clone)]
struct DataHeapObjectFooter {
    size: usize,
}

pub struct DataHeapMemoryPage {
    memory: Vec<u8>,
    size: usize,
    position: usize,
}

impl Drop for DataHeapMemoryPage {
    fn drop(&mut self) {
        unsafe {
            let header_layout = Layout::new::<DataHeapObjectHeader>().pad_to_align();
            let footer_layout = Layout::new::<DataHeapObjectFooter>().pad_to_align();
            self.position = 0;
            while self.position < self.capacity() {
                match self
                    .memory
                    .as_mut_ptr()
                    .add(self.position)
                    .cast::<DataHeapObjectHeader>()
                    .read()
                {
                    DataHeapObjectHeader::Occupied {
                        layout,
                        finalizer,
                        tail_size,
                        ..
                    } => {
                        let data_pointer = self
                            .memory
                            .as_mut_ptr()
                            .add(self.position + header_layout.size())
                            .cast::<()>();
                        (finalizer)(data_pointer);
                        self.position +=
                            header_layout.size() + layout.size() + tail_size + footer_layout.size();
                    }
                    DataHeapObjectHeader::Free { size } => {
                        self.position += header_layout.size() + size + footer_layout.size();
                    }
                }
            }
        }
    }
}

impl DataHeapMemoryPage {
    pub fn new(mut capacity: usize) -> Self {
        capacity = capacity.max(MINIMUM_SIZE).next_power_of_two();
        let mut memory = vec![0; capacity];
        let header_layout = Layout::new::<DataHeapObjectHeader>().pad_to_align();
        let footer_layout = Layout::new::<DataHeapObjectFooter>().pad_to_align();
        unsafe {
            let header_pointer = memory.as_mut_ptr().cast::<DataHeapObjectHeader>();
            let footer_pointer = memory
                .as_mut_ptr()
                .add(capacity)
                .cast::<DataHeapObjectFooter>();
            let size = capacity - header_layout.size() - footer_layout.size();
            header_pointer.write(DataHeapObjectHeader::Free { size });
            footer_pointer.write(DataHeapObjectFooter { size });
        }
        Self {
            memory,
            size: 0,
            position: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.memory.len()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn available_size(&self) -> usize {
        self.capacity() - self.size()
    }

    pub fn has_object<T>(&self, data: &DataHeapBox<T>) -> bool {
        unsafe {
            let header_layout = Layout::new::<DataHeapObjectHeader>();
            let pointer = data.data.as_ptr().cast::<u8>().cast_const();
            if pointer < self.memory.as_ptr()
                || pointer >= self.memory.as_ptr().add(self.capacity())
            {
                return false;
            }
            match pointer
                .sub(header_layout.size())
                .cast::<DataHeapObjectHeader>()
                .read()
            {
                DataHeapObjectHeader::Occupied { id, .. } => data.id == id,
                DataHeapObjectHeader::Free { .. } => false,
            }
        }
    }

    pub fn alloc<T: Finalize>(page: &DataHeapMemoryPageHandle, value: T) -> Option<DataHeapBox<T>> {
        unsafe {
            let result = Self::alloc_uninitialized(page)?;
            let pointer: *mut T = result.data.as_ptr();
            pointer.write(value);
            Some(result)
        }
    }

    /// # Safety
    pub unsafe fn alloc_uninitialized<T: Finalize>(
        page: &DataHeapMemoryPageHandle,
    ) -> Option<DataHeapBox<T>> {
        unsafe {
            let page_weak = DataHeapMemoryPageHandle::downgrade(page);
            let mut this = page.borrow_mut();
            let header_layout = Layout::new::<DataHeapObjectHeader>().pad_to_align();
            let footer_layout = Layout::new::<DataHeapObjectFooter>().pad_to_align();
            let layout = Layout::new::<T>().pad_to_align();
            if header_layout.size() + layout.size() + footer_layout.size() > this.available_size() {
                return None;
            }
            let mut accumulator = 0;
            loop {
                if accumulator >= this.capacity() {
                    return None;
                }
                let header_pointer = this
                    .memory
                    .as_mut_ptr()
                    .add(this.position)
                    .cast::<DataHeapObjectHeader>();
                let offset = match header_pointer.read_unaligned() {
                    DataHeapObjectHeader::Occupied {
                        layout, tail_size, ..
                    } => header_layout.size() + layout.size() + tail_size + footer_layout.size(),
                    DataHeapObjectHeader::Free { size } => {
                        if size >= layout.size() {
                            break;
                        }
                        header_layout.size() + size + footer_layout.size()
                    }
                };
                if this.position + offset >= this.capacity() {
                    accumulator += this.capacity() - this.position;
                    this.position = 0;
                } else {
                    this.position += offset;
                    accumulator += offset;
                }
            }
            let header_pointer = this
                .memory
                .as_mut_ptr()
                .add(this.position)
                .cast::<DataHeapObjectHeader>();
            let size = if let DataHeapObjectHeader::Free { size } = header_pointer.read_unaligned()
            {
                size
            } else {
                return None;
            };
            let id = DataHeapObjectID::new();
            let data_pointer = this
                .memory
                .as_mut_ptr()
                .add(this.position + header_layout.size());
            let mut tail_size = size - layout.size() - footer_layout.size();
            if tail_size > header_layout.size() + footer_layout.size() {
                let new_size = tail_size - header_layout.size();
                let extra_header_pointer = this.memory.as_mut_ptr().add(
                    this.position + header_layout.size() + layout.size() + footer_layout.size(),
                );
                let extra_footer_pointer = extra_header_pointer.add(new_size);
                extra_header_pointer
                    .cast::<DataHeapObjectHeader>()
                    .write(DataHeapObjectHeader::Free { size: new_size });
                extra_footer_pointer
                    .cast::<DataHeapObjectFooter>()
                    .write(DataHeapObjectFooter { size: new_size });
                tail_size = 0;
            }
            header_pointer.write(DataHeapObjectHeader::Occupied {
                id,
                layout,
                finalizer: T::finalize_raw,
                tail_size,
            });
            let size = layout.size() + tail_size;
            let footer_pointer = this
                .memory
                .as_mut_ptr()
                .add(this.position + header_layout.size() + size)
                .cast::<DataHeapObjectFooter>();
            footer_pointer.write(DataHeapObjectFooter { size });
            this.size += header_layout.size() + size + footer_layout.size();
            let data_pointer = data_pointer.cast::<T>();
            Some(DataHeapBox {
                id,
                data: NonNull::new_unchecked(data_pointer),
                page: page_weak,
            })
        }
    }

    fn dealloc(page: &DataHeapMemoryPageHandle, data_pointer: *mut (), id: DataHeapObjectID) {
        unsafe {
            let finalizer = Self::leak(page, data_pointer, id);
            (finalizer)(data_pointer)
        };
    }

    /// # Safety
    #[must_use]
    pub unsafe fn leak(
        page: &DataHeapMemoryPageHandle,
        data_pointer: *mut (),
        id: DataHeapObjectID,
    ) -> unsafe fn(*mut ()) {
        let mut this = page.borrow_mut();
        let header_layout = Layout::new::<DataHeapObjectHeader>().pad_to_align();
        let mut header_pointer = data_pointer.cast::<u8>().sub(header_layout.size());
        let (other_id, layout, finalizer, tail_size) =
            match header_pointer.cast::<DataHeapObjectHeader>().read() {
                DataHeapObjectHeader::Occupied {
                    id,
                    layout,
                    finalizer,
                    tail_size,
                } => (id, layout, finalizer, tail_size),
                DataHeapObjectHeader::Free { .. } => panic!("Trying to deallocate empty space!"),
            };
        if id != other_id {
            panic!("Trying to deallocate wrong object!");
        }
        let footer_layout = Layout::new::<DataHeapObjectFooter>().pad_to_align();
        let mut footer_pointer =
            header_pointer.add(header_layout.size() + layout.size() + tail_size);
        let extra_header_pointer = footer_pointer.add(footer_layout.size());
        if extra_header_pointer < this.memory.as_mut_ptr().add(this.capacity()) {
            if let DataHeapObjectHeader::Free { size } =
                extra_header_pointer.cast::<DataHeapObjectHeader>().read()
            {
                footer_pointer = extra_header_pointer.add(header_layout.size() + size);
            }
        }
        if header_pointer > this.memory.as_mut_ptr() {
            let extra_footer_pointer = header_pointer.sub(footer_layout.size());
            let prev_size = extra_footer_pointer
                .cast::<DataHeapObjectFooter>()
                .read()
                .size;
            let extra_header_pointer = extra_footer_pointer.sub(prev_size + header_layout.size());
            if let DataHeapObjectHeader::Free { .. } =
                extra_header_pointer.cast::<DataHeapObjectHeader>().read()
            {
                header_pointer = extra_header_pointer;
            }
        }
        let size = footer_pointer as usize - header_pointer as usize - header_layout.size();
        this.position = header_pointer as usize - this.memory.as_mut_ptr() as usize;
        header_pointer
            .cast::<DataHeapObjectHeader>()
            .write(DataHeapObjectHeader::Free { size });
        footer_pointer
            .cast::<DataHeapObjectFooter>()
            .write(DataHeapObjectFooter { size });
        this.size -= header_layout.size() + layout.size() + tail_size + footer_layout.size();
        finalizer
    }

    pub fn stats(&self) -> DataHeapStatsPage {
        let header_layout = Layout::new::<DataHeapObjectHeader>().pad_to_align();
        let footer_layout = Layout::new::<DataHeapObjectFooter>().pad_to_align();
        let mut total_allocated_size = 0;
        let mut total_free_size = 0;
        let mut allocated_size = 0;
        let mut free_size = 0;
        let mut occupied_fragments = 0;
        let mut free_fragments = 0;
        let mut fragments = vec![];
        let mut position = 0;
        while position < self.capacity() {
            unsafe {
                let header = self
                    .memory
                    .as_ptr()
                    .add(position)
                    .cast::<DataHeapObjectHeader>()
                    .read();
                match header {
                    DataHeapObjectHeader::Occupied {
                        id,
                        layout,
                        tail_size,
                        ..
                    } => {
                        occupied_fragments += 1;
                        fragments.push(DataHeapStatsFragment::OccupiedHeader {
                            start_position: position,
                            end_position: position + header_layout.size(),
                            size: header_layout.size(),
                            id,
                            layout,
                            tail_size,
                        });
                        position += header_layout.size();
                        let size = layout.size() + tail_size;
                        fragments.push(DataHeapStatsFragment::OccupiedSpace {
                            start_position: position,
                            end_position: position + size,
                            size,
                        });
                        position += size;
                        allocated_size += layout.size() + tail_size;
                        total_allocated_size +=
                            header_layout.size() + layout.size() + tail_size + footer_layout.size();
                    }
                    DataHeapObjectHeader::Free { size } => {
                        free_fragments += 1;
                        fragments.push(DataHeapStatsFragment::FreeHeader {
                            start_position: position,
                            end_position: position + header_layout.size(),
                            size: header_layout.size(),
                        });
                        position += header_layout.size();
                        fragments.push(DataHeapStatsFragment::FreeSpace {
                            start_position: position,
                            end_position: position + size,
                            size,
                        });
                        position += size;
                        free_size += size;
                        total_free_size += header_layout.size() + size + footer_layout.size();
                    }
                }
                let footer = self
                    .memory
                    .as_ptr()
                    .add(position)
                    .cast::<DataHeapObjectFooter>()
                    .read();
                fragments.push(DataHeapStatsFragment::Footer {
                    start_position: position,
                    end_position: position + footer_layout.size(),
                    size: footer_layout.size(),
                    item_size: footer.size,
                });
                position += footer_layout.size();
            }
        }
        DataHeapStatsPage {
            page_capacity: self.capacity(),
            position: self.position,
            total_allocated_size,
            total_free_size,
            allocated_size,
            free_size,
            occupied_fragments,
            free_fragments,
            fragments,
        }
    }
}

pub struct DataHeap {
    pages: Vec<Rc<RefCell<DataHeapMemoryPage>>>,
    page_capacity: usize,
    pub pages_count_limit: Option<usize>,
    position: usize,
}

impl DataHeap {
    pub fn new(page_capacity: usize) -> Self {
        Self {
            pages: Vec::with_capacity(1),
            page_capacity: page_capacity.max(MINIMUM_SIZE).next_power_of_two(),
            pages_count_limit: None,
            position: 0,
        }
    }

    pub fn page_capacity(&self) -> usize {
        self.page_capacity
    }

    pub fn pages_count(&self) -> usize {
        self.pages.len()
    }

    pub fn capacity(&self) -> usize {
        self.pages.iter().map(|page| page.borrow().capacity()).sum()
    }

    pub fn size(&self) -> usize {
        self.pages.iter().map(|page| page.borrow().size()).sum()
    }

    pub fn available_size(&self) -> usize {
        self.pages
            .iter()
            .map(|page| page.borrow().available_size())
            .sum()
    }

    pub fn has_object<T>(&self, data: &DataHeapBox<T>) -> bool {
        self.pages.iter().any(|page| page.borrow().has_object(data))
    }

    pub fn ensure_pages(&mut self, total_pages_count: usize) {
        let additional = total_pages_count.saturating_sub(self.pages.len());
        let capacity = (self.pages.len() + additional).next_power_of_two() - self.pages.len();
        self.pages.reserve(capacity);
        for _ in 0..additional {
            let page = Rc::new(RefCell::new(DataHeapMemoryPage::new(self.page_capacity)));
            self.pages.push(page);
        }
    }

    pub fn collect_empty_pages(&mut self) {
        let mut index = 0;
        while index < self.pages.len() {
            if self.pages[index].borrow().size() == 0 {
                self.pages.swap_remove(index);
            } else {
                index += 1;
            }
        }
        self.position = 0;
    }

    pub fn ensure_capacity(&mut self, total_capacity: usize) {
        let count = (total_capacity / self.page_capacity) + 1;
        self.ensure_pages(count);
    }

    pub fn alloc<T>(&mut self, value: T) -> Option<DataHeapBox<T>> {
        let layout = Layout::new::<T>().pad_to_align();
        let mut accumulator = 0;
        while accumulator < self.pages.len() {
            self.position %= self.pages.len();
            let page = &self.pages[self.position];
            if layout.size() <= page.borrow().available_size() {
                return DataHeapMemoryPage::alloc(page, value);
            }
            self.position += 1;
            accumulator += 1;
        }
        if let Some(pages_count_limit) = self.pages_count_limit {
            if self.pages.len() >= pages_count_limit {
                return None;
            }
        }
        let capacity = self.page_capacity.max(layout.size());
        let page = Rc::new(RefCell::new(DataHeapMemoryPage::new(capacity)));
        let result = DataHeapMemoryPage::alloc(&page, value)?;
        let capacity = (self.pages.len() + 1).next_power_of_two() - self.pages.len();
        self.position = self.pages.len();
        self.pages.reserve(capacity);
        self.pages.push(page);
        Some(result)
    }

    pub fn stats(&self) -> DataHeapStats {
        DataHeapStats {
            position: self.position,
            pages: self
                .pages
                .iter()
                .map(|page| page.borrow().stats())
                .collect(),
        }
    }
}

pub struct DataHeapBox<T> {
    page: Weak<RefCell<DataHeapMemoryPage>>,
    data: NonNull<T>,
    id: DataHeapObjectID,
}

impl<T> Drop for DataHeapBox<T> {
    fn drop(&mut self) {
        if let Some(page) = self.page.upgrade() {
            DataHeapMemoryPage::dealloc(&page, self.data.as_ptr().cast::<()>(), self.id);
        }
    }
}

impl<T> DataHeapBox<T> {
    pub fn id(&self) -> DataHeapObjectID {
        self.id
    }

    pub fn exists(&self) -> bool {
        self.page
            .upgrade()
            .map(|page| page.borrow().has_object(self))
            .unwrap_or(false)
    }

    pub fn read(&self) -> Option<&T> {
        if self.exists() {
            Some(unsafe { self.data.as_ref() })
        } else {
            None
        }
    }

    pub fn write(&mut self) -> Option<&mut T> {
        if self.exists() {
            Some(unsafe { self.data.as_mut() })
        } else {
            None
        }
    }

    pub fn consume(self) -> Result<T, Self> {
        if let Some(page) = self.page.upgrade() {
            unsafe {
                let result = self.data.as_ptr().read();
                let _ = DataHeapMemoryPage::leak(&page, self.data.as_ptr().cast::<()>(), self.id);
                Ok(result)
            }
        } else {
            Err(self)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataHeapStatsFragment {
    OccupiedHeader {
        start_position: usize,
        end_position: usize,
        size: usize,
        id: DataHeapObjectID,
        layout: Layout,
        tail_size: usize,
    },
    FreeHeader {
        start_position: usize,
        end_position: usize,
        size: usize,
    },
    Footer {
        start_position: usize,
        end_position: usize,
        size: usize,
        item_size: usize,
    },
    OccupiedSpace {
        start_position: usize,
        end_position: usize,
        size: usize,
    },
    FreeSpace {
        start_position: usize,
        end_position: usize,
        size: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataHeapStatsPage {
    pub page_capacity: usize,
    pub position: usize,
    pub total_allocated_size: usize,
    pub total_free_size: usize,
    pub allocated_size: usize,
    pub free_size: usize,
    pub occupied_fragments: usize,
    pub free_fragments: usize,
    pub fragments: Vec<DataHeapStatsFragment>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DataHeapStats {
    pub position: usize,
    pub pages: Vec<DataHeapStatsPage>,
}

#[cfg(test)]
mod tests {
    use crate::data_heap::{DataHeap, DataHeapBox, DataHeapStats};
    use std::{cell::RefCell, io::Write, rc::Rc};

    static ANALIZE_DIAGNOSTICS: bool = false;

    fn analize_stats(heap: &DataHeap) -> DataHeapStats {
        if !ANALIZE_DIAGNOSTICS {
            return Default::default();
        }
        static mut ITERATION: usize = 0;
        static mut SKIP_TO: usize = 0;
        static mut PRINT: bool = true;
        let stats = heap.stats();
        unsafe {
            ITERATION += 1;
            if PRINT {
                println!("{} | Diagnostics: {:#?}", ITERATION, stats);
            }
            let command = if ITERATION >= SKIP_TO {
                print!("{} | Diagnostics command: ", ITERATION);
                let _ = std::io::stdout().flush();
                let mut command = String::new();
                let _ = std::io::stdin().read_line(&mut command);
                command
            } else {
                String::new()
            };
            let parts = command
                .as_str()
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            if let Some(id) = parts.get(0) {
                match *id {
                    "exit" => {
                        std::process::exit(0);
                    }
                    "show" => {
                        PRINT = true;
                    }
                    "hide" => {
                        PRINT = false;
                    }
                    "skip" => {
                        if let Some(count) = parts.get(1) {
                            if let Ok(count) = count.parse::<usize>() {
                                SKIP_TO = ITERATION + count;
                            }
                        }
                    }
                    "skip-to" => {
                        if let Some(iteration) = parts.get(1) {
                            if let Ok(iteration) = iteration.parse::<usize>() {
                                SKIP_TO = iteration;
                            }
                        }
                    }
                    "skip-to-end" => {
                        SKIP_TO = usize::MAX;
                    }
                    _ => {}
                }
            }
            stats
        }
    }

    #[test]
    fn test_data_heap() {
        struct Droppable(Rc<RefCell<bool>>);

        impl Drop for Droppable {
            fn drop(&mut self) {
                *self.0.borrow_mut() = true;
            }
        }

        struct Nested(DataHeapBox<usize>);

        let mut heap = DataHeap::new(4);
        analize_stats(&heap);
        assert_eq!(heap.page_capacity(), 1024);
        assert_eq!(heap.pages_count(), 0);
        assert_eq!(heap.capacity(), 0);
        assert_eq!(heap.size(), 0);
        assert_eq!(heap.available_size(), 0);
        let mut object = heap.alloc(42u32).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 60);
        assert_eq!(heap.available_size(), 964);
        assert!(heap.has_object(&object));
        assert_eq!(*object.read().unwrap(), 42);
        *object.write().unwrap() = 100;
        assert_eq!(*object.read().unwrap(), 100);
        let huge = heap.alloc(42usize);
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 124);
        assert_eq!(heap.available_size(), 900);
        let big = heap.alloc(42u16).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 182);
        assert_eq!(heap.available_size(), 842);
        let small = heap.alloc(42u8).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 239);
        assert_eq!(heap.available_size(), 785);
        drop(huge);
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 175);
        assert_eq!(heap.available_size(), 849);
        drop(big);
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 117);
        assert_eq!(heap.available_size(), 907);
        drop(small);
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 60);
        assert_eq!(heap.available_size(), 964);
        drop(object);
        analize_stats(&heap);
        assert_eq!(heap.pages_count(), 1);
        assert_eq!(heap.capacity(), 1024);
        assert_eq!(heap.size(), 0);
        assert_eq!(heap.available_size(), 1024);

        let dropped = Rc::new(RefCell::new(false));
        heap.alloc(Droppable(dropped.clone())).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 0);
        assert_eq!(heap.available_size(), 1024);
        assert_eq!(*dropped.borrow(), true);
        let keep = heap.alloc(42u8).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        heap.alloc(42u8).unwrap();
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        analize_stats(&heap);
        heap.alloc(42u8).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        heap.alloc(42u8).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        heap.alloc(42u16).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        let temp = heap.alloc(42usize).unwrap();
        analize_stats(&heap);
        assert_eq!(heap.size(), 121);
        assert_eq!(heap.available_size(), 903);
        heap.alloc(Nested(temp));
        analize_stats(&heap);
        assert_eq!(heap.size(), 57);
        assert_eq!(heap.available_size(), 967);
        drop(heap);
        assert_eq!(keep.exists(), false);
    }
}
