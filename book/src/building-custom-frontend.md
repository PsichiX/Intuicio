# Building custom frontend

Here we will create a very simple frontend to put in use all what we have learned before.

This is gonna be basically a pretty much stripped down version of `assembler` frontend just to get intuition on creating frontends. For more advanced examples of frontends please take a look at frontends already made, located in `frontends` folder on repository.

---

Let's start with defining goals for this frontend to achieve:
- scripts operate only on `i32` values.
- scripts will have two operations:
    - push value on stack.
    - call functions that takes values from stack, performs operations on them and push results back on stack.
- syntax of this language has to be simple, so that:
    - each line is an operation.
    - we put operations in reverse order so it is easier to read script as a hierarchy of function calls with its arguments indented and in ascending order.

So now let's create new project and add Intuicio dependencies:
```toml
[dependencies]
intuicio-data = "*"
intuicio-core = "*"
intuicio-derive = "*"
intuicio-backend-vm = "*"
```
Then in `main.rs` import these dependencies:
```rust
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_data::prelude::*;
use intuicio_derive::*;
```
Now define frontend's expression type and implement `ScriptExpression` trait with its `evaluate` method that will perform pushing literal onto stack:
```rust
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
```
Next step is to create a function that will parse text content and convert it into Intuicio script operations:
```rust
fn compile_script(content: &str) -> ScriptHandle<CustomExpression> {
    // use script builder to ease adding next operations.
    let mut result = ScriptBuilder::<CustomExpression>::default();
    // reverse lines order for parsing to turn human-readable version
    // of stack operations into computer-readable stack operations.
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
            // treat any line that doesn't start with expected name
            // as comment.
            _ => {}
        }
    }
    result.build()
}
```
Now let's create few native-side functions to operate on `i32` values for our script to call:
```rust
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
```

---

At this point all what's left is to make `main` function and compile some script:
```rust
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

    // next steps gonna land here.
}
```
Next we create registry and install native functions there:
```rust
let mut registry = Registry::default().with_basic_types();
registry.add_function(add::define_function(&registry));
registry.add_function(sub::define_function(&registry));
registry.add_function(mul::define_function(&registry));
registry.add_function(div::define_function(&registry));
```
You might notice that our scripts doesn't quite look like typical scripts, we do not see defining functions there, only expressions, so we will have to create and register a dedicated function that will use compiled script operations as its body:
```rust
registry.add_function(Function::new(
    function_signature! {
        registry => mod main fn main() -> (result: i32)
    },
    VmScope::<CustomExpression>::generate_function_body(script, None)
        .unwrap()
        .0,
));
```
And finally once we have our registry filled, let's create a host and call that main function and get results of calculations:
```rust
let mut host = Host::new(
    Context::new(
        1024, // stack byte size.
        0, // we don't use registers.
        0, // we don't use heap.
    ),
    registry.into(),
);
let (result,) = host
    .call_function::<(i32,), _>("main", "main", None)
    .unwrap()
    .run(());
assert_eq!(result, 48);
```

---

And that's all for demonstration of how to make simple custom frontend!

Typical scripting languages do have much more complex definition and use intermediate data structures that define their proper script structure which is a result of more complex parsing (frontends in repository do use `pest` for that), then these intermediate script data gets compield into Intuicio script data and that gets installed into the registry.

We might tackle more complex example later, but for now feel free to get inspired by crates in `frontends` folder of this repository, especially `simpleton` if you are searching for how dynamically typed languages can be defined.