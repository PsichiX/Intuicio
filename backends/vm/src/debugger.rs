//! Watching a script while it runs.
//!
//! A [`VmDebugger`] is called before and after every scope and every operation
//! a [`VmScope`] runs. Attach one with
//! [`VmScope::with_debugger`](crate::scope::VmScope::with_debugger), or hand it
//! to the backend when a package is installed, and every scope of that script
//! passes it down to its children.
//!
//! [`PrintDebugger`] is a ready-made one that prints what it sees, with the
//! stack and the registers as far as it can read them. Give it a [`SourceMap`]
//! and the printout names places in the original source instead of operation
//! indices.
use crate::scope::{VmScope, VmScopeSymbol};
use intuicio_core::{
    context::Context,
    registry::Registry,
    script::{ScriptExpression, ScriptOperation},
};
use intuicio_data::{data_stack::DataStackVisitedItem, type_hash::TypeHash};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    sync::{Arc, RwLock},
};

/// A shared debugger, as a scope holds it.
///
/// Every callback takes the lock with `try_write`, so a debugger that is busy
/// elsewhere is skipped rather than waited for.
pub type VmDebuggerHandle<SE> = Arc<RwLock<dyn VmDebugger<SE> + Send + Sync>>;

/// A shared [`SourceMap`], for tools that build one while they run.
pub type SourceMapHandle<UL> = Arc<RwLock<SourceMap<UL>>>;

/// Callbacks a [`VmScope`] makes while it runs.
///
/// Every method does nothing by default, so implement only the ones you need.
/// They run inside the step, with the context and the registry to hand, so a
/// debugger can read and even change what the script is working on.
pub trait VmDebugger<SE: ScriptExpression> {
    /// Called once before the first operation of a scope.
    #[allow(unused_variables)]
    fn on_enter_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, registry: &Registry) {}

    /// Called when a step leaves the scope with nothing more to run, which is
    /// after its last operation or when an operation ends it early.
    #[allow(unused_variables)]
    fn on_exit_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, registry: &Registry) {}

    /// Called before each operation, with its index in the scope.
    #[allow(unused_variables)]
    fn on_enter_operation(
        &mut self,
        scope: &VmScope<SE>,
        operation: &ScriptOperation<SE>,
        position: usize,
        context: &mut Context,
        registry: &Registry,
    ) {
    }

    /// Called after each operation, with the index it ran at.
    #[allow(unused_variables)]
    fn on_exit_operation(
        &mut self,
        scope: &VmScope<SE>,
        operation: &ScriptOperation<SE>,
        position: usize,
        context: &mut Context,
        registry: &Registry,
    ) {
    }

    /// Wraps this debugger in a shared handle a scope can take.
    fn into_handle(self) -> VmDebuggerHandle<SE>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Arc::new(RwLock::new(self))
    }
}

/// A place in a script, as a [`SourceMap`] keys it.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceMapLocation {
    /// The script this location belongs to.
    pub symbol: VmScopeSymbol,
    /// Index of the operation, or [`None`] for the scope as a whole.
    pub operation: Option<usize>,
}

impl SourceMapLocation {
    /// A location naming a whole script.
    pub fn symbol(symbol: VmScopeSymbol) -> Self {
        Self {
            symbol,
            operation: None,
        }
    }

    /// A location naming one operation of a script.
    pub fn symbol_operation(symbol: VmScopeSymbol, operation: usize) -> Self {
        Self {
            symbol,
            operation: Some(operation),
        }
    }
}

/// What each place in a script came from in the original source.
///
/// `UL` is whatever the frontend wants to point at: a line and column, a file
/// name, a node id in a graph. A frontend fills this in while it produces the
/// script, and a debugger reads it back.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SourceMap<UL> {
    /// Source location of each place in the script.
    pub mappings: HashMap<SourceMapLocation, UL>,
}

impl<UL> SourceMap<UL> {
    /// Returns what `location` came from, or [`None`] when it was never mapped.
    pub fn map(&self, location: SourceMapLocation) -> Option<&UL> {
        self.mappings.get(&location)
    }
}

/// When a [`PrintDebugger`] prints the state around an operation or a scope.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum PrintDebuggerMode {
    /// Print only on the way in.
    Enter,
    /// Print only on the way out.
    Exit,
    /// Print both times. The default.
    #[default]
    All,
}

impl PrintDebuggerMode {
    /// Returns `true` when this mode prints on the way in.
    pub fn can_enter(self) -> bool {
        self == Self::All || self == Self::Enter
    }

    /// Returns `true` when this mode prints on the way out.
    pub fn can_exit(self) -> bool {
        self == Self::All || self == Self::Exit
    }
}

/// A debugger that prints every step to standard output.
///
/// Everything is off by default, so a bare `PrintDebugger` only announces the
/// scopes and operations it passes. Turn the parts you want on with the builder
/// methods, or start from [`PrintDebugger::full`].
///
/// A value on the stack or in a register is only printed as a value when its
/// type was registered with [`PrintDebugger::printable`] or one of its
/// siblings. Anything else is printed as raw bytes.
#[derive(Default)]
pub struct PrintDebugger {
    /// Names the printed places after the original source.
    pub source_map: SourceMap<String>,
    /// Print how many bytes the stack holds.
    pub stack: bool,
    /// Print the raw bytes of the stack.
    pub stack_bytes: bool,
    /// Print each value on the stack, one by one.
    pub visit_stack: bool,
    /// Print the register count and the scope barriers.
    pub registers: bool,
    /// Print the raw bytes of the register storage.
    pub registers_bytes: bool,
    /// Print each register, one by one.
    pub visit_registers: bool,
    /// Print the whole operation rather than only its name.
    pub operation_details: bool,
    /// Wait for a line on standard input after every printout.
    pub step_through: bool,
    /// When to print around an operation or a scope.
    pub mode: PrintDebuggerMode,
    #[allow(clippy::type_complexity)]
    printable: HashMap<
        TypeHash,
        (
            &'static str,
            Box<dyn Fn(&Self, *const ()) -> String + Send + Sync>,
        ),
    >,
    step: usize,
}

impl PrintDebugger {
    /// A debugger with everything turned on, `step_through` included.
    pub fn full() -> Self {
        Self {
            source_map: Default::default(),
            stack: true,
            stack_bytes: true,
            visit_stack: true,
            registers: true,
            registers_bytes: true,
            visit_registers: true,
            operation_details: true,
            step_through: true,
            mode: PrintDebuggerMode::All,
            printable: Default::default(),
            step: 0,
        }
    }

    /// Sets [`PrintDebugger::stack`], builder style.
    pub fn stack(mut self, mode: bool) -> Self {
        self.stack = mode;
        self
    }

    /// Sets [`PrintDebugger::stack_bytes`], builder style.
    pub fn stack_bytes(mut self, mode: bool) -> Self {
        self.stack_bytes = mode;
        self
    }

    /// Sets [`PrintDebugger::visit_stack`], builder style.
    pub fn visit_stack(mut self, mode: bool) -> Self {
        self.visit_stack = mode;
        self
    }

    /// Sets [`PrintDebugger::registers`], builder style.
    pub fn registers(mut self, mode: bool) -> Self {
        self.registers = mode;
        self
    }

    /// Sets [`PrintDebugger::registers_bytes`], builder style.
    pub fn registers_bytes(mut self, mode: bool) -> Self {
        self.registers_bytes = mode;
        self
    }

    /// Sets [`PrintDebugger::visit_registers`], builder style.
    pub fn visit_registers(mut self, mode: bool) -> Self {
        self.visit_registers = mode;
        self
    }

    /// Sets [`PrintDebugger::operation_details`], builder style.
    pub fn operation_details(mut self, mode: bool) -> Self {
        self.operation_details = mode;
        self
    }

    /// Sets [`PrintDebugger::step_through`], builder style.
    ///
    /// With it on, every printout waits for a line on standard input, so the
    /// script only moves when you press enter.
    pub fn step_through(mut self, mode: bool) -> Self {
        self.step_through = mode;
        self
    }

    /// Sets [`PrintDebugger::mode`], builder style.
    pub fn mode(mut self, mode: PrintDebuggerMode) -> Self {
        self.mode = mode;
        self
    }

    /// Prints values of type `T` with their [`Debug`](std::fmt::Debug) output.
    pub fn printable<T: std::fmt::Debug + 'static>(mut self) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (
                std::any::type_name::<T>(),
                Box::new(|_, pointer| unsafe {
                    format!("{:#?}", pointer.cast::<T>().as_ref().unwrap())
                }),
            ),
        );
        self
    }

    /// Prints values of type `T` with a function of your own.
    pub fn printable_custom<T: 'static>(
        mut self,
        f: impl Fn(&Self, &T) -> String + Send + Sync + 'static,
    ) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (
                std::any::type_name::<T>(),
                Box::new(move |debugger, pointer| unsafe {
                    f(debugger, pointer.cast::<T>().as_ref().unwrap())
                }),
            ),
        );
        self
    }

    /// [`PrintDebugger::printable_custom`] with the value as a raw pointer.
    ///
    /// For a type that cannot be named as a Rust reference here. The pointer
    /// given to `f` holds a value of `T`.
    pub fn printable_raw<T: 'static>(
        mut self,
        f: impl Fn(&Self, *const ()) -> String + Send + Sync + 'static,
    ) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (std::any::type_name::<T>(), Box::new(f)),
        );
        self
    }

    /// Registers the Rust primitives, [`char`] and [`String`] as printable.
    pub fn basic_printables(self) -> Self {
        self.printable::<()>()
            .printable::<bool>()
            .printable::<i8>()
            .printable::<i16>()
            .printable::<i32>()
            .printable::<i64>()
            .printable::<i128>()
            .printable::<isize>()
            .printable::<u8>()
            .printable::<u16>()
            .printable::<u32>()
            .printable::<u64>()
            .printable::<u128>()
            .printable::<usize>()
            .printable::<f32>()
            .printable::<f64>()
            .printable::<char>()
            .printable::<String>()
    }

    fn map(&self, location: SourceMapLocation) -> String {
        self.source_map
            .map(location)
            .map(|mapping| mapping.to_owned())
            .unwrap_or_else(|| format!("{location:?}"))
    }

    /// Formats `data` with what was registered for its type.
    ///
    /// Returns the type name and the text, or [`None`] when the type was never
    /// registered as printable.
    pub fn display<T>(&self, data: &T) -> Option<(&'static str, String)> {
        let pointer = data as *const T as *const ();
        self.display_raw(TypeHash::of::<T>(), pointer)
    }

    /// [`PrintDebugger::display`] for a value whose type is only known at
    /// runtime.
    ///
    /// `pointer` must hold a value of the type named by `type_hash`, which is
    /// what the caller has to get right.
    pub fn display_raw(
        &self,
        type_hash: TypeHash,
        pointer: *const (),
    ) -> Option<(&'static str, String)> {
        let (type_name, callback) = self.printable.get(&type_hash)?;
        let result = callback(self, pointer);
        Some((type_name, result))
    }

    fn print_extra(&self, context: &mut Context) {
        if self.stack {
            println!("- stack position: {}", context.stack().position());
        }
        if self.stack_bytes {
            println!("- stack bytes:\n{:?}", context.stack().as_bytes());
        }
        if self.visit_stack {
            let mut index = 0;
            context.stack().visit(|item| {
                let DataStackVisitedItem::Value {
                    type_hash,
                    layout,
                    data: bytes,
                    range,
                } = item else {
                    return true;
                };
                assert_eq!(bytes.len(), layout.size());
                if let Some((type_name, callback)) = self.printable.get(&type_hash) {
                    println!(
                        "- stack value #{} of type {}:\n{}",
                        index,
                        type_name,
                        callback(self, bytes.as_ptr().cast::<()>())
                    );
                } else {
                    println!(
                        "- stack value #{index} of unknown type id {type_hash:?} and layout: {layout:?}"
                    );
                }
                println!(
                    "- stack value #{index} bytes in range {range:?}:\n{bytes:?}"
                );
                index += 1;
                true
            });
        }
        if self.registers {
            println!("- registers position: {}", context.registers().position());
            println!(
                "- registers count: {}",
                context.registers().registers_count()
            );
            println!("- registers barriers: {:?}", context.registers_barriers());
        }
        if self.registers_bytes {
            println!("- registers bytes:\n{:?}", context.registers().as_bytes());
        }
        if self.visit_registers {
            let mut index = 0;
            let registers_count = context.registers().registers_count();
            context.registers().visit(|item| {
                let DataStackVisitedItem::Register {
                    type_hash,
                    layout,
                    data: bytes,
                    range,
                    valid,
                } = item
                else {
                    return true;
                };
                if let Some((type_name, callback)) = self.printable.get(&type_hash) {
                    if valid {
                        println!(
                            "- register value #{} of type {}:\n{}",
                            registers_count - index - 1,
                            type_name,
                            callback(self, bytes.as_ptr().cast::<()>())
                        );
                    } else {
                        println!(
                            "- invalid register value #{} of type {}",
                            registers_count - index - 1,
                            type_name
                        );
                    }
                } else {
                    println!(
                        "- register value #{} of unknown type id {:?} and layout: {:?}",
                        registers_count - index - 1,
                        type_hash,
                        layout
                    );
                }
                println!(
                    "- register value #{} bytes in range: {:?}:\n{:?}",
                    registers_count - index - 1,
                    range,
                    bytes
                );
                index += 1;
                true
            });
        }
    }

    fn try_halt(&self) {
        if self.step_through {
            print!("#{} | Confirm to step through...", self.step);
            let _ = std::io::stdout().flush();
            let mut command = String::new();
            let _ = std::io::stdin().read_line(&mut command);
        }
    }
}

impl<SE: ScriptExpression + std::fmt::Debug> VmDebugger<SE> for PrintDebugger {
    fn on_enter_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, _: &Registry) {
        println!();
        println!(
            "* #{} PrintDebugger | Enter scope:\n{}",
            self.step,
            self.map(SourceMapLocation::symbol(scope.symbol()))
        );
        if self.mode.can_enter() {
            self.print_extra(context);
            self.try_halt();
        }
        println!();
        self.step += 1;
    }

    fn on_exit_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, _: &Registry) {
        println!();
        println!(
            "* #{} PrintDebugger | Exit scope:\n{}",
            self.step,
            self.map(SourceMapLocation::symbol(scope.symbol()))
        );
        if self.mode.can_exit() {
            self.print_extra(context);
            self.try_halt();
        }
        println!();
        self.step += 1;
    }

    fn on_enter_operation(
        &mut self,
        scope: &VmScope<SE>,
        operation: &ScriptOperation<SE>,
        position: usize,
        context: &mut Context,
        _: &Registry,
    ) {
        println!();
        println!(
            "* #{} PrintDebugger | Enter operation:\n{}",
            self.step,
            self.map(SourceMapLocation::symbol_operation(
                scope.symbol(),
                position
            ))
        );
        if self.mode.can_enter() {
            println!(
                "- operation: {}",
                if self.operation_details {
                    format!("{operation:#?}")
                } else {
                    operation.label().to_owned()
                }
            );
            self.print_extra(context);
            self.try_halt();
        }
        println!();
        self.step += 1;
    }

    fn on_exit_operation(
        &mut self,
        scope: &VmScope<SE>,
        operation: &ScriptOperation<SE>,
        position: usize,
        context: &mut Context,
        _: &Registry,
    ) {
        println!();
        println!(
            "* #{} PrintDebugger | Exit operation:\n{}",
            self.step,
            self.map(SourceMapLocation::symbol_operation(
                scope.symbol(),
                position
            ))
        );
        if self.mode.can_exit() {
            println!(
                "- operation: {}",
                if self.operation_details {
                    format!("{operation:#?}")
                } else {
                    operation.label().to_owned()
                }
            );
            self.print_extra(context);
            self.try_halt();
        }
        println!();
        self.step += 1;
    }
}
