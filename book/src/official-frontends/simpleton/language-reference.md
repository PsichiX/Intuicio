# Language reference

Let's start with saying that **Simpleton is not an OOP language**, you won't also see any class methods, inheritance or dynamic method dispatch seen in other OOP scripting languages.

**Simpleton is rather functional/procedural scripting language** that operates on structures for data transformations.

## Table of contents

- [Script file structure](#script-file-structure)
- [Structures](#structures)
- [Functions](#functions)
- [Closures](#closures)
- [Primitive types](#primitive-types)
- [Variables](#variables)
- [If-else branching](#if-else-branching)
- [While loops](#while-loops)
- [For loops](#for-loops)
- [Imports](#imports)

## Script file structure

Every script file consists of module definition and module items such as structures and functions:

```javascript
mod vec2 {
    struct Vec2 { x, y }

    func new(x, y) {
        return vec2::Vec2 { x, y };
    }

    func add(a, b) {
        return vec2::Vec2 {
            x: math::add(a.x, b.x),
            y: math::add(a.y, b.y),
        };
    }
}
```

General rule of thumb is that one file describes one module and preferably one structure (if any) so functions in that module are working in context of that structure (somewhat similarly to how GDscript treats its files to some extent).

So later these module types are operated on like this:

```javascript
mod main {
    import "vec2";

    func main() {
        var a = vec2::new(0.0, 1.0);
        var b = vec2::new(1.0, 0.0);
        var c = vec2::add(a, b);
        // vec2::Vec2 { x: 1.0, y: 1.0 }
        console::log_line(debug::debug(c, false));
    }
}
```

First question that comes into mind is:

> Why there are `add` function calls instead of operators being used?

**Simpleton does not have operators** - Simpleton believes in being explicit. If something is doing what function calls does, it should be called as function.

The only exceptions are array and map accessors:

```javascript
var array = [0, 1, 2];
var array_item = array[1];

var map = { a: 0, b: 1, c: 2 };
var map_item = map{"b"};
```

## Structures

Every structure is defined by its name and list of field names:

```javascript
struct Vec2 { x, y }
```

The reason for that is that structures defined in simpleton are concrete objects that take up space defined by number of references they hold, they aren't dynamically sized bags of properties like in many scripting languages.

If we have an object of `vec2::Vec2` type and we want to access its `x` field for reads or writes, it is guaranteed this field exists in the object and we get or set its reference.

And the opposite is true too - if we try to access object field that is not defined in its type, we will get runtime error.

Since Simpleton is not an OOP language, we not only do not have class methods, but also we do not have constructors, therefore we construct objects in-place like this:

```javascript
var v = vec2::Vec2 { x: 42.0 };
```

What this does is Simpleton creates default object of type `vec2::Vec2` and then applies values to fields listed in brackets - this also means that if we omit some fields, object will have their references `null`ed.

Therefore if object expects some specific constraints on the object fields, it's good practice to make functions that return new object in-place with fields filled with arguments validated by that function:

```javascript
func new(x, y) {
    debug::assert(
        math::equals(reflect::type_of(x), <struct math::Real>),
        "`x` is not a number!",
    );
    debug::assert(
        math::equals(reflect::type_of(y), <struct math::Real>),
        "`y` is not a number!",
    );
    return vec2::Vec2 { x, y };
}
```

Additionally in rare situations when we do not know object type at compile-time, we can construct objects by `Type` object found at runtime:

```javascript
var v = reflect::new(<struct vec2::Vec2>, { x: 42 });
```

This is useful especially in case of deserialization, where type is part of deserialized data:

```javascript
var data = {
    type_name: "Vec2",
    type_module_name: "vec2",
    properties: {
        x: 42.0,
    },
};
var type = reflect::find_type_by_name(data.type_name, data.type_module_name);
var v = reflect::new(type, data.properties);
```

And finally to get or set value from object field we use `.` delimiter between object and its field name:

```javascript
var v = vec2::Vec2 { x: 42 };
v.x = math::add(v.x, 10);
```

## Functions

Every function is defined by its name, arguments list and function statements as its body:

```javascript
func sum(a, b, c) {
    console::log_line(
        text::format("sum: {0}, {1}, {2}", [
            reflect::to_text(a),
            reflect::to_text(b),
            reflect::to_text(c),
        ]),
    );

    return math::add(
        math::add(a, b),
        c,
    );
}
```

As you can see there is `return` keyword - it is used to inform Simpleton that we want to exit current function with given value.

Functions always return some reference, if function does not have `return` statement in there, it implicitly returns `null`. We can also `return null;` if we want to exit function without value.

Later functions can be called by providing their module name, function name and arguments:

```javascript
var v = main::sum(1, 2, 3);
```

If we don't know function at compile-time, we can call it at runtime by `Function` object:

```javascript
var v = reflect::call(<func main::sum>, [1, 2, 3]);
```

We can also find function type by its name and module name:

```javascript
var function = reflect::find_function_by_name("sum", "main");
var v = reflect::call(function, [1, 2, 3]);
```

## Closures

Closures are special anonymous functions that can also capture variables from outer scope:

```javascript
var a = 40;
var closure = @[a](b) {
    return math::add(a, b);
};
var v = closure::call(closure, [b]);
```

Under the hood, closures are objects that store reference to `Function` object and list of captured references, and are compiled into actual functions like this:

```javascript
mod _closures {
    func _0(a, b) {
        return math::add(a, b);
    }
}
```

## Primitive types

- `null` - reference that points to nothing.
- `Boolean` - can hold either `true` or `false`.
- `Integer` - their literals are numbers without rational part: `42`. Additionally there are hex (`#A8`) and binary (`$1011`) integer literals.
- `Real` - their literals are numbers with rational part: `4.2`.
- `Text` - also known in other languages as strings of UTF8 characters: `"Hello World!"`.
- `Array` - sequence of value references: `[0, 1, 2]`. We can access its items with: `array[0]`.
- `Map` - unordered table of key-value pairs: `{ a: 0, b: 1, c: 2 }`. We can access its items with: `map{"a"}`.

It's worth noting that all objects, even `Boolean`, `Integer` and `Real` are boxed object - choice made for the sake of simplicity of language implementation, but in the future they might endup unboxed to improve performance - _for now we don't mind them being boxed, at this point in development, performance is not the priority_.

## Variables

Variables are local to the function, they are name aliases for references to values. You define variable with its value like this:

```javascript
var answer = 42;
```

You can also assign new values to existing variables:

```javascript
answer = 10;
```

Since variables are just named references local to given function, you can assign different type values to them than what was stored there at creation:

```javascript
var v = 42;
v = "Hello World!";
```

To get value behind variable you just use its name:

```javascript
var a = 42;
console::log_line(reflect::to_text(a));
```

## If-else branching

Branching allows to execute some scope if condition succeeds:

```javascript
if math::equals(a, 42) {
    console::log_line("`a` equals 42!");
}
```

One can also specify `else` scope in case condition fails:

```javascript
if math::equals(a, 42) {
    console::log_line("`a` equals 42!");
} else {
    console::log_line("`a` does not equals 42!");
}
```

## While loops

While loops allows to execute some scope repeatedly as long as condition succeeds:

```javascript
var a = 0;
while math::less_than(a, 42) {
    a = math::add(a, 1);
}
```

## For loops

For loops combined with iterators allow to iterate on values yielded by them until `null` value is yielded, which signals end of iteration:

```javascript
// 2, 3, 4
for value in iter::walk(2, 3) {
    console::log_line(debug::debug(value, false));
}

// 0, 1, 2
for value in iter::range(0, 3) {
    console::log_line(debug::debug(value, false));
}

var v = [0, 1, 2];
// 2, 1, 0
for value in array::iter(v, true) {
    console::log_line(debug::debug(value, false));
}

var v = { a: 0, b: 1, c: 2};
// { key: "a", value: 1 }, { key: "b", value: 2 }, { key: "c", value: 3 }
for pair in map::iter(v) {
    console::log_line(debug::debug(pair, false));
}

var iter = iter::build([
    iter::range(0, 10),
    [<func iter::filter>, @[](value) {
        return math::equals(
            math::modulo(value, 2),
            0,
        );
    }],
]);
// 0, 2, 4, 6, 8
for pair in iter {
    console::log_line(debug::debug(pair, false));
}
```

## Imports

Packages are built by traversing modules tree - this tree is defined by entry module and its dependencies specified by imports:

```javascript
mod main {
    import "vec2.simp";

    func main() {
        var a = vec2::Vec2 { x: 1, y: 0 };
        var b = vec2::Vec2 { x: 0, y: 1 };
        // vec2::Vec2 { x: 1, y: 1 }
        var c = vec2::add(a, b);
    }
}
```

```javascript
mod vec2 {
    struct Vec2 { x, y }

    func add(a, b) {
        return vec2::Vec2 {
            x: math::add(a.x, b.x),
            y: math::add(a.y, b.y),
        };
    }
}
```

We provide relative path to another module file with `import` keyword.

It's worth noting that file extensions can e omited there, compiler will assume `simp` extension then.
