mod common;

use railroad::{
    Annotation, Comment, Diagram, LabeledBox, Link, LinkTarget, Node, Optional, Repeat, Terminal,
    svg,
};

#[test]
fn link_wraps_inner_node_and_emits_target_attributes() {
    let mut link = Link::new(
        Terminal::new("docs".to_owned()),
        "https://example.com/?a=1&b=2".to_owned(),
    );
    link.set_target(Some(LinkTarget::Parent));
    link.attr("data-kind".to_owned())
        .or_insert("external".to_owned());

    let svg = Diagram::new(link).to_string();

    assert!(svg.contains("<a "));
    assert!(svg.contains("xlink:href=\"https://example.com/?a=1&amp;b=2\""));
    assert!(svg.contains("target=\"_parent\""));
    assert!(svg.contains("class=\"link\""));
    assert!(svg.contains("data-kind=\"external\""));
    assert!(svg.contains("docs"));
}

#[test]
fn optional_repeat_and_labeled_box_render_distinct_structures() {
    let optional_svg = Diagram::new(Optional::new(Terminal::new("maybe".to_owned()))).to_string();
    assert!(optional_svg.contains("class=\"optional\""));
    assert!(optional_svg.contains("maybe"));

    let repeat_svg = Diagram::new(Repeat::new(
        Terminal::new("item".to_owned()),
        Comment::new(",".to_owned()),
    ))
    .to_string();
    assert!(repeat_svg.contains("class=\"repeat\""));
    assert!(repeat_svg.contains("item"));
    assert!(repeat_svg.contains(","));

    let labeled_svg = Diagram::new(LabeledBox::new(
        Terminal::new("body".to_owned()),
        Comment::new("group".to_owned()),
    ))
    .to_string();
    assert!(labeled_svg.contains("class=\"labeledbox\""));
    assert!(labeled_svg.contains("body"));
    assert!(labeled_svg.contains("group"));
    assert!(labeled_svg.contains("<rect"));
}

#[test]
fn labeled_box_without_label_avoids_reserved_label_space() {
    let unlabeled = LabeledBox::without_label(Terminal::new("body".to_owned()));
    let labeled = LabeledBox::new(
        Terminal::new("body".to_owned()),
        Comment::new("group".to_owned()),
    );

    assert!(unlabeled.height() < labeled.height());
}

#[test]
fn annotation_renders_the_supplied_box_and_label() {
    let node = Annotation::new(LabeledBox::new(
        Terminal::new("body".to_owned()),
        Comment::new("Custom label".to_owned()),
    ));
    let svg = node.draw(0, 0, svg::HDir::LTR).to_string();

    assert!(svg.contains("class=\"annotation-marker\""));
    assert!(svg.contains("class=\"annotation-connector\""));
    assert!(svg.contains("class=\"labeledbox\""));
    assert!(svg.contains("body") && svg.contains("Custom label"));
    assert!(svg.contains("class=\"annotation-symbol\""));
    assert!(!svg.contains("annotation-direction"));
}

#[test]
fn annotation_arrows_follow_the_local_reading_direction() {
    for (ahead, dir, curve) in [
        (true, svg::HDir::LTR, " c 2 "),
        (true, svg::HDir::RTL, " c -2 "),
        (false, svg::HDir::LTR, " c -2 "),
        (false, svg::HDir::RTL, " c 2 "),
    ] {
        let body = LabeledBox::without_label(Terminal::new("body".to_owned()));
        let node = if ahead {
            Annotation::new_ahead(body)
        } else {
            Annotation::new_behind(body)
        };
        let mut svg = String::new();
        node.render(&mut svg::Renderer::new(&mut svg), 0, 0, dir)
            .unwrap();

        assert!(svg.contains("class=\"annotation-direction\""));
        assert!(!svg.contains("class=\"annotation-symbol\""));
        assert!(svg.contains(curve));
    }
}
