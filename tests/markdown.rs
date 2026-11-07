//! Raw SVG must survive CommonMark conversion without changing the surrounding prose.
//!
//! Generate the same cases for Nu HTML validation with:
//! `cargo test --no-default-features --test markdown write_html_fixtures -- --ignored`

use pulldown_cmark::{Parser, html};
use railroad::{Diagram, Link, Node, Stylesheet, Terminal, svg};

const STYLESHEETS: [(&str, Stylesheet); 8] = [
    ("light", Stylesheet::Light),
    ("dark", Stylesheet::Dark),
    ("light-safe", Stylesheet::LightRendersafe),
    ("dark-safe", Stylesheet::DarkRendersafe),
    ("rust", Stylesheet::Rust),
    ("coal", Stylesheet::Coal),
    ("navy", Stylesheet::Navy),
    ("ayu", Stylesheet::Ayu),
];

// Implements only the element API, as a downstream node would.
struct CustomText;

impl Node for CustomText {
    fn width(&self) -> i64 {
        100
    }
    fn height(&self) -> i64 {
        20
    }
    fn entry_height(&self) -> i64 {
        10
    }
    fn draw(&self, x: i64, y: i64, _: svg::HDir) -> svg::Element {
        svg::Element::new("text")
            .set("x", &x)
            .set("y", &y)
            .set("data-note", "first\r\n\r\nsecond")
            .text("first\n\n**second**")
    }
}

fn diagrams() -> Vec<(String, Diagram<Box<dyn Node>>)> {
    let terminal = || Box::new(Terminal::new("a < b & **literal**".to_owned())) as Box<dyn Node>;
    let mut diagrams = vec![
        ("unstyled".to_owned(), Diagram::new(terminal())),
        (
            "link".to_owned(),
            Diagram::new(Box::new(Link::new(
                terminal(),
                "https://example.com/?a=1&b=2".to_owned(),
            )) as Box<dyn Node>),
        ),
        (
            "blank-line-label".to_owned(),
            Diagram::new(Box::new(Terminal::new("first\n\n**second**".to_owned())) as Box<dyn Node>),
        ),
        (
            "custom-node".to_owned(),
            Diagram::new(Box::new(CustomText) as Box<dyn Node>),
        ),
    ];
    let mut multiline_attribute = Diagram::new(terminal());
    multiline_attribute
        .attr("data-note".to_owned())
        .or_insert("first\r\n\r\nsecond".to_owned());
    diagrams.push(("multiline-attribute".to_owned(), multiline_attribute));
    let mut custom_css = Diagram::new(terminal());
    custom_css.add_css(
        "/* < & > */\nsvg.railroad > text { fill: red; }\n\nsvg.railroad path { stroke: blue; }\n",
    );
    diagrams.push(("custom-css".to_owned(), custom_css));
    diagrams.extend(STYLESHEETS.map(|(name, style)| {
        (
            name.to_owned(),
            Diagram::new_with_stylesheet(terminal(), &style),
        )
    }));
    diagrams
}

fn renderings(diagram: &Diagram<impl Node>) -> [(&str, String); 3] {
    let mut streamed = String::new();
    diagram
        .render(&mut svg::Renderer::new(&mut streamed), 0, 0, svg::HDir::LTR)
        .unwrap();
    [
        ("display", diagram.to_string()),
        ("streaming", streamed),
        ("element", diagram.draw(0, 0, svg::HDir::LTR).to_string()),
    ]
}

fn markdown_html(svg: &str) -> String {
    let markdown = format!("Before **bold**.\n\n{svg}\n\nAfter *italic*.\n");
    let mut output = String::new();
    html::push_html(&mut output, Parser::new(&markdown));
    output
}

#[test]
fn svg_survives_markdown() {
    let mut failures = Vec::new();
    for (name, diagram) in diagrams() {
        for (backend, svg) in renderings(&diagram) {
            let expected = format!(
                "<p>Before <strong>bold</strong>.</p>\n{}\n<p>After <em>italic</em>.</p>\n",
                svg.trim_end_matches('\n')
            );
            if markdown_html(&svg) != expected {
                failures.push(format!("{name}/{backend}"));
            }
        }
    }
    assert!(failures.is_empty(), "Markdown changed SVG: {failures:?}");
}

#[test]
#[ignore = "writes fixtures for Nu HTML validation"]
fn write_html_fixtures() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/markdown-html");
    std::fs::create_dir_all(&output).unwrap();
    for (name, diagram) in diagrams() {
        for (backend, svg) in renderings(&diagram) {
            let document = format!(
                "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
                 <title>Markdown SVG: {name}/{backend}</title>\n</head>\n<body>\n{}\
                 </body>\n</html>\n",
                markdown_html(&svg)
            );
            std::fs::write(output.join(format!("{name}-{backend}.html")), document).unwrap();
        }
    }
}
