# Building custom backend

Goal for our custom backend is to simplify Virtual Machine by limiting its execution to only expressions and function calls, everything else will be treated with errors when used.

You might ask:
> Ok, but our frontend already works on official VM backend, why creating custom one?

And you're completely right, there is no need for custom backend, although showcasing how one could create custom backend might help gain understanding and spark some ideas or even make someone improve if not create completely new backend that's gonna execute scripts much faster than what official VM backend offers!

So yeah, **this part of tutorial is educational only**, there is no need to create custom backends for custom frontends.

---

Now let's start with creating `backend.rs` as part of our existing project and import dependencies, and create custom VM scope type:

```rust
pub struct CustomScope<'a, SE: ScriptExpression> {
    handle: ScriptHandle<'a, SE>,
    position: usize,
}

impl<'a, SE: ScriptExpression> CustomScope<'a, SE> {
    pub fn new(handle: ScriptHandle<'a, SE>) -> Self {
        Self {
            handle,
            position: 0,
        }
    }
}
```

As you can see, it is generic over the expression type of scripts so this backend could be used by any frontend, limiting scripts execution to expressions and function calls only.

Next we implement simple VM execution of entire script operations set:

```rust
impl<'a, SE: ScriptExpression> CustomScope<'a, SE> {
    pub fn run(&mut self, context: &mut Context, registry: &Registry) {
        while let Some(operation) = self.handle.get(self.position) {
            match operation {
                ScriptOperation::None => {
                    self.position += 1;
                }
                ScriptOperation::Expression { expression } => {
                    expression.evaluate(context, registry);
                    self.position += 1;
                }
                ScriptOperation::CallFunction { query } => {
                    let handle = registry
                        .functions()
                        .find(|handle| query.is_valid(handle.signature()))
                        .unwrap_or_else(|| {
                            panic!("Could not call non-existent function: {:#?}", query)
                        });
                    handle.invoke(context, registry);
                    self.position += 1;
                }
                _ => unreachable!("Trying to perform unsupported operation!"),
            }
        }
    }
}
```

And last thing for this file is to implement `ScriptFunctionGenerator` for custom VM scope, so it will take any script and turn it into function body that will be provided later to function definitions:

```rust
impl<SE: ScriptExpression + 'static> ScriptFunctionGenerator<SE> for CustomScope<'static, SE> {
    type Input = ();
    type Output = ();

    fn generate_function_body(
        script: ScriptHandle<'static, SE>,
        ignore: Self::Input,
    ) -> Option<(FunctionBody, Self::Output)> {
        Some((
            FunctionBody::closure(move |context, registry| {
                Self::new(script.clone()).run(context, registry);
            }),
            ignore,
        ))
    }
}
```

---

Finally we need to change our `main.rs` file slightly to use custom VM scope instead of official VM scope.

First we need to make new dependency imports in place of old ones:

```rust
mod backend;
mod frontend;
mod library;

use crate::backend::*;
use crate::frontend::*;
```

We can also remove `intuicio-backend-vm` dependency from `Cargo.toml` since it won't be used anymore.

Next the only thing we change in `main` function is we just replace our `main` scripting function definition into:

```rust
registry.add_function(Function::new(
    function_signature! {
        registry => mod main fn main() -> (result: i32)
    },
    CustomScope::<CustomExpression>::generate_function_body(script, ())
        .unwrap()
        .0,
));
```

So it will generate function body that runs custom VM scope with previously compiled script.
