
use std::fs;
use std::io::Write;

fn main() {
    use railroad::*;

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
    macro_rules! seq { ($($r: expr),*) => { Sequence::new(vec![ $( Box::new($r), )+ ]) } }
    macro_rules! choice { ($($r: expr),*) => { Choice::new(vec![ $( Box::new($r), )* ]) } }
    macro_rules! stck { ($($r: expr),*) => { Stack::new(vec![ $( Box::new($r), )* ]) } }
    macro_rules! cmt {
        ($r:expr) => {
            Comment::new($r.to_owned())
        };
    }
    macro_rules! vert { ($($r: expr),*) => { VerticalGrid::new(vec![ $( Box::new($r), )+ ]) } }
    macro_rules! horiz { ($($r: expr),*) => { HorizontalGrid::new(vec![ $( Box::new($r), )+ ]) } }
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

    f.write_all(b"<html>").unwrap();
    f.write_all(b"<head><style type=\"text/css\">svg.railroad { border: 1px solid; margin: 10px }</style></head>").unwrap();

    // Very simple
    dia!(nonterm!("Foo"));
    dia!(lbox!(nonterm!("Foo")));
    dia!(lbox!(nonterm!("Foo"), cmt!("Read the docs regarding foo!")));
    hr!();

    // Very simple, varying size
    dia!(dbg!());
    dia!(dbg!(20, 50, 50));
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
    dia!(stck!());
    dia!(stck!(Empty));
    dia!(stck!(Empty, Empty));
    dia!(stck!(Empty, dbg!(5, 25, 10)));
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
