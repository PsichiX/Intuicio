mod backend;
mod frontend;
mod library;

use crate::backend::*;
use crate::frontend::*;
use intuicio_core::prelude::*;
use std::str::FromStr;

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

fn main() {
    let mut repl = Repl::default();
    println!("Custom REPL.\nPlease feed operation per line or type empty line to execute:");
    loop {
        repl.feed_line();
    }
}
