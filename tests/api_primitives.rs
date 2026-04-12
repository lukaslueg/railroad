use railroad::{
    Debug, Diagram, Empty, End, Node, NodeCollection, NodeGeometry, SimpleEnd, SimpleStart, Start,
    Stylesheet, svg,
};

use crate::common::boxed;

mod common;

#[test]
fn stylesheet_helpers_preserve_theme_and_render_safety() {
    assert_eq!(Stylesheet::Light.to_dark(), Stylesheet::Dark);
    assert_eq!(
        Stylesheet::LightRendersafe.to_dark(),
        Stylesheet::DarkRendersafe
    );
    assert_eq!(Stylesheet::Dark.to_light(), Stylesheet::Light);
    assert_eq!(
        Stylesheet::DarkRendersafe.to_light(),
        Stylesheet::LightRendersafe
    );
    assert!(Stylesheet::Light.is_light());
    assert!(Stylesheet::LightRendersafe.is_light());
    assert!(!Stylesheet::Dark.is_light());
    assert_eq!(Stylesheet::Light.stylesheet(), railroad::DEFAULT_CSS);
}

#[test]
fn node_defaults_and_collection_helpers_follow_the_public_contract() {
    struct LeafNode;

    impl Node for LeafNode {
        fn entry_height(&self) -> i64 {
            3
        }

        fn height(&self) -> i64 {
            9
        }

        fn width(&self) -> i64 {
            15
        }

        fn draw(&self, x: i64, y: i64, h_dir: svg::HDir) -> svg::Element {
            let direction = match h_dir {
                svg::HDir::LTR => "ltr",
                svg::HDir::RTL => "rtl",
            };
            svg::Element::new("leaf")
                .set("data-x", &x)
                .set("data-y", &y)
                .set("data-hdir", direction)
        }
    }

    let geo = LeafNode.compute_geometry();
    assert_eq!(geo.entry_height, 3);
    assert_eq!(geo.height, 9);
    assert_eq!(geo.width, 15);
    assert!(geo.children.is_empty());

    let drawn = LeafNode
        .draw_with_geometry(7, 11, svg::HDir::RTL, &geo)
        .to_string();
    assert!(drawn.contains("<leaf"));
    assert!(drawn.contains("data-hdir=\"rtl\""));

    let mut rendered = String::new();
    let mut renderer = svg::Renderer::new(&mut rendered);
    LeafNode
        .render(&mut renderer, 7, 11, svg::HDir::RTL)
        .unwrap();
    assert!(rendered.contains("data-x=\"7\""));
    assert!(rendered.contains("data-y=\"11\""));
    assert!(rendered.contains("data-hdir=\"rtl\""));

    let nodes: Vec<Box<dyn Node>> = vec![boxed(Start), boxed(SimpleStart), boxed(End)];
    assert_eq!(nodes.iter().max_entry_height(), 10);
    assert_eq!(nodes.iter().max_height_below_entry(), 10);
    assert_eq!(nodes.iter().max_width(), 20);
    assert_eq!(nodes.iter().total_width(), 55);
    assert_eq!(nodes.iter().total_height(), 50);

    let geo = NodeGeometry {
        entry_height: 4,
        height: 9,
        width: 1,
        children: vec![],
    };
    assert_eq!(geo.height_below_entry(), 5);
}

#[test]
fn built_in_primitives_render_distinct_markup() {
    let svg = Diagram::new(railroad::Sequence::new(vec![
        boxed(Start),
        boxed(SimpleStart),
        boxed(Debug::new(4, 10, 12)),
        boxed(SimpleEnd),
        boxed(End),
        boxed(Empty),
    ]))
    .to_string();

    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("class=\"debug\""));
    assert!(svg.contains("stroke: red"));
    assert!(svg.matches("<path").count() >= 4);
    assert_eq!(Empty.entry_height(), 0);
    assert_eq!(Empty.height(), 0);
    assert_eq!(Empty.width(), 0);
}
