mod common;

use railroad::{Comment, Diagram, LabeledBox, Link, LinkTarget, Node, Optional, Repeat, Terminal};

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
