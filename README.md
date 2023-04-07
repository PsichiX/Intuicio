# Intuicio
![crates-io version](https://raster.shields.io/crates/v/intuicio-core.png)

Modular scripting solution for Rust

## The Book
Here is Intuicio Bible - book explaining what Intuicio is, how it works and
how to use it: **https://psichix.github.io/Intuicio/**

## Important note ⚠
This is highly experimental ecosystem that will evolve in time!

Important things missing in curent version that will get eventually addressed:
- Proper informative errors.
  For the sake of quick iteration there is plenty of `.unwrap()` and panics.
  Only places that were frequently reporting errors, these have got more proper
  human-readable error reporting.
- Performance.
  At this point the bare bone `host` scripting layer is as fast as it can be
  (it competes with other scripting solutions in speed of execution), but the
  VM layer makes running scripts slower, which will be improved in the future
  either by completely rewriting its internals to reduce overhead, changing
  direction completely, or by iteratively improving the most costly bits of it.
- Documentation.
  This is shameful but this ecosystem being at very experimental phase is a
  good enough excuse to approach where we document only bits that are gonna be
  stabilized enough to make sure not being changed - writing informative
  documentation takes twice as producing this piece of software.