mod frontend;
mod library;

use crate::frontend::*;
use intuicio_backend_vm::scope::VmScope;
use intuicio_core::{
    context::Context,
    function::Function,
    function_signature,
    host::Host,
    registry::Registry,
    script::{BytesContentParser, ScriptFunctionGenerator},
};

fn main() {
    let script = b"
    call lib div
        call lib mul
            push 3
            call lib sub
                call lib add
                    push 40
                    push 2
                push 10
        push 2
    ";
    let script = CustomContentParser.parse(script.to_vec()).unwrap();
    let script = CustomOperation::compile_script(&script);

    let mut registry = Registry::default().with_basic_types();
    crate::library::install(&mut registry);
    registry.add_function(Function::new(
        function_signature! {
            registry => mod main fn main() -> (result: i32)
        },
        VmScope::<CustomExpression>::generate_function_body(script, None)
            .unwrap()
            .0,
    ));

    let mut host = Host::new(Context::new(10240, 10240), registry.into());
    let (result,) = host
        .call_function::<(i32,), _>("main", "main", None)
        .unwrap()
        .run(());
    assert_eq!(result, 48);
}
