# Building custom runner

In this part of tutorial we will be creating REPL solution that will prompts users to type operations in each next line and issue execution of collected operations as single script.

---

First clear entire `main.rs` file, and start with importing new set of dependencies:

```rust
mod backend;
mod frontend;
mod library;

use crate::backend::*;
use crate::frontend::*;
use intuicio_core::prelude::*;
use std::str::FromStr;
```

Then we create our REPL structure that will hold both collected script operations and host that will run provided scripts:

```rust
struct Repl {
    script: CustomScript,
    host: Host,
}

impl Default for Repl {
    fn default() -> Self {
        let mut registry = Registry::default().with_basic_types();
        crate::library::install(&mut registry);
        let context = Context::new(10240, 10240);
        Self {
            script: Default::default(),
            host: Host::new(context, registry.into()),
        }
    }
}
```

Next we need to implement feeding lines functionality, that will ask user for next operation, parse, compile and either collect into script or execute collected script if user types empty line:

```rust
impl Repl {
    fn feed_line(&mut self) {
        let mut line = String::default();
        if let Err(error) = std::io::stdin().read_line(&mut line) {
            println!("* Could not read line: {}", error);
        }
        if line.trim().is_empty() {
            let (context, registry) = self.host.context_and_registry();
            let script = CustomOperation::compile_script(&self.script);
            let body = match CustomScope::<CustomExpression>::generate_function_body(script, ()) {
                Some(body) => body.0,
                None => {
                    println!("Could not generate custom function body!");
                    return;
                }
            };
            let function = Function::new(
                function_signature! {
                    registry => mod main fn main() -> (result: i32)
                },
                body,
            );
            function.invoke(context, registry);
            if let Some(value) = context.stack().pop::<i32>() {
                println!("* Completed with result: {}", value);
            } else {
                println!("* Completed!");
            }
            self.script.clear();
        } else {
            match CustomOperation::from_str(&line) {
                Ok(operation) => self.script.push(operation),
                Err(error) => println!("* Could not parse operation: {}", error),
            }
        }
    }
}
```

---

Finally here comes `main` function that runs REPL in a loop:

```rust
fn main() {
    let mut repl = Repl::default();
    println!("Custom REPL.\nPlease feed operation per line or type empty line to execute:");
    loop {
        repl.feed_line();
    }
}
```

---

Let's test it:

```text
$ cargo run
Custom REPL.
Please feed operation per line or type empty line to execute:
call lib add
push 40
push 2

* Completed with result: 42
```

---

And with that our simple tutorial completes - i hope you have learned something new today and i hope it gave you some intuition on how to create entire scripting solution with Intuicio!
