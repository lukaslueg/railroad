//! Construct some diagrams and verify them according to W3C's DTD.
//! This does not ensure that we *always* generate SVGs for all knobs and
//! switches. At least every primitive should appear here once.
//!
//! DTD checks use `xmllint` from libxml2 and are ignored by default. Attribute
//! regression tests check the output directly and run without external tools.

use std::sync::OnceLock;

use railroad::Node;

fn init_verifier() -> &'static railroad_verification::Verifier {
    static VERIFIER: OnceLock<railroad_verification::Verifier> = OnceLock::new();
    VERIFIER.get_or_init(|| railroad_verification::Verifier::new().unwrap())
}

macro_rules! verify {
    ($testname:ident, $src:expr) => {
        #[test]
        #[ignore]
        fn $testname() {
            init_verifier().verify($src).unwrap();
        }
    };
}

macro_rules! raw_dia {
    ($r:expr) => {
        railroad::Diagram::with_default_css($r).to_string()
    };
}
macro_rules! dia {
    ($r:expr) => {
        raw_dia!(seq!(railroad::SimpleStart, $r, railroad::SimpleEnd))
    };
}
macro_rules! nonterm {
    ($r:expr) => {
        railroad::NonTerminal::new($r.to_owned())
    };
}
macro_rules! term {
    ($r:expr) => {
        railroad::Terminal::new($r.to_owned())
    };
}
macro_rules! seq { ($($r: expr),*) => { railroad::Sequence::new(vec![ $( Box::new($r) as Box<dyn Node>, )+ ]) } }
macro_rules! choice { ($($r: expr),*) => { railroad::Choice::new(vec![ $( Box::new($r) as Box<dyn Node>, )* ]) } }
macro_rules! stck { ($($r: expr),*) => { railroad::Stack::new(vec![ $( Box::new($r) as Box<dyn Node>, )* ]) } }
macro_rules! cmt {
    ($r:expr) => {
        railroad::Comment::new($r.to_owned())
    };
}
macro_rules! vert { ($($r: expr),*) => { railroad::VerticalGrid::new(vec![ $( Box::new($r) as Box<dyn Node>, )+ ]) } }
macro_rules! horiz { ($($r: expr),*) => { railroad::HorizontalGrid::new(vec![ $( Box::new($r) as Box<dyn Node>, )+ ]) } }
macro_rules! rpt {
    ($r:expr, $s:expr) => {
        railroad::Repeat::new($r, $s)
    };
    ($r:expr) => {
        rpt!($r, railroad::Empty)
    };
}
macro_rules! opt {
    ($r:expr) => {
        railroad::Optional::new($r)
    };
}
macro_rules! lbox {
    ($r:expr, $u:expr) => {
        railroad::LabeledBox::new($r, $u)
    };
    ($r:expr) => {
        railroad::LabeledBox::new($r, railroad::Empty)
    };
}
macro_rules! lnk {
    ($r:expr) => {
        railroad::Link::new($r, "https://www.google.com".to_owned())
    };
}

verify!(simple_nonterm, dia!(nonterm!("Foobar")));
verify!(
    continuation_markers,
    raw_dia!(seq!(
        railroad::ContinuationStart,
        nonterm!("expr"),
        railroad::Continuation,
        nonterm!("tail"),
        railroad::ContinuationEnd
    ))
);
verify!(escape_nonterm, dia!(nonterm!("Foo<bar>")));
verify!(simple_term, dia!(term!("Foobar")));
verify!(
    scaled_choice,
    dia!(railroad::Scale::new(
        choice!(term!("Foo"), term!("Bar")),
        0.65
    ))
);
verify!(
    precise_scale,
    raw_dia!(railroad::Scale::new_precise(
        railroad::Annotation::new(railroad::LabeledBox::without_label(term!("Foo"))),
        0.65
    ))
);
verify!(escape_term, dia!(term!("Foo<bar>")));
verify!(simple_choice, dia!(choice!(term!("Foo"), term!("Bar"))));
verify!(
    simple_multichoice,
    dia!(railroad::MultiChoice::new(vec![
        vec![
            Box::new(term!("Foo")) as Box<dyn Node>,
            Box::new(term!("Bar")) as Box<dyn Node>,
        ],
        vec![Box::new(term!("Baz")) as Box<dyn Node>],
    ]))
);
verify!(simple_stack, dia!(stck!(term!("Foo"), term!("Bar"))));
verify!(simple_comment, dia!(cmt!("Foobar")));
verify!(
    simple_alignment,
    dia!(railroad::Alignment::new(
        term!("Foobar"),
        150,
        60,
        railroad::HorizontalAlignment::End,
        railroad::VerticalAlignment::Bottom,
        true,
    ))
);
verify!(escape_comment, dia!(cmt!("Foo<bar>")));
verify!(simple_vertical, dia!(vert!(term!("Foo"), term!("Bar"))));
verify!(simple_horizontal, dia!(horiz!(term!("Foo"), term!("Bar"))));
verify!(simple_repeat, dia!(rpt!(term!("Foo"))));
verify!(simple_opt, dia!(opt!(term!("Foo"))));
verify!(
    negative_lookahead,
    dia!(ahead(choice!(term!("\""), term!("\\"), nonterm!("CR"))))
);
verify!(
    negative_lookbehind,
    dia!(behind(seq!(term!("a"), term!("b"))))
);
verify!(simple_lbox, dia!(lbox!(term!("Foo"))));
verify!(simple_link, dia!(lnk!(term!("Foo"))));
verify!(
    blank_link,
    dia!({
        let mut l = lnk!(term!("Foo"));
        l.set_target(Some(railroad::LinkTarget::Blank));
        l
    })
);

fn renderings(node: &impl Node) -> [String; 3] {
    let direction = railroad::svg::HDir::LTR;
    let mut streamed = String::new();
    node.render(
        &mut railroad::svg::Renderer::new(&mut streamed),
        0,
        0,
        direction,
    )
    .unwrap();
    [
        streamed,
        node.draw(0, 0, direction).to_string(),
        node.draw_with_geometry(0, 0, direction, &node.compute_geometry())
            .to_string(),
    ]
}

#[test]
fn diagram_owned_attributes_take_precedence() {
    let mut diagram = railroad::Diagram::new(term!("one"));
    for key in ["xmlns", "xmlns:xlink", "class", "viewBox"] {
        diagram
            .attr(key.to_owned())
            .or_insert("overridden".to_owned());
    }
    #[cfg(feature = "visual-debug")]
    diagram
        .attr("xmlns:railroad".to_owned())
        .or_insert("overridden".to_owned());
    diagram.attr("id".to_owned()).or_insert("kept".to_owned());
    for svg in renderings(&diagram) {
        assert!(!svg.contains("overridden"));
        assert!(svg.contains("id=\"kept\""));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("xmlns:xlink=\"http://www.w3.org/1999/xlink\""));
        assert!(svg.contains("class=\"railroad\""));
        assert!(svg.contains(&format!(
            "viewBox=\"0 0 {} {}\"",
            diagram.width(),
            diagram.height()
        )));
        #[cfg(feature = "visual-debug")]
        assert!(svg.contains("xmlns:railroad=\"http://www.github.com/lukaslueg/railroad\""));
    }
}

#[test]
fn link_owned_attributes_take_precedence() {
    let mut link = lnk!(term!("one"));
    link.attr("xlink:href".to_owned())
        .or_insert("overridden".to_owned());
    link.attr("target".to_owned()).or_insert("_top".to_owned());
    for (target, expected) in [
        (Some(railroad::LinkTarget::Parent), "_parent"),
        (None, "_top"),
    ] {
        link.set_target(target);
        for svg in renderings(&link) {
            assert!(!svg.contains("overridden"));
            assert!(svg.contains("xlink:href=\"https://www.google.com\""));
            assert!(svg.contains(&format!("target=\"{expected}\"")));
            assert_eq!(svg.matches(" target=").count(), 1);
        }
    }
}

#[test]
fn comment_owned_coordinates_take_precedence() {
    let mut comment = cmt!("one");
    for key in ["x", "y"] {
        comment
            .attr(key.to_owned())
            .or_insert("overridden".to_owned());
    }
    for svg in renderings(&comment) {
        assert!(!svg.contains("overridden"));
        assert!(svg.contains(&format!(" x=\"{}\"", comment.width() / 2)));
        assert!(svg.contains(" y=\"15\""));
    }
}

#[test]
fn node_owned_classes_and_debug_attributes_take_precedence() {
    macro_rules! check {
        ($node:expr, $class:literal) => {{
            let mut node = $node;
            let class = node
                .attr("class".to_owned())
                .or_insert("overridden".to_owned());
            assert_eq!(class.as_str(), "overridden");
            #[cfg(feature = "visual-debug")]
            for key in [
                "railroad:type",
                "railroad:x",
                "railroad:y",
                "railroad:entry_height",
                "railroad:height",
                "railroad:width",
            ] {
                node.attr(key.to_owned()).or_insert("overridden".to_owned());
            }
            for svg in renderings(&node) {
                let tag = svg.lines().next().unwrap();
                assert!(tag.contains(concat!("class=\"", $class, "\"")));
                assert_eq!(tag.matches(" class=").count(), 1);
                assert!(!svg.contains("overridden"));
                #[cfg(feature = "visual-debug")]
                {
                    assert!(tag.contains("railroad:type=\""));
                    for (key, value) in [
                        ("x", 0),
                        ("y", 0),
                        ("entry_height", node.entry_height()),
                        ("height", node.height()),
                        ("width", node.width()),
                    ] {
                        assert!(tag.contains(&format!("railroad:{key}=\"{value}\"")));
                    }
                }
            }
        }};
    }
    check!(term!("one"), "terminal");
    check!(nonterm!("one"), "nonterminal");
    check!(cmt!("one"), "comment");
    check!(stck!(term!("one")), "stack");
    check!(choice!(term!("one")), "choice");
    check!(
        railroad::MultiChoice::new(vec![vec![term!("one")]]),
        "multichoice"
    );
    check!(vert!(term!("one")), "verticalgrid");
    check!(horiz!(term!("one")), "horizontalgrid");
    check!(opt!(term!("one")), "optional");
    check!(rpt!(term!("one")), "repeat");
    check!(lbox!(term!("one")), "labeledbox");
    check!(lnk!(term!("one")), "link");
    check!(railroad::Scale::new(term!("one"), 0.5), "scale");
    check!(
        railroad::Alignment::new(
            term!("one"),
            100,
            50,
            railroad::HorizontalAlignment::Start,
            railroad::VerticalAlignment::Top,
            true,
        ),
        "alignment"
    );
    check!(ahead(term!("one")), "annotation");
    check!(railroad::Stack::<railroad::Empty>::default(), "stack");
    check!(railroad::Choice::<railroad::Empty>::default(), "choice");
    check!(
        railroad::MultiChoice::<railroad::Empty>::default(),
        "multichoice"
    );
    check!(
        railroad::VerticalGrid::<railroad::Empty>::default(),
        "verticalgrid"
    );
    check!(
        railroad::HorizontalGrid::<railroad::Empty>::default(),
        "horizontalgrid"
    );
    check!(railroad::Optional::<railroad::Empty>::default(), "optional");
    check!(
        railroad::Repeat::<railroad::Empty, railroad::Empty>::default(),
        "repeat"
    );
    check!(
        railroad::LabeledBox::<railroad::Empty, railroad::Empty>::default(),
        "labeledbox"
    );
}

fn ahead<N>(inner: N) -> railroad::Annotation<railroad::LabeledBox<N, railroad::Comment>> {
    railroad::Annotation::new_ahead(railroad::LabeledBox::new(
        inner,
        railroad::Comment::new("Must not match ahead; consumes no input".to_owned()),
    ))
}

fn behind<N>(inner: N) -> railroad::Annotation<railroad::LabeledBox<N, railroad::Comment>> {
    railroad::Annotation::new_behind(railroad::LabeledBox::new(
        inner,
        railroad::Comment::new("Must not match behind; consumes no input".to_owned()),
    ))
}
