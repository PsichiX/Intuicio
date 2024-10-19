use crate::scope::{VmScope, VmScopeSymbol};
use intuicio_core::{
    context::Context,
    registry::Registry,
    script::{ScriptExpression, ScriptOperation},
};
use intuicio_data::type_hash::TypeHash;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    sync::{Arc, RwLock},
};

pub type VmDebuggerHandle<SE> = Arc<RwLock<dyn VmDebugger<SE> + Send + Sync>>;
pub type SourceMapHandle<UL> = Arc<RwLock<SourceMap<UL>>>;

pub trait VmDebugger<SE: ScriptExpression> {
    #[allow(unused_variables)]
    fn on_enter_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, registry: &Registry) {}

    #[allow(unused_variables)]
    fn on_exit_scope(&mut self, scope: &VmScope<SE>, context: &mut Context, registry: &Registry) {}

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

    fn into_handle(self) -> VmDebuggerHandle<SE>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Arc::new(RwLock::new(self))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceMapLocation {
    pub symbol: VmScopeSymbol,
    pub operation: Option<usize>,
}

impl SourceMapLocation {
    pub fn symbol(symbol: VmScopeSymbol) -> Self {
        Self {
            symbol,
            operation: None,
        }
    }

    pub fn symbol_operation(symbol: VmScopeSymbol, operation: usize) -> Self {
        Self {
            symbol,
            operation: Some(operation),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SourceMap<UL> {
    pub mappings: HashMap<SourceMapLocation, UL>,
}

impl<UL> SourceMap<UL> {
    pub fn map(&self, location: SourceMapLocation) -> Option<&UL> {
        self.mappings.get(&location)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum PrintDebuggerMode {
    Enter,
    Exit,
    #[default]
    All,
}

impl PrintDebuggerMode {
    pub fn can_enter(self) -> bool {
        self == Self::All || self == Self::Enter
    }

    pub fn can_exit(self) -> bool {
        self == Self::All || self == Self::Exit
    }
}

#[derive(Default)]
pub struct PrintDebugger {
    pub source_map: SourceMap<String>,
    pub stack: bool,
    pub stack_bytes: bool,
    pub visit_stack: bool,
    pub registers: bool,
    pub registers_bytes: bool,
    pub visit_registers: bool,
    pub operation_details: bool,
    pub step_through: bool,
    pub mode: PrintDebuggerMode,
    #[allow(clippy::type_complexity)]
    printable: HashMap<TypeHash, (&'static str, Box<dyn Fn(&[u8]) -> String + Send + Sync>)>,
    step: usize,
}

impl PrintDebugger {
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

    pub fn stack(mut self, mode: bool) -> Self {
        self.stack = mode;
        self
    }

    pub fn stack_bytes(mut self, mode: bool) -> Self {
        self.stack_bytes = mode;
        self
    }

    pub fn visit_stack(mut self, mode: bool) -> Self {
        self.visit_stack = mode;
        self
    }

    pub fn registers(mut self, mode: bool) -> Self {
        self.registers = mode;
        self
    }

    pub fn registers_bytes(mut self, mode: bool) -> Self {
        self.registers_bytes = mode;
        self
    }

    pub fn visit_registers(mut self, mode: bool) -> Self {
        self.visit_registers = mode;
        self
    }

    pub fn operation_details(mut self, mode: bool) -> Self {
        self.operation_details = mode;
        self
    }

    pub fn step_through(mut self, mode: bool) -> Self {
        self.step_through = mode;
        self
    }

    pub fn mode(mut self, mode: PrintDebuggerMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn printable<T: std::fmt::Debug + 'static>(mut self) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (
                std::any::type_name::<T>(),
                Box::new(|bytes| unsafe {
                    format!("{:#?}", bytes.as_ptr().cast::<T>().as_ref().unwrap())
                }),
            ),
        );
        self
    }

    pub fn printable_custom<T: 'static>(
        mut self,
        f: impl Fn(&T) -> String + Send + Sync + 'static,
    ) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (
                std::any::type_name::<T>(),
                Box::new(move |bytes| unsafe { f(bytes.as_ptr().cast::<T>().as_ref().unwrap()) }),
            ),
        );
        self
    }

    pub fn printable_raw<T: 'static>(
        mut self,
        f: impl Fn(&[u8]) -> String + Send + Sync + 'static,
    ) -> Self {
        self.printable.insert(
            TypeHash::of::<T>(),
            (std::any::type_name::<T>(), Box::new(f)),
        );
        self
    }

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
            .unwrap_or_else(|| format!("{:?}", location))
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
            context.stack().visit(|type_hash, layout, bytes, range, _| {
                assert_eq!(bytes.len(), layout.size());
                if let Some((type_name, callback)) = self.printable.get(&type_hash) {
                    println!(
                        "- stack value #{} of type {}:\n{}",
                        index,
                        type_name,
                        callback(bytes)
                    );
                } else {
                    println!(
                        "- stack value #{} of unknown type id {:?} and layout: {:?}",
                        index, type_hash, layout
                    );
                }
                println!(
                    "- stack value #{} bytes in range {:?}:\n{:?}",
                    index, range, bytes
                );
                index += 1;
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
            context
                .registers()
                .visit(|type_hash, layout, bytes, range, valid| {
                    if let Some((type_name, callback)) = self.printable.get(&type_hash) {
                        if valid {
                            println!(
                                "- register value #{} of type {}:\n{}",
                                registers_count - index - 1,
                                type_name,
                                callback(bytes)
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
                    format!("{:#?}", operation)
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
                    format!("{:#?}", operation)
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
