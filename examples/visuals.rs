use std::fs;
use std::io::Write;

fn main() {
    use railroad::*;
    use railroad::{HorizontalAlignment as HAlign, VerticalAlignment as VAlign};

    let mut f = fs::File::create("examples/visuals.html").unwrap();

    macro_rules! hr {
        () => {
            f.write_all(b"<hr>").unwrap();
        };
    }

    macro_rules! raw_dia {
        ($r:expr) => {
            let dia = Diagram::with_default_css($r);
            writeln!(f, "<div style=\"width: {}px; height: auto; max-width: 100%; max-height: 100%\">{}</div>", dia.width(), dia).unwrap();
        };
    }

    macro_rules! dia {
        ($r:expr) => {
            raw_dia!(seq!(SimpleStart, $r, SimpleEnd));
        };
    }

    macro_rules! nonterm {
        ($r:expr) => {
            NonTerminal::new($r.to_owned())
        };
    }
    macro_rules! term {
        ($r:expr) => {
            Terminal::new($r.to_owned())
        };
    }
    macro_rules! seq { ($($r: expr),*) => { Sequence::<Box<dyn Node>>::new(vec![ $( Box::new($r), )+ ]) } }
    macro_rules! choice { ($($r: expr),*) => { Choice::<Box<dyn Node>>::new(vec![ $( Box::new($r), )* ]) } }
    macro_rules! multichoice {
        ( $( [ $($r:expr),* $(,)? ] ),* $(,)? ) => {
            MultiChoice::<Box<dyn Node>>::new(vec![
                $( vec![ $( Box::new($r) as Box<dyn Node>, )* ], )*
            ])
        }
    }
    macro_rules! stck { ($($r: expr),*) => { Stack::<Box<dyn Node>>::new(vec![ $( Box::new($r), )* ]) } }
    macro_rules! cmt {
        ($r:expr) => {
            Comment::new($r.to_owned())
        };
    }
    macro_rules! vert { ($($r: expr),*) => { VerticalGrid::new(vec![ $( Box::new($r) as Box<dyn Node>, )+ ]) } }
    macro_rules! horiz { ($($r: expr),*) => { HorizontalGrid::new(vec![ $( Box::new($r) as Box<dyn Node>, )+ ]) } }
    macro_rules! lnk {
        ($r:expr) => {
            Link::new($r, "https://www.rust-lang.org".to_owned())
        };
    }
    macro_rules! rpt {
        ($r:expr, $s:expr) => {
            Repeat::new($r, $s)
        };
        ($r:expr) => {
            rpt!($r, Empty)
        };
    }
    macro_rules! dbg {
        ($eh:expr, $h:expr, $w:expr) => {
            Debug::new($eh, $h, $w)
        };
        () => {
            dbg!(20, 30, 50)
        };
    }

    macro_rules! opt {
        ($r:expr) => {
            Optional::new($r)
        };
    }

    macro_rules! lbox {
        ($r:expr, $u:expr) => {
            LabeledBox::new($r, $u)
        };
        ($r:expr) => {
            LabeledBox::new($r, Empty)
        };
    }

    f.write_all(b"<!DOCTYPE html><html>").unwrap();
    f.write_all(
        br#"
<head>
    <meta charset="utf-8">
    <title>Railroad diagram examples</title>
    <style type="text/css">
        svg.railroad {
            border: 1px solid;
            margin: 10px;
        }
    </style>
</head>"#,
    )
    .unwrap();

    // Very simple
    dia!(nonterm!("Foo"));
    dia!(lbox!(nonterm!("Foo")));
    dia!(lbox!(nonterm!("Foo"), cmt!("Read the docs regarding foo!")));
    hr!();

    // Assertions, including sequences, nesting, and RTL rendering on a return rail.
    dia!(seq!(
        ahead(choice!(term!("\""), term!("\\"), nonterm!("CR"))),
        nonterm!("ASCII")
    ));
    dia!(opt!(rpt!(seq!(
        ahead(seq!(term!("\""), term!("##"))),
        seq!(ahead(nonterm!("CR")), nonterm!("ASCII"))
    ))));
    dia!(seq!(
        nonterm!("DECIMAL"),
        behind(seq!(nonterm!("START_OF_INPUT"), term!("0")))
    ));
    dia!(rpt!(
        nonterm!("item"),
        seq!(ahead(term!("end")), term!(","))
    ));
    hr!();

    // Very simple, varying size
    dia!(dbg!());
    dia!(dbg!(20, 50, 50));
    hr!();

    f.write_all(b"<section id=\"scale\"><h2>Scale corner cases</h2>")
        .unwrap();
    // Half-unit target width and entry height: the precise version deliberately
    // leaves its connections fractional; the adjusted version aligns them.
    f.write_all(b"<p>Width 45, entry height 11, scale 0.5: adjusted above, precise below.</p>")
        .unwrap();
    dia!(choice!(
        Scale::new(dbg!(11, 22, 45), 0.5),
        Scale::new_precise(dbg!(11, 22, 45), 0.5)
    ));

    // Tiny positive widths clamp to one unit; zero-width children keep their
    // requested factor, including when they still have a nonzero height.
    f.write_all(b"<p>Tiny target width, empty child, and zero width with nonzero height.</p>")
        .unwrap();
    raw_dia!(horiz!(
        lbox!(
            Scale::new(dbg!(11, 22, 45), 0.0001),
            cmt!("Clamped to one unit")
        ),
        lbox!(Scale::new(Empty, 0.5), cmt!("Empty stays empty")),
        lbox!(
            Scale::new(dbg!(11, 22, 0), 0.5),
            cmt!("Zero width, nonzero height")
        )
    ));

    // This ratio must not gain a whole unit of padding from floating-point noise.
    f.write_all(b"<p>Width and entry height 25, scale 0.28: both become exactly 7.</p>")
        .unwrap();
    dia!(Scale::new(dbg!(25, 50, 25), 0.28));

    // Nested scales and repetitions exercise both reading directions and an
    // empty alternative without changing the children's natural geometry.
    f.write_all(b"<p>Nested scales on a return path, with an empty forward alternative.</p>")
        .unwrap();
    dia!(rpt!(
        Scale::new(choice!(term!("item"), Empty), 1.25),
        Scale::new(
            Scale::new(rpt!(term!("separator"), cmt!("repeat separator")), 0.65),
            1.25
        )
    ));

    // Scale a complete grammar while also scaling individual tokens, optional
    // groups, labels, and the separator on a right-to-left repetition rail.
    f.write_all(b"<p>ALTER TABLE stack: scale the whole subtree by 0.85, with child factors ranging from 0.6 to 1.8.</p>")
        .unwrap();
    dia!(Scale::new(
        stck!(
            seq!(
                Scale::new(term!("ALTER"), 1.7),
                Scale::new(term!("TABLE"), 0.85),
                Scale::new(opt!(seq!(nonterm!("schema-name"), term!("."))), 0.6),
                Scale::new(nonterm!("table-name"), 1.2)
            ),
            lbox!(
                choice!(
                    seq!(
                        Scale::new(term!("RENAME"), 0.9),
                        Scale::new(term!("TO"), 1.4),
                        Scale::new(nonterm!("new-table-name"), 0.75)
                    ),
                    seq!(
                        Scale::new(term!("ADD"), 1.15),
                        Scale::new(opt!(term!("COLUMN")), 0.8),
                        Scale::new(nonterm!("column-def"), 1.6)
                    ),
                    seq!(
                        Scale::new(term!("DROP"), 0.85),
                        Scale::new(opt!(seq!(term!("IF"), term!("EXISTS"))), 1.1),
                        rpt!(
                            Scale::new(nonterm!("column-name"), 0.9),
                            Scale::new(term!(","), 1.8)
                        )
                    )
                ),
                Scale::new(cmt!("Choose an alteration"), 1.05)
            )
        ),
        0.85
    ));
    f.write_all(b"</section>").unwrap();
    hr!();

    // Comments name their placement inside the surrounding frame.
    f.write_all(
        b"<h2>Alignment</h2><p>Each comment names its placement within its surrounding box.</p><p>Choice: start/top, centered, and end/bottom in equal-size frames.</p>",
    ).unwrap();
    dia!(choice!(
        lbox!(Alignment::new(
            cmt!("Start / Top"),
            240,
            61,
            HAlign::Start,
            VAlign::Top,
            true
        )),
        lbox!(Alignment::new(
            cmt!("Centered / Centered"),
            240,
            61,
            HAlign::Centered,
            VAlign::Centered,
            true
        )),
        lbox!(Alignment::new(
            cmt!("End / Bottom"),
            240,
            61,
            HAlign::End,
            VAlign::Bottom,
            true
        ))
    ));

    // The nested frame distinguishes the inner and outer alignment rectangles.
    f.write_all(b"<p>Stack: aligned text, nested frames, and a comment exceeding both minima.</p>")
        .unwrap();
    dia!(stck!(
        lbox!(Alignment::new(
            cmt!("Start / Top"),
            320,
            71,
            HAlign::Start,
            VAlign::Top,
            true
        )),
        lbox!(
            Alignment::new(
                lbox!(Alignment::new(
                    cmt!("Inner: End / Bottom"),
                    220,
                    41,
                    HAlign::End,
                    VAlign::Bottom,
                    true
                )),
                320,
                91,
                HAlign::Start,
                VAlign::Centered,
                true
            ),
            cmt!("Outer: Start / Centered")
        ),
        lbox!(Alignment::new(
            cmt!("Natural size exceeds 60 x 10"),
            60,
            10,
            HAlign::End,
            VAlign::Bottom,
            true
        ))
    ));

    // The return path reads right-to-left, so start is on the physical right.
    f.write_all(b"<p>Repeat: start and end follow the RTL return path.</p>")
        .unwrap();
    dia!(rpt!(
        cmt!("Forward path"),
        choice!(
            lbox!(Alignment::new(
                cmt!("Start = right / Top"),
                240,
                61,
                HAlign::Start,
                VAlign::Top,
                true
            )),
            lbox!(Alignment::new(
                cmt!("End = left / Bottom"),
                240,
                61,
                HAlign::End,
                VAlign::Bottom,
                true
            ))
        )
    ));

    // These minima leave each comment at its natural size.
    f.write_all(
        b"<p>Stack: zero, negative, and undersized minima leave no extra alignment space.</p>",
    )
    .unwrap();
    dia!(stck!(
        lbox!(Alignment::new(
            cmt!("0 x 0: natural size"),
            0,
            0,
            HAlign::Centered,
            VAlign::Bottom,
            true
        )),
        lbox!(Alignment::new(
            cmt!("-40 x -10: natural size"),
            -40,
            -10,
            HAlign::End,
            VAlign::Centered,
            true
        )),
        lbox!(Alignment::new(
            cmt!("20 x 10: natural size"),
            20,
            10,
            HAlign::Start,
            VAlign::Top,
            true
        ))
    ));

    // Empty children still need an external label to explain their rail position.
    f.write_all(b"<p>Choice: captions identify the rails of empty children.</p>")
        .unwrap();
    dia!(choice!(
        lbox!(
            Alignment::new(Empty, 240, 0, HAlign::Start, VAlign::Top, true),
            cmt!("Empty: no minimum height")
        ),
        lbox!(
            Alignment::new(Empty, 240, 31, HAlign::Centered, VAlign::Centered, true),
            cmt!("Empty: centered rail")
        ),
        lbox!(
            Alignment::new(Empty, 240, 31, HAlign::End, VAlign::Bottom, true),
            cmt!("Empty: bottom rail")
        )
    ));

    // No rail is drawn across the alignment padding inside these frames.
    f.write_all(b"<p>Independent comments: blank alignment padding inside equal-size frames.</p>")
        .unwrap();
    raw_dia!(horiz!(
        lbox!(Alignment::new(
            cmt!("Start / Top"),
            180,
            61,
            HAlign::Start,
            VAlign::Top,
            false
        )),
        lbox!(Alignment::new(
            cmt!("Centered"),
            180,
            61,
            HAlign::Centered,
            VAlign::Centered,
            false
        )),
        lbox!(Alignment::new(
            cmt!("End / Bottom"),
            180,
            61,
            HAlign::End,
            VAlign::Bottom,
            false
        ))
    ));
    hr!();

    // Long text, difficult width
    dia!(nonterm!(
        "This is a very long text that should not escape it's bounding box, like ever..."
    ));
    dia!(term!(
        "This is a very long text that should not escape it's bounding box, like ever..."
    ));
    dia!(cmt!(
        "This is a very long text that should not escape it's bounding box, like ever..."
    ));
    dia!(term!("大家好"));
    dia!(cmt!("ｆｏｏｂａｒ"));
    dia!(lbox!(
        nonterm!("大家好 🤸"),
        cmt!("ｆｏｏｂａｒｆｏｏｂａｒｆｏｏｂａｒ")
    ));
    hr!();

    // Optional
    dia!(opt!(dbg!(0, 20, 10)));
    dia!(opt!(dbg!(25, 45, 20)));
    dia!(opt!(dbg!(30, 50, 50)));
    hr!();

    // Sequences of varying size
    dia!(seq!(dbg!(20, 30, 10), dbg!(30, 50, 70)));
    dia!(seq!(dbg!(20, 30, 10), dbg!(20, 50, 50), dbg!(30, 50, 70)));
    hr!();

    // Choices
    dia!(choice!());
    dia!(choice!(Empty));
    dia!(choice!(Empty, Empty));
    dia!(choice!(Empty, dbg!(5, 25, 10)));
    dia!(choice!(dbg!(15, 40, 10), dbg!(25, 30, 20)));
    dia!(choice!(
        dbg!(10, 15, 10),
        dbg!(10, 15, 5),
        dbg!(20, 35, 22),
        dbg!(10, 15, 10)
    ));
    dia!(choice!(Empty, dbg!(5, 20, 10), dbg!(25, 35, 5)));
    hr!();

    // MultiChoices
    // Empty node: dimensions match an empty Choice.
    dia!(multichoice!());
    // Single column with multiple rows: dimensions match a Choice.
    dia!(multichoice!([Empty, dbg!(5, 20, 10), dbg!(25, 35, 5)]));
    // Two balanced columns with one branch in each row group.
    dia!(multichoice!(
        [dbg!(15, 40, 10), dbg!(25, 30, 20)],
        [dbg!(20, 35, 15), dbg!(10, 25, 10)]
    ));
    // Ragged columns, covering columns with different row counts.
    dia!(multichoice!(
        [dbg!(10, 15, 10), dbg!(10, 15, 5), dbg!(20, 35, 22)],
        [dbg!(30, 45, 20)],
        [dbg!(8, 20, 12), dbg!(25, 30, 8)]
    ));
    // Empty alternatives mixed into multiple columns.
    dia!(multichoice!(
        [Empty, dbg!(5, 20, 10)],
        [dbg!(25, 35, 5), Empty]
    ));
    // Uneven child widths across all columns.
    dia!(multichoice!(
        [dbg!(12, 30, 12), dbg!(12, 30, 120)],
        [dbg!(12, 30, 20), dbg!(12, 30, 75)]
    ));
    // Uneven entry heights and nested containers.
    dia!(multichoice!(
        [
            seq!(dbg!(20, 30, 10), dbg!(30, 50, 70)),
            opt!(dbg!(30, 50, 50))
        ],
        [
            stck!(dbg!(10, 15, 10), dbg!(20, 35, 22)),
            choice!(dbg!(10, 25, 20), dbg!(18, 35, 20))
        ]
    ));
    // First, middle, and final column routing all present.
    dia!(multichoice!(
        [dbg!(12, 30, 45), dbg!(12, 30, 45)],
        [dbg!(12, 30, 55), dbg!(12, 30, 55), dbg!(12, 30, 55)],
        [dbg!(12, 30, 45), dbg!(12, 30, 45)]
    ));
    // One row spread across four columns.
    dia!(multichoice!(
        [dbg!(12, 30, 40)],
        [dbg!(12, 30, 35)],
        [dbg!(12, 30, 65)],
        [dbg!(12, 30, 45)]
    ));
    // Empty middle column is ignored by the layout.
    dia!(multichoice!(
        [dbg!(12, 30, 40), dbg!(18, 40, 30)],
        [],
        [dbg!(10, 25, 55), dbg!(15, 35, 25)]
    ));
    // Single-row first column feeding tall later columns.
    dia!(multichoice!(
        [dbg!(12, 30, 35)],
        [dbg!(8, 70, 50), dbg!(45, 90, 35)],
        [dbg!(16, 35, 40), dbg!(28, 70, 60), dbg!(12, 30, 30)]
    ));
    // Tall first column with shallow final column.
    dia!(multichoice!(
        [
            dbg!(10, 35, 35),
            dbg!(40, 85, 50),
            dbg!(12, 30, 25),
            dbg!(30, 60, 45)
        ],
        [dbg!(12, 30, 60)]
    ));
    // Final column top row needs downward placement before merging upward.
    dia!(multichoice!(
        [dbg!(28, 60, 55), dbg!(12, 35, 40)],
        [dbg!(5, 20, 65), dbg!(12, 30, 45)],
        [dbg!(5, 20, 50), dbg!(14, 35, 70)]
    ));
    // Very wide middle column with narrow side columns.
    dia!(multichoice!(
        [dbg!(12, 30, 25), dbg!(15, 35, 35)],
        [dbg!(20, 45, 150), dbg!(8, 25, 120)],
        [dbg!(12, 30, 30), dbg!(18, 45, 20)]
    ));
    // High entry heights in later columns.
    dia!(multichoice!(
        [dbg!(10, 30, 45), dbg!(20, 45, 30)],
        [dbg!(45, 80, 55), dbg!(35, 70, 35)],
        [dbg!(50, 95, 65)]
    ));
    // Many ragged columns with mixed heights and widths.
    dia!(multichoice!(
        [dbg!(8, 25, 25), dbg!(18, 45, 40)],
        [dbg!(15, 35, 80)],
        [dbg!(10, 30, 35), dbg!(25, 60, 45), dbg!(12, 30, 30)],
        [dbg!(22, 55, 70), dbg!(8, 25, 20)]
    ));
    hr!();

    // Vertical grid
    raw_dia!(vert!(
        seq!(SimpleStart, term!("42"), SimpleEnd),
        cmt!("This is the answer")
    ));
    raw_dia!(vert!(
        seq!(SimpleStart, dbg!(15, 40, 10), SimpleEnd),
        seq!(Start, dbg!(25, 35, 5), End)
    ));

    hr!();

    // Horizontal grid
    raw_dia!(horiz!(
        seq!(SimpleStart, term!("42"), SimpleEnd),
        cmt!("This is the answer")
    ));
    raw_dia!(horiz!(
        seq!(SimpleStart, dbg!(15, 40, 10), SimpleEnd),
        seq!(Start, dbg!(25, 35, 5), End)
    ));

    hr!();

    // LabeledBox
    dia!(lbox!(term!("Foo"), term!("Bar!")));
    dia!(choice!(
        lbox!(Empty, cmt!("Do nothing")),
        lbox!(term!("bar"), cmt!("Do something"))
    ));

    hr!();

    // Repeats
    dia!(rpt!(Empty, Empty));
    dia!(rpt!(dbg!(20, 30, 10), dbg!(30, 50, 20)));
    dia!(rpt!(dbg!(5, 15, 10), dbg!(5, 15, 20)));
    dia!(rpt!(nonterm!("Foo"), term!(",")));
    dia!(rpt!(
        cmt!("<-- this is longer -->"),
        cmt!("this is shorter")
    ));
    dia!(rpt!(
        nonterm!("Foo"),
        lbox!(term!(","), cmt!("A comment that runs long"))
    ));
    dia!(rpt!(
        cmt!("this is shorter"),
        cmt!("<-- this is longer -->")
    ));
    hr!();

    // Stacks
    // A singleton stack used to leave a gap before the end marker.
    dia!(stck!(term!("one")));
    dia!(stck!());
    dia!(stck!(Empty));
    dia!(stck!(Empty, Empty));
    dia!(stck!(Empty, dbg!(5, 25, 10)));
    // Connector overshoot reproduction: the second child is narrower than the
    // final inter-child connector segment.
    dia!(stck!(Empty, dbg!(5, 25, 4)));
    dia!(stck!(dbg!(15, 40, 10), dbg!(25, 30, 20)));
    dia!(stck!(
        dbg!(10, 15, 10),
        dbg!(10, 15, 5),
        seq!(dbg!(), dbg!(20, 35, 22)),
        dbg!(10, 15, 10)
    ));
    hr!();

    // Links
    dia!(lnk!(term!("www.rust-lang.org")));
    dia!({
        let mut l = lnk!(term!("www.rust-lang.org"));
        l.set_target(Some(LinkTarget::Blank));
        l
    });

    hr!();

    dia!(choice!(
        rpt!(
            seq!(
                choice!(
                    term!("Foo"),
                    seq!(opt!(term!("BarNoodle")), term!("NoodleBox"), term!("foo")),
                    term!("More"),
                    term!("42")
                ),
                stck!(
                    nonterm!("Stack1"),
                    opt!(nonterm!("Stack2")),
                    nonterm!("Stack2"),
                    opt!(nonterm!("Stack4"))
                )
            ),
            term!(",")
        ),
        rpt!(term!("x"), cmt!("1-6 times"))
    ));
    hr!();

    dia!(stck!(
        seq!(
            term!("ALTER"),
            term!("TABLE"),
            opt!(seq!(term!("schema-name"), term!("."))),
            term!("table-name")
        ),
        lbox!(
            choice!(
                lbox!(
                    seq!(term!("RENAME"), term!("TO"), term!("new-table-name")),
                    cmt!("Wow")
                ),
                seq!(term!("ADD"), opt!(term!("COLUMN")), nonterm!("column-def"))
            ),
            cmt!("Foo")
        )
    ));
    hr!();

    dia!(opt!(choice!(
        seq!(term!("ON"), nonterm!("expr")),
        seq!(
            term!("USING"),
            term!("("),
            rpt!(term!("column-name"), term!(",")),
            term!(")")
        )
    )));
    hr!();

    dia!(seq!(
        nonterm!("$i:expr"),
        term!(","),
        choice!(
            seq!(
                nonterm!("$submac:ident"),
                term!("!("),
                opt!(rpt!(nonterm!("$args:tt"), Empty)),
                term!(")")
            ),
            nonterm!("$f:expr")
        )
    ));
    hr!();

    dia!(choice!(
        cmt!("Macro-internal"),
        seq!(
            nonterm!("$i:expr"),
            term!(","),
            choice!(
                seq!(
                    nonterm!("$submac:ident"),
                    term!("!"),
                    lbox!(seq!(
                        term!("("),
                        opt!(rpt!(nonterm!("$args:tt"))),
                        term!(")")
                    )),
                    term!(",")
                ),
                nonterm!("$f:expr")
            ),
            nonterm!("$g:expr")
        )
    ));

    hr!();

    f.write_all(b"</html>").unwrap();
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
