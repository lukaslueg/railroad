### A library to create syntax ("railroad") diagrams as Scalable Vector Graphics (SVG).


[![Build status](https://github.com/lukaslueg/railroad/actions/workflows/check.yml/badge.svg)](https://github.com/lukaslueg/railroad/actions/workflows/check.yml)
[![Crates.io Version](https://img.shields.io/crates/v/railroad.svg)](https://crates.io/crates/railroad)
[![Docs](https://docs.rs/railroad/badge.svg)](https://docs.rs/railroad)

**[Live demo](https://lukaslueg.github.io/macro_railroad_wasm_demo/)** ([code](https://github.com/lukaslueg/macro_railroad_wasm))
**[Some examples](https://htmlpreview.github.io/?https://github.com/lukaslueg/railroad_dsl/blob/master/examples/example_diagrams.html)** using a small [DSL of it's own](https://github.com/lukaslueg/railroad_dsl).


Railroad diagrams are a way to represent context-free grammar. Every diagram has exactly one starting- and end-point; everything that belongs to the described language is represented by one of the possible paths between those points.

Using this library, diagrams are created using primitives which implement `Node`. Primitives are combined into more complex structures by wrapping simple elements into more complex ones. The public API stays flat at the crate root, so built-in nodes such as `Sequence`, `Choice`, `Terminal`, `Optional`, and `Diagram` are all available directly from `railroad::*`.


```rust
use railroad::*;

let mut seq: Sequence<Box<dyn Node>> = Sequence::default();
seq.push(Box::new(Start))
   .push(Box::new(Terminal::new("BEGIN".to_owned())))
   .push(Box::new(NonTerminal::new("syntax".to_owned())))
   .push(Box::new(End));

let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::Light);
println!("{}", dia);
```

![diagram for create table sql syntax](https://raw.githubusercontent.com/lukaslueg/railroad/master/examples/create_table_stmt.jpeg)

For simple custom nodes, implementing `entry_height()`, `height()`, `width()`, and `draw()` is often enough. A custom node must only draw within the geometry it advertises, and its connecting path must stay at `y + entry_height()`. Composite or performance-sensitive nodes should usually override `compute_geometry()` and the geometry-aware draw/render hooks so child geometry is computed once and reused.

The lower-level SVG helpers are available as `railroad::svg`. Downstream crates can use them to build custom `Node` implementations while still exposing their nodes through the regular `railroad` API.

When adding new `Node` primitives to this library, `examples/visuals.rs` is a useful manual harness for generating edge cases and checking layout. Use the `visual-debug` feature to add guide lines to the rendered diagram and extra metadata to the SVG output.
