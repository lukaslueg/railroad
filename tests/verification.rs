//! Construct some diagrams and verify them according to W3C's DTD.
//! This does not ensure that we *always* generate SVGs for all knobs and
//! switches. At least every primitive should appear here once.
//!
//! This uses `xmllint` from libxml2, which may not be available when tests are
//! executed, so all tests should be #[ignore]

#[macro_use]
extern crate lazy_static;

use railroad::Node;

lazy_static! {
    static ref VERIFIER: railroad_verification::Verifier =
        railroad_verification::Verifier::new().unwrap();
}

macro_rules! verify {
    ($testname:ident, $src:expr) => {
        #[test]
        #[ignore]
        fn $testname() {
            VERIFIER.verify($src).unwrap();
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
        raw_dia!(seq!(railroad::SimpleStart, $r, railroad::SimpleEnd));
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
verify!(escape_nonterm, dia!(nonterm!("Foo<bar>")));
verify!(simple_term, dia!(term!("Foobar")));
verify!(escape_term, dia!(term!("Foo<bar>")));
verify!(simple_choice, dia!(choice!(term!("Foo"), term!("Bar"))));
verify!(simple_stack, dia!(stck!(term!("Foo"), term!("Bar"))));
verify!(simple_comment, dia!(cmt!("Foobar")));
verify!(escape_comment, dia!(cmt!("Foo<bar>")));
verify!(simple_vertical, dia!(vert!(term!("Foo"), term!("Bar"))));
verify!(simple_horizontal, dia!(horiz!(term!("Foo"), term!("Bar"))));
verify!(simple_repeat, dia!(rpt!(term!("Foo"))));
verify!(simple_opt, dia!(opt!(term!("Foo"))));
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
