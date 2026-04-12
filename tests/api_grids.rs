mod common;

use railroad::{Comment, Diagram, HorizontalGrid, Terminal, VerticalGrid};

#[test]
fn vertical_and_horizontal_grids_render_their_children() {
    let mut vertical = VerticalGrid::new(vec![
        Terminal::new("row one".to_owned()),
        Terminal::new("row two".to_owned()),
    ]);
    vertical
        .attr("data-layout".to_owned())
        .or_insert("vertical".to_owned());
    let vertical_svg = Diagram::new(vertical).to_string();
    assert!(vertical_svg.contains("class=\"verticalgrid\""));
    assert!(vertical_svg.contains("data-layout=\"vertical\""));
    assert!(vertical_svg.contains("row one"));
    assert!(vertical_svg.contains("row two"));

    let mut horizontal = HorizontalGrid::new(vec![
        Comment::new("left".to_owned()),
        Comment::new("right".to_owned()),
    ]);
    horizontal
        .attr("data-layout".to_owned())
        .or_insert("horizontal".to_owned());
    let horizontal_svg = Diagram::new(horizontal).to_string();
    assert!(horizontal_svg.contains("class=\"horizontalgrid\""));
    assert!(horizontal_svg.contains("data-layout=\"horizontal\""));
    assert!(horizontal_svg.contains("left"));
    assert!(horizontal_svg.contains("right"));
}
