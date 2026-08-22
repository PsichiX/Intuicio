//! The storage a function call runs on.
//!
//! See [`Context`].
use intuicio_data::data_stack::{DataStack, DataStackMode, DataStackRegisterAccess};
use std::{any::Any, collections::HashMap};

/// Everything a running function can reach, other than the registry.
///
/// A context holds three things:
///
/// - a **stack**, which carries arguments and results between calls,
/// - **registers**, the closest thing to local variables,
/// - **custom data**, a name to value map for anything else a frontend needs
///   to keep around.
///
/// Both stack and registers only ever move data, never copy it. A type that
/// wants copies has to provide a function that pushes a duplicate itself.
///
/// Registers are scoped: every call saves the register count on entry and
/// drops back down to it on exit, so a function only sees its own registers.
/// That is what [`Context::store_registers`] and
/// [`Context::restore_registers`] do, and [`crate::function::Function::invoke`]
/// calls them for you.
pub struct Context {
    stack: DataStack,
    registers: DataStack,
    registers_barriers: Vec<usize>,
    custom: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Context {
    /// Allocates a context with fixed stack and register capacities, in bytes.
    ///
    /// Both are rounded up to a power of two. Nothing grows later, so pick
    /// sizes that fit the deepest call chain the scripts will make.
    pub fn new(stack_capacity: usize, registers_capacity: usize) -> Self {
        Self {
            stack: DataStack::new(stack_capacity, DataStackMode::Values),
            registers: DataStack::new(registers_capacity, DataStackMode::Registers),
            registers_barriers: vec![],
            custom: Default::default(),
        }
    }

    /// Builds a fresh, empty context with the same capacities as this one.
    ///
    /// Used to give a worker thread a context of its own.
    pub fn fork(&self) -> Self {
        Self::new(self.stack.size(), self.registers.size())
    }

    /// Returns the stack size in bytes.
    pub fn stack_capacity(&self) -> usize {
        self.stack.size()
    }

    /// Returns the register storage size in bytes.
    pub fn registers_capacity(&self) -> usize {
        self.registers.size()
    }

    /// Returns the value stack, where arguments and results are passed.
    pub fn stack(&mut self) -> &mut DataStack {
        &mut self.stack
    }

    /// Returns the register storage.
    ///
    /// Indices used here are absolute. Prefer [`Context::access_register`],
    /// which counts from the current call's barrier.
    pub fn registers(&mut self) -> &mut DataStack {
        &mut self.registers
    }

    /// Returns stack and registers at once, for moving values between them.
    pub fn stack_and_registers(&mut self) -> (&mut DataStack, &mut DataStack) {
        (&mut self.stack, &mut self.registers)
    }

    /// Marks the current register count, so the next
    /// [`Context::restore_registers`] drops back down to it.
    pub fn store_registers(&mut self) {
        self.registers_barriers
            .push(self.registers.registers_count());
    }

    /// Drops every register defined since the matching
    /// [`Context::store_registers`].
    pub fn restore_registers(&mut self) {
        if let Some(count) = self.registers_barriers.pop() {
            while self.registers.registers_count() > count {
                self.registers.drop_register();
            }
        }
    }

    /// Returns the stored register counts, one per call currently on the stack.
    pub fn registers_barriers(&self) -> &[usize] {
        &self.registers_barriers
    }

    /// Turns a register index relative to the current call into an absolute one.
    pub fn absolute_register_index(&self, index: usize) -> usize {
        self.registers_barriers
            .last()
            .map(|count| index + count)
            .unwrap_or(index)
    }

    /// Takes a handle to one of the current call's registers.
    ///
    /// Returns [`None`] when the index was never defined.
    pub fn access_register(&'_ mut self, index: usize) -> Option<DataStackRegisterAccess<'_>> {
        let index = self.absolute_register_index(index);
        self.registers.access_register(index)
    }

    /// Reads a value stored under `name`, or returns [`None`] when it is absent
    /// or of another type.
    pub fn custom<T: Send + Sync + 'static>(&self, name: &str) -> Option<&T> {
        self.custom.get(name)?.downcast_ref::<T>()
    }

    /// Mutable [`Context::custom`].
    pub fn custom_mut<T: Send + Sync + 'static>(&mut self, name: &str) -> Option<&mut T> {
        self.custom.get_mut(name)?.downcast_mut::<T>()
    }

    /// Stores a value under `name`, replacing anything already there.
    ///
    /// This is the escape hatch for state a frontend needs but the platform does
    /// not model, for example a producer that builds a host per worker thread.
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
