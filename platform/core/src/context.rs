use intuicio_data::data_stack::{DataStack, DataStackMode, DataStackRegisterAccess};
use std::{any::Any, collections::HashMap};

pub struct Context {
    stack: DataStack,
    registers: DataStack,
    registers_barriers: Vec<usize>,
    custom: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Context {
    pub fn new(stack_capacity: usize, registers_capacity: usize) -> Self {
        Self {
            stack: DataStack::new(stack_capacity, DataStackMode::Values),
            registers: DataStack::new(registers_capacity, DataStackMode::Registers),
            registers_barriers: vec![],
            custom: Default::default(),
        }
    }

    pub fn fork(&self) -> Self {
        Self::new(self.stack.size(), self.registers.size())
    }

    pub fn stack_capacity(&self) -> usize {
        self.stack.size()
    }

    pub fn registers_capacity(&self) -> usize {
        self.registers.size()
    }

    pub fn stack(&mut self) -> &mut DataStack {
        &mut self.stack
    }

    pub fn registers(&mut self) -> &mut DataStack {
        &mut self.registers
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

    pub fn access_register(&'_ mut self, index: usize) -> Option<DataStackRegisterAccess<'_>> {
        let index = self.absolute_register_index(index);
        self.registers.access_register(index)
    }

    pub fn custom<T: Send + Sync + 'static>(&self, name: &str) -> Option<&T> {
        self.custom.get(name)?.downcast_ref::<T>()
    }

    pub fn custom_mut<T: Send + Sync + 'static>(&mut self, name: &str) -> Option<&mut T> {
        self.custom.get_mut(name)?.downcast_mut::<T>()
    }

    pub fn set_custom<T: Send + Sync + 'static>(&mut self, name: impl ToString, data: T) {
        self.custom.insert(name.to_string(), Box::new(data));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async() {
        fn is_async<T: Send + Sync>() {}

        is_async::<Context>();
    }
}
