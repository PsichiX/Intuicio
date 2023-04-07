# Introduction

**Project repository**: [github.com/PsichiX/Intuicio](https://github.com/PsichiX/Intuicio)

![crates-io version](https://raster.shields.io/crates/v/intuicio-core.png)

## What is Intuicio?

In short words: Intuicio is a set of building blocks designed to build your own scripting solution in Rust.

Every complete scripting solution built on Intuicio is split into:

## Script

Scripts are all the information that defines a script in form of data.

It is an interface between frontends and backends to allow modularity and relatively universal way of communication between them.

Scripts are produced by frontends for backends to "run" them (more precisely to just use them, usually backends are used to execute scripts, but one can create something like a nativizer to transpile script into native code to run).

Scripts data is defined in `intuicio-core` crate.

## Frontend

Frontends are used to convert some data into scripts data.

Usually when we talk about scripting frontend, we are talking about compilers/transpilers for example - in general frontends can parse text code file for particular scripting language and turn it into Intuicio scripts that will be later used by backends. This doesn't mean we are forced to parsing text code files - one can create a node-graph-like scripting language and frontend for converting that into scripts, or even turn images into scripts, only limit is imagination, all Intuicio needs is just scripts data, and how we get them doesn't matter.

There are examples of few frontends in Intuicio repositiory `frontends` folder, these are:
- `intuicio-frontend-assembler` - simple assembler-like language that has the closest representation to script data one can get.
- `intuicio-frontend-serde` - this one allows to represent scripts in any of the file formats that `serde` can support, for example JSON and YAML, or even LEXPR.
- `intuicio-frontend-vault` - an attempt to create strongly typed language written with LEXPR syntax.
- `intuicio-frontend-simpleton` - a full example of simple dynamically typed scripting language with rich standard library.

## Backend

Backends are used to "run" scripts, or more specifically to use scripts produced by frontends in any way they want.

An obvious backend that anyone can think of is a Virtual Machine, already made as `intuicio-backend-vm` crate. This one grabs script data and executes it directly in `VmScope`s that are self-contained units that executes set of script operations.

Another example of backend can be "nativizer" - nativizers are units that transpile scripts into native code, in this case Rust code. Nativizers are great way to speed up scripts execution, by removing the need for constructing and loading scripts at runtime, rather to just get native code that would do what scripts do, but without extra overhead. Although there is an `intuicio-backend-rust` crate that aims to do that, it is still incomplete, mostly non-functional until it gets proper definition, but experiments are being made and eventually Intuicio will have its own default nativizer.

## Host

Host is basically an application-space (native side) of scripting logic, where user creates libraries of native functions and structs to bind into `Registry`, which later can be called or accessed by script operations or other native side code within given `Context`, shared between scripting and native side and treated equally as same.

The goal here is to allow for seamless interoperability between scripting and native sides of program logic, not forcing users to focus all their effort onto one particular side, something that is quite unique for scripting solutions, and this design decision was borrowed from Unreal Engine where it proven to be at least quite useful.