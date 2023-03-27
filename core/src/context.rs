use intuicio_data::{
    data_heap::DataHeap,
    data_stack::{DataStack, DataStackMode, DataStackRegisterAccess},
};

pub struct Context {
    stack: DataStack,
    registers: DataStack,
    registers_barriers: Vec<usize>,
    heap: DataHeap,
}

impl Context {
    pub fn new(
        stack_capacity: usize,
        registers_capacity: usize,
        heap_page_capacity: usize,
    ) -> Self {
        Self {
            stack: DataStack::new(stack_capacity, DataStackMode::Values),
            registers: DataStack::new(registers_capacity, DataStackMode::Registers),
            registers_barriers: vec![],
            heap: DataHeap::new(heap_page_capacity),
        }
    }

    pub fn stack(&mut self) -> &mut DataStack {
        &mut self.stack
    }

    pub fn registers(&mut self) -> &mut DataStack {
        &mut self.registers
    }

    pub fn heap(&mut self) -> &mut DataHeap {
        &mut self.heap
    }

    pub fn stack_and_registers(&mut self) -> (&mut DataStack, &mut DataStack) {
        (&mut self.stack, &mut self.registers)
    }

    pub fn store_registers(&mut self) {
        self.registers_barriers
            .push(self.registers.registers_count());
    }

    pub fn restore_registers(&mut self) {
        if let Some(count) = self.registers_barriers.pop() {
            while self.registers.registers_count() > count {
                self.registers.drop_register();
            }
        }
    }

    pub fn registers_barriers(&self) -> &[usize] {
        &self.registers_barriers
    }

    pub fn absolute_register_index(&self, index: usize) -> usize {
        self.registers_barriers
            .last()
            .map(|count| index + count)
            .unwrap_or(index)
    }

    pub fn access_register(&mut self, index: usize) -> Option<DataStackRegisterAccess> {
        let index = self.absolute_register_index(index);
        self.registers.access_register(index)
    }
}
