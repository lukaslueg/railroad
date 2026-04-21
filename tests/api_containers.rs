mod common;

use railroad::{Choice, Diagram, Empty, MultiChoice, Node, Stack, Terminal};

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

#[test]
fn multichoice_renders_container_markup_and_children() {
    let mut multichoice: MultiChoice<Box<dyn railroad::Node>> = MultiChoice::new(vec![
        vec![
            boxed(Terminal::new("red".to_owned())),
            boxed(Terminal::new("blue".to_owned())),
        ],
        vec![boxed(Terminal::new("green".to_owned()))],
    ]);
    multichoice
        .attr("data-layout".to_owned())
        .or_insert("multichoice".to_owned());

    let svg = Diagram::new(multichoice).to_string();

    assert!(svg.contains("class=\"multichoice\""));
    assert!(svg.contains("data-layout=\"multichoice\""));
    assert!(svg.contains("red"));
    assert!(svg.contains("blue"));
    assert!(svg.contains("green"));
    assert!(svg.matches("<path").count() >= 5);
}

#[test]
fn multichoice_supports_push_column_and_into_inner() {
    let mut multichoice = MultiChoice::default();
    multichoice.push_column(vec![Terminal::new("one".to_owned())]);
    multichoice.push_column(vec![
        Terminal::new("two".to_owned()),
        Terminal::new("three".to_owned()),
    ]);

    let columns = multichoice.into_inner();

    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].len(), 1);
    assert_eq!(columns[1].len(), 2);
}

#[test]
fn multichoice_empty_and_single_column_match_choice_geometry() {
    let empty_choice: Choice<Box<dyn railroad::Node>> = Choice::new(vec![]);
    let empty_multichoice: MultiChoice<Box<dyn railroad::Node>> = MultiChoice::new(vec![]);
    assert_eq!(
        empty_multichoice.entry_height(),
        empty_choice.entry_height()
    );
    assert_eq!(empty_multichoice.height(), empty_choice.height());
    assert_eq!(empty_multichoice.width(), empty_choice.width());

    let choice: Choice<Box<dyn railroad::Node>> = Choice::new(vec![
        boxed(Terminal::new("one".to_owned())),
        boxed(Empty),
        boxed(Terminal::new("three".to_owned())),
    ]);
    let multichoice: MultiChoice<Box<dyn railroad::Node>> = MultiChoice::new(vec![vec![
        boxed(Terminal::new("one".to_owned())),
        boxed(Empty),
        boxed(Terminal::new("three".to_owned())),
    ]]);

    assert_eq!(multichoice.entry_height(), choice.entry_height());
    assert_eq!(multichoice.height(), choice.height());
    assert_eq!(multichoice.width(), choice.width());
}
