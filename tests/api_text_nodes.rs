mod common;

use railroad::{Comment, Diagram, HorizontalGrid, NonTerminal, Terminal};

use crate::common::boxed;

#[test]
fn text_nodes_escape_content_and_forward_representative_attributes() {
    let mut terminal = Terminal::new("A < B".to_owned());
    terminal
        .attr("data-token".to_owned())
        .or_insert("term".to_owned());

    let mut non_terminal = NonTerminal::new("expr & tail".to_owned());
    non_terminal
        .attr("data-role".to_owned())
        .or_insert("rule".to_owned());

    let mut comment = Comment::new("note > detail".to_owned());
    comment
        .attr("data-note".to_owned())
        .or_insert("commentary".to_owned());

    let svg = Diagram::new(HorizontalGrid::new(vec![
        boxed(terminal),
        boxed(non_terminal),
        boxed(comment),
    ]))
    .to_string();

    assert!(svg.contains("class=\"terminal\""));
    assert!(svg.contains("class=\"nonterminal\""));
    assert!(svg.contains("class=\"comment\""));
    assert!(svg.contains("A &lt; B"));
    assert!(svg.contains("expr &amp; tail"));
    assert!(svg.contains("note &gt; detail"));
    assert!(svg.contains("data-token=\"term\""));
    assert!(svg.contains("data-role=\"rule\""));
    assert!(svg.contains("data-note=\"commentary\""));
}
