mod common;

use railroad::{Choice, Diagram, Empty, Stack, Terminal};

use crate::common::boxed;

#[test]
fn sequence_supports_builder_style_composition() {
    let mut sequence: railroad::Sequence<Box<dyn railroad::Node>> = railroad::Sequence::default();
    sequence
        .push(boxed(railroad::Start))
        .push(boxed(Terminal::new("middle".to_owned())))
        .push(boxed(railroad::End));

    let svg = Diagram::new(sequence).to_string();

    assert!(svg.contains("class=\"sequence\""));
    assert!(svg.contains("middle"));
    assert!(svg.matches("<path").count() >= 3);
}

#[test]
fn stack_and_choice_render_container_markup_and_children() {
    let mut stack = Stack::new(vec![
        Terminal::new("top".to_owned()),
        Terminal::new("bottom".to_owned()),
    ]);
    stack
        .attr("data-layout".to_owned())
        .or_insert("stack".to_owned());
    let stack_svg = Diagram::new(stack).to_string();
    assert!(stack_svg.contains("class=\"stack\""));
    assert!(stack_svg.contains("data-layout=\"stack\""));
    assert!(stack_svg.contains("top"));
    assert!(stack_svg.contains("bottom"));

    let mut choice: Choice<Box<dyn railroad::Node>> = Choice::new(vec![
        boxed(Terminal::new("one".to_owned())),
        boxed(Terminal::new("two".to_owned())),
        boxed(Empty),
    ]);
    choice
        .attr("data-layout".to_owned())
        .or_insert("choice".to_owned());
    let choice_svg = Diagram::new(choice).to_string();
    assert!(choice_svg.contains("class=\"choice\""));
    assert!(choice_svg.contains("data-layout=\"choice\""));
    assert!(choice_svg.contains("one"));
    assert!(choice_svg.contains("two"));
    assert!(choice_svg.matches("<path").count() >= 3);
}
