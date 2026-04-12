use railroad::{Diagram, End, Node, Sequence, Start, Stylesheet, Terminal};

#[allow(dead_code)]
pub fn boxed<N>(node: N) -> Box<dyn Node>
where
    N: Node + 'static,
{
    Box::new(node)
}

#[allow(dead_code)]
pub fn basic_sequence() -> Sequence<Box<dyn Node>> {
    Sequence::new(vec![
        boxed(Start),
        boxed(Terminal::new("BEGIN".to_owned())),
        boxed(End),
    ])
}

#[allow(dead_code)]
pub fn render_svg<N>(node: N) -> String
where
    N: Node,
{
    Diagram::new_with_stylesheet(node, &Stylesheet::Light).to_string()
}
