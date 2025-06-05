# Tutorial

In this tutorial we will be building entire custom and very simple scripting pipeline step by step, in order:

- [Frontend](./frontend.html)
- [Backend](./backend.html)
- [Runner (REPL)](./runner.html)

We have choosen this particular order because each next part uses things made in its previous steps.

**This entire tutorial sits in `/demos/custom/` project on repository, if you want to look at complete project.**

---

Before we start tutorials, let's provide native-side library of functions we will use in the scripts right away, as `library.rs` file:

```rust
use intuicio_core::prelude::*;
use intuicio_derive::*;

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

pub fn install(registry: &mut Registry) {
    registry.add_function(add::define_function(registry));
    registry.add_function(sub::define_function(registry));
    registry.add_function(mul::define_function(registry));
    registry.add_function(div::define_function(registry));
}
```

So whenever you'll see in next tutorials this line, remember it calls `install` function from `library.rs` provided above:

```rust
crate::library::install(&mut registry);
```
