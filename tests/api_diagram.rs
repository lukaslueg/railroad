mod common;

use railroad::{DEFAULT_CSS, Diagram, Terminal, svg};

use crate::common::{basic_sequence, render_svg};

#[test]
fn diagram_includes_stylesheets_attributes_and_extra_elements() {
    let mut diagram = Diagram::new_with_stylesheet(basic_sequence(), &railroad::Stylesheet::Light);
    diagram.add_css(".accent { fill: rebeccapurple; }");
    diagram
        .add_element(svg::Element::new("defs").add(svg::Element::new("marker").set("id", "arrow")));
    diagram
        .attr("data-kind".to_owned())
        .or_insert("demo".to_owned());

    let svg = diagram.to_string();

    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("class=\"railroad\""));
    assert!(svg.contains("viewBox=\"0 0 "));
    assert!(svg.contains("data-kind=\"demo\""));
    assert!(svg.contains(DEFAULT_CSS.trim()));
    assert!(svg.contains(".accent { fill: rebeccapurple; }"));
    assert!(svg.contains("<defs>"));
    assert!(svg.contains("id=\"arrow\""));
    assert!(svg.contains("class=\"railroad_canvas\""));
}

#[test]
fn diagram_write_matches_display_output() {
    let diagram = Diagram::with_default_css(Terminal::new("write".to_owned()));
    let expected = diagram.to_string();

    let mut buf = Vec::new();
    diagram.write(&mut buf).unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), expected);
}

#[test]
fn diagram_into_inner_returns_root_node() {
    let root = Terminal::new("inner".to_owned());
    let expected = render_svg(Terminal::new("inner".to_owned()));

    let svg = render_svg(Diagram::new(root).into_inner());

    assert_eq!(svg, expected);
}
