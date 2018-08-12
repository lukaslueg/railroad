### A library to create syntax ("railroad") diagrams as Scalable Vector Graphics (SVG).

**[Some examples](https://htmlpreview.github.io/?https://github.com/lukaslueg/railroad_dsl/blob/master/examples/example_diagrams.html)** using a small [DSL of it's own](https://github.com/lukaslueg/railroad_dsl).


Railroad diagrams are a way to represent context-free grammar. Every diagram has exactly one starting- and end-point; everything that belongs to the described language is represented by one of the possible paths between those points.

Using this library, diagrams are created using primitives which implement `RailroadNode`. Primitives are combined into more complex structures by wrapping simple elements into more complex ones.


```rust
use railroad::*;

let mut seq = Sequence::default();
seq.push(Box::new(Start))
   .push(Box::new(Terminal::new("BEGIN".to_owned())))
   .push(Box::new(NonTerminal::new("syntax".to_owned())))
   .push(Box::new(End));

let mut dia = Diagram::new(seq);

dia.add_element(svg::Element::new("style")
                .set("type", "text/css")
                .text(DEFAULT_CSS));
                
println!("{}", dia);
````
