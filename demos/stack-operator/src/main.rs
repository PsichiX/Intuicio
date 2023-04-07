use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_derive::*;

#[derive(Debug)]
pub enum CustomExpression {
    Literal(i32),
}

impl ScriptExpression for CustomExpression {
    fn evaluate(&self, context: &mut Context, _: &Registry) {
        match self {
            Self::Literal(value) => {
                context.stack().push(*value);
            }
        }
    }
}

fn compile_script(content: &str) -> ScriptHandle<CustomExpression> {
    let mut result = ScriptBuilder::<CustomExpression>::default();
    for line in content.lines().rev() {
        if line.is_empty() {
            continue;
        }
        let mut tokens = line.split_ascii_whitespace();
        match tokens.next() {
            Some("push") => {
                let value = tokens.next().unwrap().parse::<i32>().unwrap();
                result = result.expression(CustomExpression::Literal(value));
            }
            Some("call") => {
                let module_name = tokens.next().unwrap();
                let name = tokens.next().unwrap();
                result = result.call_function(FunctionQuery {
                    name: Some(name.into()),
                    module_name: Some(module_name.into()),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
    result.build()
}

#[intuicio_function(module_name = "lib")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[intuicio_function(module_name = "lib")]
fn sub(a: i32, b: i32) -> i32 {
    a - b
}

#[intuicio_function(module_name = "lib")]
fn mul(a: i32, b: i32) -> i32 {
    a * b
}

#[intuicio_function(module_name = "lib")]
fn div(a: i32, b: i32) -> i32 {
    a / b
}

fn main() {
    let script = compile_script(
        r#"
        call lib div
            call lib mul
                push 3
                call lib sub
                    call lib add
                        push 40
                        push 2
                    push 10
            push 2
        "#,
    );

    let mut registry = Registry::default().with_basic_types();
    registry.add_function(add::define_function(&registry));
    registry.add_function(sub::define_function(&registry));
    registry.add_function(mul::define_function(&registry));
    registry.add_function(div::define_function(&registry));
    registry.add_function(Function::new(
        function_signature! {
            registry => mod main fn main() -> (result: i32)
        },
        VmScope::<CustomExpression>::generate_function_body(script, None)
            .unwrap()
            .0,
    ));

    let mut host = Host::new(Context::new(1024, 1024, 1024), registry.into());
    let (result,) = host
        .call_function::<(i32,), _>("main", "main", None)
        .unwrap()
        .run(());
    assert_eq!(result, 48);
}
