# Simpleton

High level text-based scripting language.

Key features:
- Dynamically typed.
- Influenced by **JavaScript** and **Python**.
- Simple to understand and learn.
- Rich standard library.

**Useful for scripting tools and game logic, where simplicity and ergonomics is vastly more important than performance.**

## Goals

Simpleton was created for simplicity in mind as priority, mostly for allowing to create tools and simple game logic the easiest way possible.

It also has an educational value to showcase Intuicio users how one can create fully fledged dynamically typed scripting language - something that might be counter intuitive since Intuicio is rather strongly typed scripting platform. We achieve that by introducing Reference, Type and Function as fundamental Simpleton types, so that entire frontend is built around interactions with these types.

## Syntax
```javascript
mod main {
    func main(args) {
        console::log_line("Hello World!");
        
        var fib = main::fib(10);
        console::log_line(debug::debug(fib, false));
    }

    func fib(n) {
        if math::less_than(n, 2) {
            return n;
        } else {
            return math::add(
                main::fib(math::sub(n, 1)),
                main::fib(math::sub(n, 2)),
            );
        }
    }
}
```