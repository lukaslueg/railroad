//! Generate the visual vocabulary in README.md and its SVG illustrations.
//!
//! Run `cargo run --no-default-features --example readme`, or append `-- --check`
//! to verify that the checked-in documentation is current without writing files.

use std::{fmt::Write as _, fs, io, path::Path};

use railroad::*;

const START: &str = "<!-- BEGIN GENERATED VISUAL VOCABULARY -->";
const END: &str = "<!-- END GENERATED VISUAL VOCABULARY -->";
const SOURCE: &str = include_str!("readme.rs");

/// Keep the displayed Rust expression identical to the one compiled below.
fn snippet(slug: &str) -> &str {
    let marker = format!("// snippet: {slug}\n");
    SOURCE
        .split_once(&marker)
        .expect("example must have a snippet marker")
        .1
        .split_once("// end-snippet")
        .expect("example must have an end marker")
        .0
        .trim_end()
        .trim_end_matches(',')
}

struct Vocabulary {
    markdown: String,
    images: Vec<(String, String)>,
}

impl Vocabulary {
    fn section(&mut self, title: &str, description: &str) {
        writeln!(self.markdown, "---\n\n**{title}**\n\n{description}\n").unwrap();
    }

    fn entry(&mut self, slug: &str, title: &str, description: &str, node: impl Node + 'static) {
        self.bare_entry(slug, title, description, with_endpoints(node));
    }

    fn bare_entry(&mut self, slug: &str, title: &str, description: &str, node: impl Node) {
        let mut diagram = Diagram::new_with_stylesheet(node, &Stylesheet::Light);
        // Explicit dimensions preserve a consistent scale when images are embedded.
        let width = diagram.width().to_string();
        let height = diagram.height().to_string();
        diagram.attr("width".to_owned()).or_insert(width);
        diagram.attr("height".to_owned()).or_insert(height);
        diagram.add_element(svg::Element::new("title").text(description));
        self.images
            .push((format!("{slug}.svg"), format!("{diagram}\n")));

        writeln!(
            self.markdown,
            "### {title}\n\n{description}\n\n![{description}](examples/vocabulary/{slug}.svg)\n\n<details>\n<summary>Rust</summary>\n\n```rust\nuse railroad::*;\n"
        )
        .unwrap();
        let source = snippet(slug);
        let indent = source
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        let mut lines = source.lines();
        write!(
            self.markdown,
            "let node = {}",
            &lines.next().unwrap()[indent..]
        )
        .unwrap();
        for line in lines {
            write!(
                self.markdown,
                "\n{}",
                if line.trim().is_empty() {
                    ""
                } else {
                    &line[indent..]
                }
            )
            .unwrap();
        }
        writeln!(self.markdown, ";\n```\n\n</details>\n").unwrap();
    }
}

fn with_endpoints(node: impl Node + 'static) -> Sequence<Box<dyn Node>> {
    Sequence::new(vec![
        Box::new(SimpleStart),
        Box::new(node),
        Box::new(SimpleEnd),
    ])
}

fn vocabulary() -> Vocabulary {
    let mut guide = Vocabulary {
        markdown: String::from(
            "## Visual vocabulary\n\nFollow a path from start to end. Read literal tokens along the path, expand named rules, and follow the arrows around branches and loops.\n\nThe examples below use `SimpleStart` and `SimpleEnd` around each connected pattern. Expand **Rust** to see the node inside those markers; grids and the marker examples show their complete node trees.\n\n",
        ),
        images: Vec::new(),
    };

    guide.section(
        "Reading the rails",
        "These patterns describe what a grammar accepts.",
    );
    guide.entry(
        "terminal",
        "Literal token — `Terminal`",
        "Consume exactly the token `if`.",
        // snippet: terminal
        Terminal::new("if".to_owned()), // end-snippet
    );
    guide.entry(
        "nonterminal",
        "Another rule — `NonTerminal`",
        "Expand the named rule `expr`.",
        // snippet: nonterminal
        NonTerminal::new("expr".to_owned()), // end-snippet
    );
    guide.entry(
        "sequence",
        "In order — `Sequence`",
        "Consume `(`, an expression, and `)` in order.",
        // snippet: sequence
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(Terminal::new("(".to_owned())),
            Box::new(NonTerminal::new("expr".to_owned())),
            Box::new(Terminal::new(")".to_owned())),
        ]), // end-snippet
    );
    guide.entry(
        "choice",
        "Choose one — `Choice`",
        "Choose exactly one alternative: `true` or `false`.",
        // snippet: choice
        Choice::new(vec![
            Terminal::new("true".to_owned()),
            Terminal::new("false".to_owned()),
        ]), // end-snippet
    );
    guide.entry(
        "optional",
        "Take it or skip it — `Optional`",
        "Consume `else`, or take the upper bypass and consume nothing.",
        // snippet: optional
        Optional::new(Terminal::new("else".to_owned())), // end-snippet
    );
    guide.entry(
        "one-or-more",
        "One or more — `Repeat`",
        "Consume one or more items using the lower return path.",
        // snippet: one-or-more
        Repeat::new(NonTerminal::new("item".to_owned()), Empty), // end-snippet
    );
    guide.entry(
        "zero-or-more",
        "Zero or more — `Optional` + `Repeat`",
        "Skip all items via the upper bypass, or consume one or more via the lower loop.",
        // snippet: zero-or-more
        Optional::new(Repeat::new(NonTerminal::new("item".to_owned()), Empty)), // end-snippet
    );
    guide.entry(
        "separated-list",
        "Separated list — `Repeat`",
        "Consume one or more comma-separated items, without a leading or trailing comma.",
        // snippet: separated-list
        Repeat::new(
            NonTerminal::new("item".to_owned()),
            Terminal::new(",".to_owned()),
        ), // end-snippet
    );

    guide.section(
        "Annotations and assertion recipes",
        "Annotations connect a checkpoint to a detached LabeledBox. Wavy arrows refer ahead or behind in the local reading direction. Labels and grammar meaning are supplied by the author; the following assertions are recipes.",
    );
    guide.entry(
        "annotation",
        "Attach a note — `Annotation`",
        "Attach a caller-supplied label and body to a plain checkpoint. This annotation supplies no automatic grammar wording.",
        // snippet: annotation
        Annotation::new(LabeledBox::new(
            NonTerminal::new("statement".to_owned()),
            Comment::new("Only at top level".to_owned()),
        )), // end-snippet
    );
    guide.entry(
        "negative-lookahead",
        "Negative lookahead recipe — `Annotation::new_ahead`",
        "Consume one ASCII character if the upcoming input does not start with a quote, backslash, or CR. The assertion itself consumes no input.",
        // snippet: negative-lookahead
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(Annotation::new_ahead(LabeledBox::new(
                Sequence::<Box<dyn Node>>::new(vec![
                    Box::new(Choice::<Box<dyn Node>>::new(vec![
                        Box::new(Terminal::new("\"".to_owned())),
                        Box::new(Terminal::new("\\".to_owned())),
                        Box::new(NonTerminal::new("CR".to_owned())),
                    ])),
                    Box::new(Continuation)]),
                Comment::new("Must not match ahead".to_owned()),
            ))),
            Box::new(NonTerminal::new("ASCII".to_owned())),
        ]), // end-snippet
    );
    guide.entry(
        "negative-lookbehind",
        "Negative lookbehind recipe — `Annotation::new_behind`",
        "Match a decimal integer, then reject exactly `0`.",
        // snippet: negative-lookbehind
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(NonTerminal::new("DECIMAL".to_owned())),
            Box::new(Annotation::new_behind(LabeledBox::new(
                Sequence::<Box<dyn Node>>::new(vec![
                    Box::new(Continuation),
                    Box::new(Terminal::new("0".to_owned())),
                ]),
                Comment::new("Must not match behind".to_owned()),
            ))),
        ]), // end-snippet
    );

    guide.section(
        "Arranging the diagram",
        "Stacks and choices connect their children; grids arrange independent diagrams.",
    );
    guide.entry(
        "stack",
        "Continue on another row — `Stack`",
        "Read the same `(`, expression, `)` sequence across connected rows.",
        // snippet: stack
        Stack::<Box<dyn Node>>::new(vec![
            Box::new(Terminal::new("(".to_owned())),
            Box::new(NonTerminal::new("expr".to_owned())),
            Box::new(Terminal::new(")".to_owned())),
        ]), // end-snippet
    );
    guide.entry(
        "multi-choice", "Spread alternatives across columns — `MultiChoice`", "Choose one of four alternatives spread across two columns: `true`, `false`, `null`, or `undefined`.",
        // snippet: multi-choice
        MultiChoice::new(vec![
            vec![
                Terminal::new("true".to_owned()),
                Terminal::new("false".to_owned()),
            ],
            vec![
                Terminal::new("null".to_owned()),
                Terminal::new("undefined".to_owned()),
            ],
        ])
        // end-snippet
    );
    guide.bare_entry(
        "vertical-grid",
        "Independent rows — `VerticalGrid`",
        "Place independent diagrams above one another, with no connecting path.",
        // snippet: vertical-grid
        VerticalGrid::new(vec![
            Sequence::<Box<dyn Node>>::new(vec![
                Box::new(SimpleStart),
                Box::new(Terminal::new("yes".to_owned())),
                Box::new(SimpleEnd),
            ]),
            Sequence::<Box<dyn Node>>::new(vec![
                Box::new(SimpleStart),
                Box::new(Terminal::new("no".to_owned())),
                Box::new(SimpleEnd),
            ]),
        ]), // end-snippet
    );
    guide.bare_entry(
        "horizontal-grid",
        "Independent columns — `HorizontalGrid`",
        "Place the same independent diagrams beside one another.",
        // snippet: horizontal-grid
        HorizontalGrid::new(vec![
            Sequence::<Box<dyn Node>>::new(vec![
                Box::new(SimpleStart),
                Box::new(Terminal::new("yes".to_owned())),
                Box::new(SimpleEnd),
            ]),
            Sequence::<Box<dyn Node>>::new(vec![
                Box::new(SimpleStart),
                Box::new(Terminal::new("no".to_owned())),
                Box::new(SimpleEnd),
            ]),
        ]), // end-snippet
    );

    guide.section(
        "Explaining and navigating",
        "Annotations and links help readers interpret a diagram without adding grammar tokens.",
    );
    guide.entry(
        "comment",
        "Add an annotation — `Comment`",
        "Show explanatory text along the path, without consuming a token.",
        // snippet: comment
        Comment::new("an expression follows".to_owned()), // end-snippet
    );
    guide.entry(
        "labeled-box",
        "Label a group — `LabeledBox`",
        "Explain a group with a labeled box around its node.",
        // snippet: labeled-box
        LabeledBox::new(
            NonTerminal::new("expr".to_owned()),
            Comment::new("expression".to_owned()),
        ), // end-snippet
    );
    guide.entry(
        "link",
        "Make a node clickable — `Link`",
        "Link a node to documentation. Open the SVG directly or embed it inline to use the link.",
        // snippet: link
        Link::new(
            NonTerminal::new("expr".to_owned()),
            "https://docs.rs/railroad/latest/railroad/struct.NonTerminal.html".to_owned(),
        ), // end-snippet
    );
    guide.bare_entry(
        "start-end",
        "Mark the boundaries — `Start` and `End`",
        "Vertical bars mark the beginning and end of a complete diagram.",
        // snippet: start-end
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(Start),
            Box::new(NonTerminal::new("expr".to_owned())),
            Box::new(End),
        ]), // end-snippet
    );
    guide.bare_entry(
        "simple-start-end",
        "Compact boundaries — `SimpleStart` and `SimpleEnd`",
        "Circles provide compact start and end markers.",
        // snippet: simple-start-end
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(SimpleStart),
            Box::new(NonTerminal::new("expr".to_owned())),
            Box::new(SimpleEnd),
        ]), // end-snippet
    );
    guide.bare_entry(
        "continuation-start-end",
        "Show a fragment — `ContinuationStart` and `ContinuationEnd`",
        "Three dots mark omitted portions before and after the fragment.",
        // snippet: continuation-start-end
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(ContinuationStart),
            Box::new(NonTerminal::new("expr".to_owned())),
            Box::new(ContinuationEnd),
        ]), // end-snippet
    );
    guide.entry(
        "continuation",
        "Omit a section — `Continuation`",
        "An ellipsis joins the displayed parts of a diagram around a section that is not shown here.",
        // snippet: continuation
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(Terminal::new("BEGIN".to_owned())),
            Box::new(Continuation),
            Box::new(Terminal::new("END".to_owned())),
        ]), // end-snippet
    );
    guide.entry(
        "empty",
        "Draw nothing — `Empty`",
        "Use `Empty` where a node is required but no input is consumed. Here it fills the inner node of the labeled first alternative.",
        // snippet: empty
        Choice::<Box<dyn Node>>::new(vec![
            Box::new(LabeledBox::new(
                Empty,
                Comment::new("Default".to_owned()),
            )),
            Box::new(Terminal::new("ASC".to_owned())),
            Box::new(Terminal::new("DESC".to_owned())),
        ])
        // end-snippet
    );
    guide.markdown.push_str(
        "<!--\nGenerated by examples/readme.rs.\nRegenerate: cargo run --no-default-features --example readme\nCheck: cargo run --no-default-features --example readme -- --check\n-->\n",
    );
    guide
}

fn update(path: &Path, content: &str, check: bool) -> io::Result<bool> {
    match fs::read_to_string(path) {
        Ok(current) if current == content => return Ok(false),
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    if check {
        eprintln!("Stale or missing generated file: {}", path.display());
    } else {
        fs::write(path, content)?;
        println!("Updated {}", path.display());
    }
    Ok(true)
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let check = match args.as_slice() {
        [] => false,
        [arg] if arg == "--check" => true,
        _ => {
            return Err(io::Error::other(
                "Usage: cargo run --no-default-features --example readme -- [--check]",
            ));
        }
    };
    if cfg!(feature = "visual-debug") {
        return Err(io::Error::other(
            "Regenerate documentation without the visual-debug feature.",
        ));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = root.join("README.md");
    let readme = fs::read_to_string(&readme_path)?;
    if readme.matches(START).count() != 1 || readme.matches(END).count() != 1 {
        return Err(io::Error::other(
            "README.md must contain exactly one pair of visual vocabulary markers.",
        ));
    }
    let (before, rest) = readme.split_once(START).unwrap();
    let (_, after) = rest
        .split_once(END)
        .ok_or_else(|| io::Error::other("Visual vocabulary markers are out of order."))?;
    let guide = vocabulary();
    let generated = format!("{before}{START}\n\n{}{END}{after}", guide.markdown);
    let image_dir = root.join("examples/vocabulary");
    if !check {
        fs::create_dir_all(&image_dir)?;
    }
    let mut changed = false;
    for (name, image) in guide.images {
        changed |= update(&image_dir.join(name), &image, check)?;
    }
    changed |= update(&readme_path, &generated, check)?;
    if check && changed {
        return Err(io::Error::other(
            "Run cargo run --no-default-features --example readme to regenerate documentation.",
        ));
    }
    Ok(())
}
