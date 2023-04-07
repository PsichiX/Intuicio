# Scripting pipeline explained

Here we will attempt to explain entire scripting pipeline using examples of building blocks already made.

## First things first

Now let's pick some building blocks. For backend we will use `intuicio-backend-vm`, and for frontend we will use `intuicio-frontend-assembler` (this may look quite unintuitive but it will allow us better explain how bindings work with scripts, so please bare with me for a little).

Create new Cargo project and add these dependencies:
```toml
[dependencies]
intuicio-core = "*"
intuicio-derive = "*"
intuicio-backend-vm = "*"
intuicio-frontend-assembler = "*"
```

Once we have created new project, now in `main.rs` import these:
```rust
use intuicio_backend_vm::prelude::*;
use intuicio_core::prelude::*;
use intuicio_derive::*;
use intuicio_frontend_assembler::*;
```

---

Now let's define a simple function that our script will run:
```rust
#[intuicio_function(module_name = "lib")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```
We use procedural attribute macro that will generate and fill `Function` type with all the information about this function. We could also construct it by hand, or even with procedural macro like this to register its result into `Registry`:
```rust
registry.add_function(define_function! {
    registry => mod lib fn add(a: i32, b: i32) -> (result: i32) {
        (a + b,)
    }
});
```
But for the sake of simplicity, just stay with `intuicio_function` macro.

---

Then in `main` function first thing we should create is `Registry` to store all definitions of structures and functions, both native and script side ones, we also add basic types to not have to add them manually, and register defined function into that registry:
```rust
let mut registry = Registry::default().with_basic_types();
registry.add_function(add::define_function());
```
Like we sait before, registry should hold all the structures and functions information that scripts can interact with, and because Registry turns immutable once it gets into execution phase, better to do that from the get go - you won't be able to modify registry later.

---

Next step is to load, parse, compile and install assembly script into registry:
```rust
let mut content_provider = FileContentProvider::new("iasm", AsmContentParser);
AsmPackage::new("./resources/main.iasm", &mut content_provider)
    .unwrap()
    .compile()
    .install::<VmScope<AsmExpression>>(&mut registry, None);
```
`FileContentProvider` allows to pull script content from file system, `AsmContentParser` parses assembly script content into `AsmPackage` (intermediate representation of assembly scripts), `VmScope` is a container for script operations compiled from `AsmPackage` and finally `AsmExpression` is a set of custom operations that assembly scripting language performs. Expressions are an essential part of Intuicio scripting, since bare bones script data has very limited, universal set of basic operations required to make scripts call functions and move data between stack and registers - expressions allow extending set of operations, in case of `AsmExpression` scripts can push literals into stack and drop and forget value from stack.

---

Last step is to construct `Context` and put it with `Registry` into `Host` that will allow calling any function from the registry:
```rust
let context = Context::new(
    // stack bytes capacity.
    1024,
    // registers bytes capacity.
    1024,
    // heap page capacity.
    1024,
);
let mut host = Host::new(context, RegistryHandle::new(registry));
let (result,) = host
    .call_function::<(i32,), _>(
        // function name.
        "main",
        // module name.
        "test",
        // structure name if function belongs to one.
        None,
    )
    .unwrap()
    .run(());
assert_eq!(result, 42);
```

---

Now the only thing what's left is to create `./resources/main.iasm` file and fill it with script code:
```
mod test {
    fn main() -> (result: struct i32) {
        literal 40 i32;
        literal 2 i32;
        call mod lib fn add;
    }
}
```
We can see all this script is doing is pushing data on stack and calling a function - this will help us explain what is happening much easier.

## How data moves

Let's start with showing how our `add` function looks actually looks like to `Host`:
```rust
fn add(context: &mut Context, registry: &Registry) {
    let a = context.stack().pop::<i32>().unwrap();
    let b = context.stack().pop::<i32>().unwrap();
    let result = a + b;
    context.stack().push(result);
}
```

Now from the example above we can see that execution starts from application-space when we tell `Host` to find `main` function of `test` module in host's registry, once is found, host passes its `Context` and `Registry` to function generated from script.

This is how `test::main` script function would look like to `Host` when converted to Rust:
```rust
fn main(context: &mut Context, registry: &Registry) {
    context.stack().push(40_i32);
    context.stack().push(2_i32);
    registry
        .find_function(FunctionQuery {
            name: Some("add".into()),
            module_name: Some("lib".into()),
            ..Default::default()
        })
        .unwrap()
        .invoke(context, registry);
}
```
So both together simply reduce to:
```rust
fn main(context: &mut Context, registry: &Registry) {
    context.stack().push(40_i32);
    context.stack().push(2_i32);
    add(context, registry);
}
```
And this is basically exactly how and why Intuicio doesn't care about which side (script or native) calls what side - both script and native sides are calling each other the same way, and `VmScope` is just a container for script operations in the middle of interactions to make script side look the same as native side to host.

# Conclusion

To summarize, Intuicio is actually **_just_** an engine to move data from place to place via stack, it doesn't care what is the data, it doesn't specify limits on the types of data (other than data has to be owned), it also doesn't care what is the place this data moves to, all it does is moves the data and both script and native side tells it when to move what data - simple as that!