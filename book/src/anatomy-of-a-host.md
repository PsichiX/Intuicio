# Anatomy of a Host

`Host` is just a handy container for `Context` and `Registry` that simplifies calling functions registered in `Registry` within given `Context`.

## Registry

### Setup

`Registry` contains all structures and functions that both scripting and native side expose for scripting to interact with.

Here is an example of how to register functions both from native and script sides:
```rust
#[intuicio_function(module_name = "lib")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

let mut registry = Registry::default().with_basic_types();
registry.add_function(add::define_function());
let mut content_provider = FileContentProvider::new("iasm", AsmContentParser);
AsmPackage::new("main.iasm", &mut content_provider)
    .unwrap()
    .compile()
    .install::<VmScope<AsmExpression>>(&mut registry, None);
```
Of course we don't actually require frontends to register script-side functions and structures, here is an example of how one could create raw scripted function, knowing the backend it's gonna use (`VmScope` here):
```rust
registry.add_function(
    VmScope::<AsmExpression>::generate_function(
        &ScriptFunction {
            signature: function_signature! {
                registry => mod test fn main() -> (result: i32)
            },
            script: ScriptBuilder::<AsmExpression>::default()
                .literal(AsmExpression::Literal(AsmLiteral::I32(2)))
                .literal(AsmExpression::Literal(AsmLiteral::I32(40)))
                .call_function(FunctionQuery {
                    name: Some("add".into()),
                    module_name: Some("lib".into()),
                    ..Default::default()
                })
                .build(),
        },
        &registry,
        None,
    )
    .unwrap()
    .0,
);
```
And as we can see above, we can use completely different backends for each function we want to register, therefore in principle, one application can run multiple backends, not forcing user to use only one.

> Also remember that since Intuicio is all about moving data into and out of function calls, every function that we want to call, has to be always registered in the registry and registry cannot be modified when used in function calls!

### Queries

If user wants to call function from registry, user has to find it first and to do that we use `FunctionQuery` that defines search parameters for registry to filter functions with:
```rust
let (result,) = registry.find_function(FunctionQuery {
    name: Some("add".into()),
    module_name: Some("lib".into()),
    ..Default::default()
})
.expect("`lib::add` function not found!")
.call::<(i32,), _>(&mut context, &registry, (40_i32, 2_i32), true);
assert_eq!(result, 42);
```

When we want to find and call registered function from within another native function, we should be able as long as native-side function has access to context and registry:
```rust
#[intuicio_function(module_name = "script", use_context, use_registry)]
fn script_add(context: &mut Context, registry: &Registry, a: i32, b: i32) -> i32 {
    context.stack().push(b);
    context.stack().push(a);
    registry.find_function(FunctionQuery {
        name: Some("add".into()),
        module_name: Some("lib".into()),
        ..Default::default()
    })
    .expect("`lib::add` function not found!")
    .invoke(context, registry);
    context.stack().pop::<i32>().expect("Expected to return `i32`!")
}
```
Btw. in snippet above we perform function invoke instead of a call, and notice order of pushing values into stack - by design native functions expect to pop their arguments from first to last argument, and push its result in reverse order to match later function calls proper argument pop order. For convienience it is advised to perform function calls instead of invokes, because function calls keep proper stack push and pop order on their own.

## Context

`Context` is a container that holds:
- **Stack**

    Used to move data between function calls.
    ```rust
    context.stack().push(42_i32);
    assert_eq!(context.stack().pop::<i32>().unwrap(), 42);
    ```

- **Registers**

    Indexed data storage, the closest analogue for local function variables.
    ```rust
    let (stack, registers) = context.stack_and_registers();
    let index = registers.push_register().unwrap();
    stack.push(42_i32);
    let mut register = registers.access_register(index).unwrap();;
    registers.pop_to_register(&mut register);
    stack.push_from_register(&mut register);
    assert_eq!(stack.pop::<i32>().unwrap(), 42);
    ```
    Please remember that with registers, just like with stack, data can be only moved in and out of registers, registers operations does not copy/clone their data - this design choice was dictated by master rule of Intuicio: "data can only be moved", copy/clone is a special operation that given structure has to provide a dedicated function for it to push duplicated source data into stack, from which original data gets moved back to register and its clone stays on the stack - this is what for example `simpleton` frontend does when it has to copy `Reference` from local variable to stack for later use.

- **Heap**

    Used to store dynamically allocated data in case user wants to ensure that data lifetime to be bound to the context (die along with context death). To be honest, this is not widely used piece of context, since data stored in any of the other pieces of context has no requirement to come from context's heap, it works perfectly fine with data allocated purely on rust-side, although at some point there might be a scenario where having boxed data owned by context is beneficial, therefore it is exposed to the user.
    ```rust
    let mut value = context.heap().alloc(0_i32);
    *value.write().uwnrap() = 42;
    assert_eq!(*value.read().unwrap(), 42);
    ```
    It's worth noting that memory allocated by heap box gets automatically returned to the heap once heap box drops, so there is no explicit heap box deallocation.

- **Custom data**

    Now this is the interesting bit, it is basically a hash map of `Box<Any + Send + Sync>` objects that does not fit to any of the other context pieces. It is useful for storing any meta information. For example `simpleton` frontend stores there its `HostProducer` that is used to construct new `Host` for any spawned `Jobs` worker thread, so each worker thread can execute closures passed into it.
    ```rust
    context.set_custom("foo", 42_i32);
    assert_eq!(*context.custom::<i32>().unwrap(), 42);
    ```
