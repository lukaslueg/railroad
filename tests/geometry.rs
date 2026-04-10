//! Geometry tests for every built-in [`Node`] implementation.
//!
//! Each test checks [`Node::entry_height`], [`Node::height`], and
//! [`Node::width`] for a specific primitive or composite node, as well as
//! the basic invariants (non-negative values, `height >= entry_height`).
//!
//! [`railroad::Debug`] is used as a building block for composite-node tests
//! because its geometry is fully user-controlled, making the expected values
//! easy to derive by hand.
//!
//! ## Notation used in inline comments
//!
//! | symbol | meaning |
//! |--------|---------|
//! | `R`    | `ARC_RADIUS` (= 12) |
//! | `eh`   | `entry_height()` |
//! | `h`    | `height()` |
//! | `hbe`  | `height_below_entry()` = `h - eh` |
//! | `w`    | `width()` |

use railroad::node_test_utils::{assert_node_geometry, check_invariants};
use railroad::Node;

// Alias to avoid collision with std::fmt::Debug.
type Dbg = railroad::Debug;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Check invariants and exact geometry in one call.
fn check(node: &dyn Node, entry_height: i64, height: i64, width: i64) {
    check_invariants(node);
    assert_node_geometry(node, entry_height, height, width);
}

// ── primitive nodes ───────────────────────────────────────────────────────────

#[test]
fn empty_geometry() {
    check(&railroad::Empty, 0, 0, 0);
}

#[test]
fn start_geometry() {
    // Fixed constants declared in the implementation.
    check(&railroad::Start, 10, 20, 20);
}

#[test]
fn simple_start_geometry() {
    check(&railroad::SimpleStart, 5, 10, 15);
}

#[test]
fn end_geometry() {
    check(&railroad::End, 10, 20, 20);
}

#[test]
fn simple_end_geometry() {
    check(&railroad::SimpleEnd, 5, 10, 15);
}

#[test]
fn terminal_empty_label() {
    // width = text_width("") * 8 + 20 = 0 * 8 + 20 = 20
    let t = railroad::Terminal::new(String::new());
    check(&t, 11, 22, 20);
}

#[test]
fn terminal_ascii_label() {
    // "Foobar": unicode_width = 6, fudge = 6/20 = 0  →  text_width = 6
    // width = 6 * 8 + 20 = 68
    let t = railroad::Terminal::new("Foobar".to_owned());
    check(&t, 11, 22, 68);
}

#[test]
fn nonterminal_empty_label() {
    let nt = railroad::NonTerminal::new(String::new());
    check(&nt, 11, 22, 20);
}

#[test]
fn nonterminal_ascii_label() {
    // Same formula as Terminal.
    let nt = railroad::NonTerminal::new("Foobar".to_owned());
    check(&nt, 11, 22, 68);
}

#[test]
fn comment_empty_text() {
    // width = text_width("") * 7 + 10 = 10
    let c = railroad::Comment::new(String::new());
    check(&c, 10, 20, 10);
}

#[test]
fn comment_ascii_text() {
    // "Foobar": text_width = 6  →  width = 6 * 7 + 10 = 52
    let c = railroad::Comment::new("Foobar".to_owned());
    check(&c, 10, 20, 52);
}

// ── Sequence ─────────────────────────────────────────────────────────────────

#[test]
fn sequence_empty() {
    let s = railroad::Sequence::<Box<dyn Node>>::new(vec![]);
    check(&s, 0, 0, 0);
}

#[test]
fn sequence_single_child() {
    // A single Dbg(10, 20, 30): the Sequence just adopts its geometry.
    let s = railroad::Sequence::new(vec![Dbg::new(10, 20, 30)]);
    // eh = max(10) = 10;  h = 10 + hbe(10) = 20;  w = 30 (no spacing for 1 child)
    check(&s, 10, 20, 30);
}

#[test]
fn sequence_two_children_symmetric() {
    // d1: eh=10, h=20, hbe=10, w=30
    // d2: eh=15, h=25, hbe=10, w=40
    // spacing = 10
    // eh  = max(10, 15) = 15
    // h   = max_entry(15) + max_hbe(10) = 25
    // w   = 30 + 40 + 10 = 80
    let s = railroad::Sequence::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 40)]);
    check(&s, 15, 25, 80);
}

#[test]
fn sequence_asymmetric_entry_heights() {
    // d1: eh=5, h=30, hbe=25, w=20
    // d2: eh=20, h=25, hbe=5, w=10
    // eh = max(5, 20) = 20
    // h  = 20 + max(25, 5) = 20 + 25 = 45
    // w  = 20 + 10 + 10 = 40
    let s = railroad::Sequence::new(vec![Dbg::new(5, 30, 20), Dbg::new(20, 25, 10)]);
    check(&s, 20, 45, 40);
}

#[test]
fn sequence_three_children() {
    // spacing = 10; applied between each adjacent pair (2 gaps total)
    // d1: eh=10, h=20, w=30;  d2: eh=10, h=20, w=20;  d3: eh=10, h=20, w=40
    // eh = 10;  h = 10 + 10 = 20;  w = 30 + 20 + 40 + 2*10 = 110
    let s = railroad::Sequence::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(10, 20, 20),
        Dbg::new(10, 20, 40),
    ]);
    check(&s, 10, 20, 110);
}

// ── Optional ─────────────────────────────────────────────────────────────────
//
// R = ARC_RADIUS = 12
// eh  = R + max(R, inner.eh)
// h   = eh + inner.hbe
// w   = 2*R + inner.w + 2*R  = 4*R + inner.w

#[test]
fn optional_empty_inner() {
    // inner: eh=0, h=0, hbe=0, w=0
    // eh = 12 + max(12, 0) = 24;  h = 24 + 0 = 24;  w = 48 + 0 = 48
    let o = railroad::Optional::new(railroad::Empty);
    check(&o, 24, 24, 48);
}

#[test]
fn optional_inner_entry_smaller_than_arc() {
    // inner: eh=5, h=15, hbe=10, w=30
    // eh = 12 + max(12, 5) = 12 + 12 = 24;  h = 24 + 10 = 34;  w = 48 + 30 = 78
    let o = railroad::Optional::new(Dbg::new(5, 15, 30));
    check(&o, 24, 34, 78);
}

#[test]
fn optional_inner_entry_larger_than_arc() {
    // inner: eh=20, h=30, hbe=10, w=10
    // eh = 12 + max(12, 20) = 12 + 20 = 32;  h = 32 + 10 = 42;  w = 48 + 10 = 58
    let o = railroad::Optional::new(Dbg::new(20, 30, 10));
    check(&o, 32, 42, 58);
}

// ── Choice ───────────────────────────────────────────────────────────────────
//
// For len > 1:
//   inner_padding = 2*R = 24
//   eh = first.eh
//   w  = 24 + max_width + 24
//   h  = eh
//       + max(R, spacing + first.hbe)
//       + Σ padded_height(child) for children[1..]
//       - spacing
//
// padded_height(c) = max(R, c.eh) + c.hbe + spacing

#[test]
fn choice_empty() {
    let c = railroad::Choice::<railroad::Empty>::new(vec![]);
    check(&c, 0, 0, 0);
}

#[test]
fn choice_single_child() {
    // single child → no padding, adopts child geometry
    let c = railroad::Choice::new(vec![Dbg::new(10, 20, 30)]);
    check(&c, 10, 20, 30);
}

#[test]
fn choice_two_children() {
    // d1: eh=10, h=20, hbe=10, w=30
    // d2: eh=15, h=25, hbe=10, w=40
    // spacing = 10,  R = 12,  inner_padding = 24
    // eh  = 10
    // w   = 24 + max(30,40) + 24 = 88
    // padded_height(d2) = max(12,15) + 10 + 10 = 35
    // h   = 10 + max(12, 10+10) + 35 - 10
    //     = 10 + 20 + 35 - 10 = 55
    let c = railroad::Choice::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 40)]);
    check(&c, 10, 55, 88);
}

#[test]
fn choice_three_children() {
    // d1: eh=10, h=20, hbe=10, w=30
    // d2: eh=8,  h=18, hbe=10, w=20
    // d3: eh=12, h=22, hbe=10, w=35
    // spacing=10, R=12, inner_padding=24
    // eh  = 10
    // w   = 24 + 35 + 24 = 83
    // padded_height(d2) = max(12,8)+10+10 = 32
    // padded_height(d3) = max(12,12)+10+10 = 32
    // h   = 10 + max(12, 10+10) + (32+32) - 10
    //     = 10 + 20 + 64 - 10 = 84
    let c = railroad::Choice::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(8, 18, 20),
        Dbg::new(12, 22, 35),
    ]);
    check(&c, 10, 84, 83);
}

// ── Stack ────────────────────────────────────────────────────────────────────
//
// For len > 1, effective padding:
//   left_padding  = max(10, R)   = 12
//   right_padding = max(10, 2*R) = 24
//
// padded_height(child, next) =
//   child.eh + max(child.hbe + spacing, 2*R) + R + max(0, R - next.eh)
//
// width:
//   l = left_padding + max_width + right_padding
//   If any non-last child has w >= last child's w  →  l + R,  else l

#[test]
fn stack_empty() {
    let s = railroad::Stack::<railroad::Empty>::new(vec![]);
    check(&s, 0, 0, 0);
}

#[test]
fn stack_single_child() {
    // No padding when there's only one child.
    let s = railroad::Stack::new(vec![Dbg::new(10, 20, 30)]);
    check(&s, 10, 20, 30);
}

#[test]
fn stack_two_children_last_wider() {
    // d1: eh=10, h=20, hbe=10, w=30
    // d2: eh=15, h=25, hbe=10, w=40  (last is wider → no extra R)
    // spacing = R = 12
    // padded_height(d1, d2) = 10 + max(10+12, 24) + 12 + max(0, 12-15)
    //                       = 10 + max(22,24) + 12 + 0
    //                       = 10 + 24 + 12 = 46
    // h   = 46 + 25 = 71
    // l   = 12 + max(30,40) + 24 = 76;  30 >= 40? No  →  w = 76
    let s = railroad::Stack::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 40)]);
    check(&s, 10, 71, 76);
}

#[test]
fn stack_two_children_first_wider() {
    // d1: eh=10, h=20, hbe=10, w=30  (first is wider → extra R added)
    // d2: eh=15, h=25, hbe=10, w=20
    // padded_height(d1, d2): same as above = 46
    // h = 46 + 25 = 71
    // l = 12 + max(30,20) + 24 = 66;  30 >= 20? Yes  →  w = 66 + 12 = 78
    let s = railroad::Stack::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 20)]);
    check(&s, 10, 71, 78);
}

// ── Repeat ───────────────────────────────────────────────────────────────────
//
// height_between_entries = max(2*R, inner.hbe + spacing + repeat.eh)
// eh  = inner.eh
// h   = inner.eh + height_between_entries + repeat.hbe
// w   = R + max(inner.w, repeat.w) + R

#[test]
fn repeat_both_empty() {
    // inner: Empty (eh=0, h=0, hbe=0, w=0)
    // repeat: Empty
    // hbe  = max(24, 0+10+0) = 24
    // eh=0;  h = 0+24+0 = 24;  w = 12+0+12 = 24
    let r = railroad::Repeat::new(railroad::Empty, railroad::Empty);
    check(&r, 0, 24, 24);
}

#[test]
fn repeat_repeat_wider() {
    // inner:  eh=10, h=20, hbe=10, w=30
    // repeat: eh=5,  h=15, hbe=10, w=40
    // hbe = max(24, 10+10+5) = max(24,25) = 25
    // eh=10;  h = 10+25+10 = 45;  w = 12+max(30,40)+12 = 64
    let r = railroad::Repeat::new(Dbg::new(10, 20, 30), Dbg::new(5, 15, 40));
    check(&r, 10, 45, 64);
}

#[test]
fn repeat_inner_wider() {
    // inner:  eh=10, h=20, hbe=10, w=40
    // repeat: eh=5,  h=15, hbe=10, w=30
    // hbe = max(24, 10+10+5) = 25
    // eh=10;  h = 10+25+10 = 45;  w = 12+max(40,30)+12 = 64
    let r = railroad::Repeat::new(Dbg::new(10, 20, 40), Dbg::new(5, 15, 30));
    check(&r, 10, 45, 64);
}

// ── LabeledBox ───────────────────────────────────────────────────────────────
//
// padding() = 8  if  label.h + inner.h + label.w + inner.w > 0,  else 0
// spacing() = 8  if  label.h > 0,  else 0
// eh  = padding + label.h + spacing + inner.eh
// h   = padding + label.h + spacing + inner.h + padding
// w   = padding + max(inner.w, label.w) + padding

#[test]
fn labeled_box_both_empty() {
    // All zeros → padding() = 0, spacing() = 0
    let lb = railroad::LabeledBox::new(railroad::Empty, railroad::Empty);
    check(&lb, 0, 0, 0);
}

#[test]
fn labeled_box_inner_only() {
    // inner: eh=10, h=20, hbe=10, w=30;  label: Empty
    // padding=8, spacing=0 (label.h==0)
    // eh = 8 + 0 + 0 + 10 = 18
    // h  = 8 + 0 + 0 + 20 + 8 = 36
    // w  = 8 + max(30,0) + 8 = 46
    let lb = railroad::LabeledBox::new(Dbg::new(10, 20, 30), railroad::Empty);
    check(&lb, 18, 36, 46);
}

#[test]
fn labeled_box_label_only() {
    // inner: Empty;  label: eh=5, h=15, hbe=10, w=40
    // padding=8, spacing=8 (label.h>0)
    // eh = 8 + 15 + 8 + 0 = 31
    // h  = 8 + 15 + 8 + 0 + 8 = 39
    // w  = 8 + max(0,40) + 8 = 56
    let lb = railroad::LabeledBox::new(railroad::Empty, Dbg::new(5, 15, 40));
    check(&lb, 31, 39, 56);
}

#[test]
fn labeled_box_both_present() {
    // inner: eh=10, h=20, hbe=10, w=30
    // label: eh=5,  h=15, hbe=10, w=40
    // padding=8, spacing=8
    // eh = 8 + 15 + 8 + 10 = 41
    // h  = 8 + 15 + 8 + 20 + 8 = 59
    // w  = 8 + max(30,40) + 8 = 56
    let lb = railroad::LabeledBox::new(Dbg::new(10, 20, 30), Dbg::new(5, 15, 40));
    check(&lb, 41, 59, 56);
}

// ── VerticalGrid ─────────────────────────────────────────────────────────────
//
// entry_height always 0
// h = Σ child.h + (max(1, len) - 1) * R     (R = 12 = spacing)
// w = max child.w

#[test]
fn vertical_grid_empty() {
    let vg = railroad::VerticalGrid::<railroad::Empty>::new(vec![]);
    // h = 0 + (max(1,0)-1)*12 = 0;  w = 0
    check(&vg, 0, 0, 0);
}

#[test]
fn vertical_grid_single() {
    let vg = railroad::VerticalGrid::new(vec![Dbg::new(10, 20, 30)]);
    // h = 20 + (1-1)*12 = 20;  w = 30
    check(&vg, 0, 20, 30);
}

#[test]
fn vertical_grid_two_children() {
    // d1: h=20, w=30;  d2: h=25, w=40
    // h = (20+25) + (2-1)*12 = 45+12 = 57;  w = max(30,40) = 40
    let vg =
        railroad::VerticalGrid::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 40)]);
    check(&vg, 0, 57, 40);
}

#[test]
fn vertical_grid_three_children() {
    // d1: h=10, w=20;  d2: h=15, w=30;  d3: h=20, w=10
    // h = (10+15+20) + 2*12 = 45+24 = 69;  w = 30
    let vg = railroad::VerticalGrid::new(vec![
        Dbg::new(5, 10, 20),
        Dbg::new(5, 15, 30),
        Dbg::new(5, 20, 10),
    ]);
    check(&vg, 0, 69, 30);
}

// ── HorizontalGrid ───────────────────────────────────────────────────────────
//
// entry_height always 0
// h = max child.h
// w = Σ child.w + (max(1, len) - 1) * R     (R = 12 = spacing)

#[test]
fn horizontal_grid_empty() {
    let hg = railroad::HorizontalGrid::<railroad::Empty>::new(vec![]);
    check(&hg, 0, 0, 0);
}

#[test]
fn horizontal_grid_single() {
    let hg = railroad::HorizontalGrid::new(vec![Dbg::new(10, 20, 30)]);
    // h = 20;  w = 30 + (1-1)*12 = 30
    check(&hg, 0, 20, 30);
}

#[test]
fn horizontal_grid_two_children() {
    // d1: h=20, w=30;  d2: h=25, w=40
    // h = max(20,25) = 25;  w = (30+40) + 1*12 = 82
    let hg =
        railroad::HorizontalGrid::new(vec![Dbg::new(10, 20, 30), Dbg::new(15, 25, 40)]);
    check(&hg, 0, 25, 82);
}

// ── Link ─────────────────────────────────────────────────────────────────────
//
// Link is a transparent wrapper: its geometry equals its inner node's.

#[test]
fn link_transparent_geometry() {
    let inner = Dbg::new(10, 20, 30);
    let link = railroad::Link::new(inner, "https://example.com".to_owned());
    check(&link, 10, 20, 30);
}

// ── Diagram ──────────────────────────────────────────────────────────────────
//
// Diagram adds padding: left=10, right=10, top=10, bottom=10 (defaults).
// eh = 0  (always)
// h  = top_padding + root.h + bottom_padding  = 10 + root.h + 10
// w  = left_padding + root.w + right_padding  = 10 + root.w + 10

#[test]
fn diagram_geometry() {
    // root: Dbg(10, 20, 30)  →  h=20, w=30
    // h = 10+20+10 = 40;  w = 10+30+10 = 50
    let root = Dbg::new(10, 20, 30);
    let dia = railroad::Diagram::new(root);
    check(&dia, 0, 40, 50);
}

// ── height_below_entry consistency ───────────────────────────────────────────

#[test]
fn height_below_entry_is_consistent() {
    // height_below_entry() is a default method: h - eh.
    // Verify it matches for a representative set of nodes.
    fn hbe_ok(node: &dyn Node) {
        assert_eq!(
            node.height_below_entry(),
            node.height() - node.entry_height(),
            "height_below_entry inconsistency"
        );
    }

    hbe_ok(&railroad::Empty);
    hbe_ok(&railroad::Start);
    hbe_ok(&railroad::SimpleStart);
    hbe_ok(&railroad::End);
    hbe_ok(&railroad::SimpleEnd);
    hbe_ok(&railroad::Terminal::new("hello".to_owned()));
    hbe_ok(&railroad::NonTerminal::new("hello".to_owned()));
    hbe_ok(&railroad::Comment::new("hello".to_owned()));
    hbe_ok(&Dbg::new(10, 20, 30));
    hbe_ok(&railroad::Optional::new(Dbg::new(10, 20, 30)));
    hbe_ok(&railroad::Sequence::new(vec![Dbg::new(10, 20, 30)]));
    hbe_ok(&railroad::Choice::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(15, 25, 40),
    ]));
    hbe_ok(&railroad::Stack::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(15, 25, 40),
    ]));
    hbe_ok(&railroad::Repeat::new(
        Dbg::new(10, 20, 30),
        Dbg::new(5, 15, 20),
    ));
    hbe_ok(&railroad::LabeledBox::new(
        Dbg::new(10, 20, 30),
        railroad::Empty,
    ));
    hbe_ok(&railroad::VerticalGrid::new(vec![Dbg::new(10, 20, 30)]));
    hbe_ok(&railroad::HorizontalGrid::new(vec![Dbg::new(10, 20, 30)]));
}

// ── invariants hold for all nodes ────────────────────────────────────────────

#[test]
fn all_builtin_nodes_satisfy_invariants() {
    fn inv(node: &dyn Node) {
        check_invariants(node);
    }

    inv(&railroad::Empty);
    inv(&railroad::Start);
    inv(&railroad::SimpleStart);
    inv(&railroad::End);
    inv(&railroad::SimpleEnd);
    inv(&railroad::Terminal::new("test".to_owned()));
    inv(&railroad::NonTerminal::new("test".to_owned()));
    inv(&railroad::Comment::new("test".to_owned()));
    inv(&Dbg::new(10, 20, 30));
    inv(&railroad::Optional::new(railroad::Empty));
    inv(&railroad::Optional::new(Dbg::new(20, 30, 10)));
    inv(&railroad::Sequence::<railroad::Empty>::new(vec![]));
    inv(&railroad::Sequence::new(vec![Dbg::new(10, 20, 30)]));
    inv(&railroad::Sequence::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(15, 25, 40),
    ]));
    inv(&railroad::Choice::<railroad::Empty>::new(vec![]));
    inv(&railroad::Choice::new(vec![Dbg::new(10, 20, 30)]));
    inv(&railroad::Choice::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(15, 25, 40),
    ]));
    inv(&railroad::Stack::<railroad::Empty>::new(vec![]));
    inv(&railroad::Stack::new(vec![Dbg::new(10, 20, 30)]));
    inv(&railroad::Stack::new(vec![
        Dbg::new(10, 20, 30),
        Dbg::new(15, 25, 40),
    ]));
    inv(&railroad::Repeat::new(railroad::Empty, railroad::Empty));
    inv(&railroad::Repeat::new(Dbg::new(10, 20, 30), Dbg::new(5, 15, 20)));
    inv(&railroad::LabeledBox::new(railroad::Empty, railroad::Empty));
    inv(&railroad::LabeledBox::new(Dbg::new(10, 20, 30), railroad::Empty));
    inv(&railroad::VerticalGrid::<railroad::Empty>::new(vec![]));
    inv(&railroad::VerticalGrid::new(vec![Dbg::new(10, 20, 30)]));
    inv(&railroad::HorizontalGrid::<railroad::Empty>::new(vec![]));
    inv(&railroad::HorizontalGrid::new(vec![Dbg::new(10, 20, 30)]));
    inv(&railroad::Link::new(
        Dbg::new(10, 20, 30),
        "https://example.com".to_owned(),
    ));
    inv(&railroad::Diagram::new(Dbg::new(10, 20, 30)));
}
