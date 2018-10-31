//! Construct some diagrams and verify them according to W3C's DTD.
//! This does not ensure that we *always* generate SVGs for all knobs and
//! switches. At least every primitive should appear here once.
//!
//! This uses `xmllint` from libxml2, which may not be available when tests are
//! executed, so all tests should be #[ignore]

#[macro_use]
extern crate lazy_static;
extern crate railroad_verification;
extern crate railroad;

use railroad::*;

lazy_static! {
    static ref VERIFIER: railroad_verification::Verifier = {
        railroad_verification::Verifier::new().unwrap()
    };
}

macro_rules! verify {
    ($testname:ident, $src:expr) => {
        #[test]
        #[ignore]
        fn $testname() {
            VERIFIER.verify($src).unwrap();
        }
    }
}

macro_rules! raw_dia { ($r:expr) => { Diagram::with_default_css($r).to_string() }; }
macro_rules! dia { ($r:expr) => { raw_dia!(seq!(SimpleStart, $r, SimpleEnd)); }; }
macro_rules! nonterm { ($r:expr) => { NonTerminal::new($r.to_owned()) } }
macro_rules! term { ($r:expr) => { Terminal::new($r.to_owned()) } }
macro_rules! seq { ($($r: expr),*) => { Sequence::new(vec![ $( Box::new($r), )+ ]) } }
macro_rules! choice { ($($r: expr),*) => { Choice::new(vec![ $( Box::new($r), )* ]) } }
macro_rules! stck { ($($r: expr),*) => { Stack::new(vec![ $( Box::new($r), )* ]) } }
macro_rules! cmt { ($r:expr) => { Comment::new($r.to_owned()) } }
macro_rules! vert { ($($r: expr),*) => { VerticalGrid::new(vec![ $( Box::new($r), )+ ]) } }
macro_rules! horiz { ($($r: expr),*) => { HorizontalGrid::new(vec![ $( Box::new($r), )+ ]) } }
macro_rules! rpt {
    ($r:expr, $s:expr) => {
        Repeat::new($r, $s)
    };
    ($r:expr) => {
        rpt!($r, Empty)
    }
}
macro_rules! opt { ($r:expr) => { Optional::new($r) } }
macro_rules! lbox {
    ($r:expr, $u:expr) => {
        LabeledBox::new($r, $u)
    };
    ($r:expr) => {
        LabeledBox::new($r, Empty)
    }
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
