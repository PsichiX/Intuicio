# Building custom frontend

This is gonna be basically a pretty much stripped down version of `assembler` frontend just to get intuition on creating frontends. For more advanced examples of frontends please take a look at frontends already made, located in `frontends` folder on repository.

---

Let's start with defining goals for this frontend to achieve:
- scripts operate only on `i32` values.
- scripts will have two operations:
    - push value on stack.
    - call functions that takes values from stack, performs operations on them and push results back on stack.
- syntax of this language has to be simple, so that:
    - each line is an operation or comment.
    - we put operations in reverse order so it is easier to read script as a hierarchy of function calls with its arguments indented and in ascending order.

Frontend syntax:
```text
call lib div
    call lib mul
        push 3
        call lib sub
            call lib add
                push 40
                push 2
            push 10
    push 2
```

---

So now let's create new project and add Intuicio dependencies:
```toml
[dependencies]
intuicio-data = "*"
intuicio-core = "*"
intuicio-derive = "*"
intuicio-backend-vm = "*"
```
Then create `frontend.rs` file, where we will heep all frontend-related code, and first import these dependencies:
```rust
use intuicio_core::prelude::*;
use std::{error::Error, str::FromStr};
```
`intuicio_core` holds types related to script information, we use `Error` trait for errors propagation and `FromStr` for parsing.

---

The most important thing to make is custom Intuiocio expression:
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
Expressions allow to extend available operations set of Intuicio scripts to enable features specific to given frontend - here we just allow to push `i32` literals onto stack.

---

Now we will define intermediate script types for our Custom scripting language:
```rust
pub type CustomScript = Vec<CustomOperation>;

pub enum CustomOperation {
    Comment { content: String },
    Push { value: i32 },
    Call { name: String, module_name: String },
}
```
Next we need to implement parsing of operations from string lines to intermediate script data:
```rust
impl FromStr for CustomOperation {
    type Err = CustomOperationError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Self::Comment {
                content: "".to_owned(),
            });
        }
        if line.starts_with("#") {
            return Ok(Self::Comment {
                content: line.to_owned(),
            });
        }
        let mut tokens = line.split_ascii_whitespace();
        match tokens.next() {
            Some("push") => {
                let value = tokens.next().unwrap().parse::<i32>().unwrap();
                Ok(Self::Push { value })
            }
            Some("call") => {
                let module_name = tokens.next().unwrap().to_owned();
                let name = tokens.next().unwrap().to_owned();
                Ok(Self::Call { name, module_name })
            }
            _ => Err(CustomOperationError {
                operation: line.to_owned(),
            }),
        }
    }
}
```
Also don't forget to implement our parsing error type:
```rust
#[derive(Debug)]
pub struct CustomOperationError {
    pub operation: String,
}

impl std::fmt::Display for CustomOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported operation: `{}`", self.operation)
    }
}

impl Error for CustomOperationError {}
```

---

After that we need to implement script compilation from intermediate to Intuicio scripts data, so scripts will be understood by Intuicio backend:
```rust
impl CustomOperation {
    pub fn compile_operation(&self) -> Option<ScriptOperation<'static, CustomExpression>> {
        match self {
            Self::Comment { .. } => None,
            Self::Push { value } => Some(ScriptOperation::Expression {
                expression: CustomExpression::Literal(*value),
            }),
            Self::Call { name, module_name } => Some(ScriptOperation::CallFunction {
                query: FunctionQuery {
                    name: Some(name.to_owned().into()),
                    module_name: Some(module_name.to_owned().into()),
                    ..Default::default()
                },
            }),
        }
    }

    pub fn compile_script(
        operations: &[CustomOperation],
    ) -> ScriptHandle<'static, CustomExpression> {
        operations
            .iter()
            .rev()
            .filter_map(|operation| operation.compile_operation())
            .collect::<Vec<_>>()
            .into()
    }
}
```
In `compile_script` method we iterate over operations in reverse order, because human-readable side of the scripts expects function call and then its parameters, while Intuicio scripts expect computer-readable order of arguments first, then function call.

---

Finally we create scripts content parser so it can be used to parse byte strings into intermediate type scripts:
```rust
pub struct CustomContentParser;

impl BytesContentParser<CustomScript> for CustomContentParser {
    fn parse(&self, bytes: Vec<u8>) -> Result<CustomScript, Box<dyn Error>> {
        Ok(String::from_utf8(bytes)?
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| CustomOperation::from_str(line))
            .collect::<Result<CustomScript, _>>()?)
    }
}
```

---

Now let's create `main.rs` file, where we will test this frontend, first import dependencies:
```rust
mod frontend;
mod library;

use crate::frontend::*;
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;

fn main() {
    // next steps go here.
}
```
Then parse and compile some script:
```rust
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
```
Next, create and setup registry:
```rust
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
```
As you can see, our scripts do not define functions, rather operations that belong to single one, so we create new main function and add it to the registry. We also use `VmScope` from VM backend to test this frontend in already existing VM backend, until we create dedicated backend ourselves.

Final thing to do is to create host and test frontend:
```rust
let mut host = Host::new(Context::new(10240, 10240), registry.into());
let (result,) = host
    .call_function::<(i32,), _>("main", "main", None)
    .unwrap()
    .run(());
assert_eq!(result, 48);
```
