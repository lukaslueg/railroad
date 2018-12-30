// MIT License
//
// Copyright (c) 2018 Lukas Lueg (lukas.lueg@gmail.com)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
//! A library to create syntax ("railroad") diagrams as Scalable Vector Graphics (SVG).
//!
//! Railroad diagrams are a graphical way to represent context-free grammar.
//! Every diagram has exactly one starting- and one end-point; everything that
//! belongs to the described language is represented by one of the possible paths
//! between those points.
//!
//! Using this library, diagrams are created by primitives which implemented `RailroadNode`.
//! Primitives are combined into more complex strctures by wrapping simple elements into more
//! complex ones.
//!
//! ```
//! use railroad::*;
//!
//! // This diagram will be a (horizontal) sequence of simple elements
//! let mut seq = Sequence::default();
//! seq.push(Box::new(Start))
//!    .push(Box::new(Terminal::new("BEGIN".to_owned())))
//!    .push(Box::new(NonTerminal::new("syntax".to_owned())))
//!    .push(Box::new(End));
//!
//! let mut dia = Diagram::new(seq);
//!
//! // The library only computes the diagram's geometry; we use CSS for layout.
//! dia.add_element(svg::Element::new("style")
//!                 .set("type", "text/css")
//!                 .raw_text(DEFAULT_CSS));
//!
//! // A `RailroadNode`'s `fmt::Display` is its SVG.
//! println!("<html>{}</html>", dia);
//! ```
//!
use std::cmp;
use std::collections;
use std::fmt;
use std::io;

use htmlescape;

pub mod notactuallysvg;
pub use crate::notactuallysvg as svg;
use crate::svg::HDir;

/// Used as a form of scale throughout geometry calculations. Smaller values result in more compact
/// diagrams.
const ARC_RADIUS: i64 = 12;

/// Determine the width some text will have when rendered.
///
/// The geometry of some primitives depends on this, which is hacky in the first place.
fn text_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    // Use a fudge-factor of 1.05
    s.width() + (s.width() / 20)
}

pub const DEFAULT_CSS: &str = r#"
    svg.railroad {
        background-color: hsl(30, 20%, 95%);
        background-size: 15px 15px;
        background-image: linear-gradient(to right, rgba(30, 30, 30, .05) 1px, transparent 1px),
                          linear-gradient(to bottom, rgba(30, 30, 30, .05) 1px, transparent 1px);
    }

    svg.railroad path {
        stroke-width: 3px;
        stroke: black;
        fill: transparent;
    }

    svg.railroad .debug {
        stroke-width: 1px;
        stroke: red;
    }

    svg.railroad text {
        font: 14px monospace;
        text-anchor: middle;
    }

    svg.railroad .nonterminal text {
        font-weight: bold;
    }

    svg.railroad text.comment {
        font: italic 12px monospace;
    }

    svg.railroad rect {
        stroke-width: 3px;
        stroke: black;
        fill:hsl(-290, 70%, 90%);
    }

    svg.railroad g.labeledbox > rect {
        stroke-width: 1px;
        stroke: grey;
        stroke-dasharray: 5px;
        fill:rgba(90, 90, 150, .1);
    }"#;

// TODO escape all the attributes

/// A diagram is built from a set of primitives which implement `RailroadNode`.
///
/// A primitive is a geometric box, within which it can draw whatever it wants.
/// Simple primitives (e.g. `Start`) have fixed width, height etc.. Complex
/// primitives, which wrap other primitives, (e.g. `Sequence`) use the methods
/// defined here to compute their own geometry. When the time comes for a primitive
/// to be drawn, the wrapping primitive computes the desired location of the wrapped
/// primitive(s) and calls `.draw()` on them. It is the primitive's job
/// to ensure that it uses only the space it announced.
pub trait RailroadNode: ::std::fmt::Debug {
    /// The vertical distance from this element's top to where the entering,
    /// connecting path is drawn.
    ///
    /// By convention, the path connecting primitives enters from the left.
    fn entry_height(&self) -> i64;

    /// Currently unused, as all elements exit on the same height as their entry.
    /// In the future, we may want to relax that constraint.
    #[doc(hidden)]
    fn exit_height(&self) -> i64 {
        self.entry_height()
    }

    /// This primitives's total height.
    fn height(&self) -> i64;

    /// This primitive's total width.
    fn width(&self) -> i64;

    /// The vertical distance from the height of the connecting path to the bottom.
    fn height_below_entry(&self) -> i64 {
        self.height() - self.entry_height()
    }

    /// Draw this element as an `svg::Element`.
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element;
}

impl<T> RailroadNode for Box<T>
where
    T: RailroadNode + ?Sized,
{
    fn entry_height(&self) -> i64 {
        (**self).entry_height()
    }
    fn width(&self) -> i64 {
        (**self).width()
    }
    fn height(&self) -> i64 {
        (**self).height()
    }
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        (**self).draw(x, y, h_dir)
    }
}

impl<'a, T> RailroadNode for &'a Box<T>
where
    T: RailroadNode + 'a + ?Sized,
{
    fn entry_height(&self) -> i64 {
        (**self).entry_height()
    }
    fn width(&self) -> i64 {
        (**self).width()
    }
    fn height(&self) -> i64 {
        (**self).height()
    }
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        (**self).draw(x, y, h_dir)
    }
}

impl<T> RailroadNode for Option<T>
where
    T: RailroadNode,
{
    fn entry_height(&self) -> i64 {
        self.as_ref().map(|c| c.entry_height()).unwrap_or_default()
    }

    fn exit_height(&self) -> i64 {
        self.as_ref().map(|c| c.exit_height()).unwrap_or_default()
    }

    fn height(&self) -> i64 {
        self.as_ref().map(|c| c.height()).unwrap_or_default()
    }

    fn width(&self) -> i64 {
        self.as_ref().map(|c| c.width()).unwrap_or_default()
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        match self {
            Some(c) => c.draw(x, y, h_dir),
            None => unimplemented!(),
        }
    }
}

trait RailroadNodeCollection {
    fn max_entry_height(&self) -> i64;
    fn max_height(&self) -> i64;
    fn max_height_below_entry(&self) -> i64;
    fn max_width(&self) -> i64;
    fn total_width(&self) -> i64;
    fn total_height(&self) -> i64;
    fn running_x<'a>(
        &'a self,
        spacing: i64,
    ) -> Box<dyn Iterator<Item = (&'a Box<dyn RailroadNode>, i64)> + 'a>;
}

impl RailroadNodeCollection for Vec<Box<dyn RailroadNode>> {
    fn running_x<'a>(
        &'a self,
        spacing: i64,
    ) -> Box<dyn Iterator<Item = (&'a Box<dyn RailroadNode>, i64)> + 'a> {
        let mut x = 0;
        let it = self.iter().map(move |child| {
            let z = x;
            x += child.width() + spacing;
            (child, z)
        });
        Box::new(it)
    }

    fn max_height_below_entry(&self) -> i64 {
        self.iter()
            .map(|c| c.height_below_entry())
            .max()
            .unwrap_or_default()
    }

    fn max_entry_height(&self) -> i64 {
        self.iter()
            .map(|c| c.entry_height())
            .max()
            .unwrap_or_default()
    }

    fn max_height(&self) -> i64 {
        self.iter().map(|c| c.height()).max().unwrap_or_default()
    }

    fn max_width(&self) -> i64 {
        self.iter().map(|c| c.width()).max().unwrap_or_default()
    }

    fn total_width(&self) -> i64 {
        self.iter().map(|c| c.width()).sum()
    }

    fn total_height(&self) -> i64 {
        self.iter().map(|c| c.height()).sum()
    }
}

#[derive(Debug)]
pub enum LinkTarget {
    Blank,
    Parent,
    Top,
}

/// Wraps another primitive, making it a clickable link to some URI.
#[derive(Debug)]
pub struct Link<I: RailroadNode> {
    inner: I,
    uri: String,
    target: Option<LinkTarget>,
    attributes: collections::HashMap<String, String>,
}

impl<I> Link<I>
where
    I: RailroadNode,
{
    pub fn new(inner: I, uri: String) -> Self {
        let mut l = Link {
            inner,
            uri,
            target: None,
            attributes: collections::HashMap::new(),
        };
        l.attributes.insert("class".to_owned(), "link".to_owned());
        l
    }

    /// Set the target-attribute
    pub fn set_target(&mut self, target: Option<LinkTarget>) {
        self.target = target;
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl<I: RailroadNode> RailroadNode for Link<I> {
    fn entry_height(&self) -> i64 {
        self.inner.entry_height()
    }
    fn height(&self) -> i64 {
        self.inner.height()
    }
    fn width(&self) -> i64 {
        self.inner.width()
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut a = svg::Element::new("a")
            .debug("Link", x, y, self)
            .set("xlink:href", self.uri.clone());
        a = match self.target {
            Some(LinkTarget::Blank) => a.set("target", "_blank"),
            Some(LinkTarget::Parent) => a.set("target", "_parent"),
            Some(LinkTarget::Top) => a.set("target", "_top"),
            None => a,
        };
        a.set_all(self.attributes.iter())
            .add(self.inner.draw(x, y, h_dir))
    }
}

/// A vertical group of unconnected elements.
#[derive(Debug)]
pub struct VerticalGrid {
    children: Vec<Box<dyn RailroadNode>>,
    spacing: i64,
    attributes: collections::HashMap<String, String>,
}

impl VerticalGrid {
    pub fn new(children: Vec<Box<dyn RailroadNode>>) -> Self {
        let mut v = VerticalGrid {
            children,
            spacing: ARC_RADIUS,
            attributes: collections::HashMap::new(),
        };
        v.attributes
            .insert("class".to_owned(), "verticalgrid".to_owned());
        v
    }

    pub fn push(&mut self, child: Box<dyn RailroadNode>) -> &mut Self {
        self.children.push(child);
        self
    }

    pub fn into_inner(self) -> Vec<Box<dyn RailroadNode>> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl ::std::iter::FromIterator<Box<dyn RailroadNode>> for VerticalGrid {
    fn from_iter<T: IntoIterator<Item = Box<dyn RailroadNode>>>(iter: T) -> Self {
        VerticalGrid::new(iter.into_iter().collect())
    }
}

impl RailroadNode for VerticalGrid {
    fn entry_height(&self) -> i64 {
        0
    }

    fn height(&self) -> i64 {
        self.children.total_height()
            + ((cmp::max(1, self.children.len() as i64) - 1) * self.spacing)
    }

    fn width(&self) -> i64 {
        self.children.max_width()
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());
        let mut running_y = y;
        for child in &self.children {
            g.push(child.draw(x, running_y, h_dir));
            running_y += child.height() + self.spacing;
        }
        g.debug("VerticalGrid", x, y, self)
    }
}

/// A horizontal group of unconnected elements.
#[derive(Debug)]
pub struct HorizontalGrid {
    children: Vec<Box<dyn RailroadNode>>,
    spacing: i64,
    attributes: collections::HashMap<String, String>,
}

impl HorizontalGrid {
    pub fn new(children: Vec<Box<dyn RailroadNode>>) -> Self {
        let mut h = HorizontalGrid {
            children,
            spacing: ARC_RADIUS,
            attributes: collections::HashMap::new(),
        };
        h.attributes
            .insert("class".to_owned(), "horizontalgrid".to_owned());
        h
    }

    pub fn push(&mut self, child: Box<dyn RailroadNode>) -> &mut Self {
        self.children.push(child);
        self
    }

    pub fn into_inner(self) -> Vec<Box<dyn RailroadNode>> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl ::std::iter::FromIterator<Box<dyn RailroadNode>> for HorizontalGrid {
    fn from_iter<T: IntoIterator<Item = Box<dyn RailroadNode>>>(iter: T) -> Self {
        HorizontalGrid::new(iter.into_iter().collect())
    }
}

impl RailroadNode for HorizontalGrid {
    fn entry_height(&self) -> i64 {
        0
    }

    fn height(&self) -> i64 {
        self.children.max_height()
    }

    fn width(&self) -> i64 {
        self.children.total_width() + ((cmp::max(1, self.children.len() as i64) - 1) * self.spacing)
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());
        let mut running_x = x;
        for child in &self.children {
            g.push(child.draw(running_x, y, h_dir));
            running_x += child.width() + self.spacing;
        }
        g.debug("HorizontalGrid", x, y, self)
    }
}

/// A horizontal group of elements, connected from left to right.
///
/// Also see `Stack` for a vertical group of elements.
#[derive(Debug)]
pub struct Sequence {
    children: Vec<Box<dyn RailroadNode>>,
    spacing: i64,
}

impl Sequence {
    pub fn new(children: Vec<Box<dyn RailroadNode>>) -> Sequence {
        Sequence {
            children,
            spacing: 10,
        }
    }

    pub fn push(&mut self, child: Box<dyn RailroadNode>) -> &mut Self {
        self.children.push(child);
        self
    }

    pub fn into_inner(self) -> Vec<Box<dyn RailroadNode>> {
        self.children
    }
}

impl ::std::iter::FromIterator<Box<dyn RailroadNode>> for Sequence {
    fn from_iter<T: IntoIterator<Item = Box<dyn RailroadNode>>>(iter: T) -> Self {
        Sequence::new(iter.into_iter().collect())
    }
}

impl Default for Sequence {
    fn default() -> Self {
        Sequence::new(vec![])
    }
}

impl RailroadNode for Sequence {
    fn entry_height(&self) -> i64 {
        self.children.max_entry_height()
    }

    fn height(&self) -> i64 {
        self.children.max_entry_height() + self.children.max_height_below_entry()
    }

    fn width(&self) -> i64 {
        let l = self.children.len();
        if l > 1 {
            self.children.total_width() + (l - 1) as i64 * self.spacing
        } else {
            self.children.total_width()
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set("class", "sequence");
        for (child, offset) in self.children.running_x(self.spacing) {
            g.push(child.draw(
                x + offset,
                y + self.entry_height() - child.entry_height(),
                h_dir,
            ));
        }

        let mut running_x = x;
        for child in self.children.iter().rev().skip(1).rev() {
            g.push(
                svg::PathData::new(h_dir)
                    .move_to(running_x + child.width(), y + self.exit_height())
                    .horizontal(self.spacing)
                    .into_path(),
            );
            running_x += child.width() + self.spacing;
        }
        g.debug("Sequence", x, y, self)
    }
}

/// A symbol indicating the logical end of a syntax-diagram via two vertical bars.
#[derive(Debug)]
pub struct End;

impl RailroadNode for End {
    fn entry_height(&self) -> i64 {
        10
    }
    fn height(&self) -> i64 {
        20
    }
    fn width(&self) -> i64 {
        20
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        svg::PathData::new(h_dir)
            .move_to(x, y + 10)
            .horizontal(20)
            .move_rel(-10, -10)
            .vertical(20)
            .move_rel(10, -20)
            .vertical(20)
            .into_path()
            .debug("End", x, y, self)
    }
}

/// A symbol indicating the logical start of a syntax-diagram via a circle
#[derive(Debug)]
pub struct SimpleStart;

impl RailroadNode for SimpleStart {
    fn entry_height(&self) -> i64 {
        5
    }
    fn height(&self) -> i64 {
        10
    }
    fn width(&self) -> i64 {
        15
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        svg::PathData::new(h_dir)
            .move_to(x, y + 5)
            .arc(5, svg::Arc::SouthToEast)
            .arc(5, svg::Arc::WestToSouth)
            .arc(5, svg::Arc::NorthToWest)
            .arc(5, svg::Arc::EastToNorth)
            .move_rel(10, 0)
            .horizontal(5)
            .into_path()
            .debug("SimpleStart", x, y, self)
    }
}

/// A symbol indicating the logical end of a syntax-diagram via a circle
#[derive(Debug)]
pub struct SimpleEnd;

impl RailroadNode for SimpleEnd {
    fn entry_height(&self) -> i64 {
        5
    }
    fn height(&self) -> i64 {
        10
    }
    fn width(&self) -> i64 {
        15
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        svg::PathData::new(h_dir)
            .move_to(x, y + 5)
            .horizontal(5)
            .arc(5, svg::Arc::SouthToEast)
            .arc(5, svg::Arc::WestToSouth)
            .arc(5, svg::Arc::NorthToWest)
            .arc(5, svg::Arc::EastToNorth)
            .into_path()
            .debug("SimpleEnd", x, y, self)
    }
}

/// A symbol indicating the logical start of a syntax-diagram via two vertical bars.
#[derive(Debug)]
pub struct Start;

impl RailroadNode for Start {
    fn entry_height(&self) -> i64 {
        10
    }
    fn height(&self) -> i64 {
        20
    }
    fn width(&self) -> i64 {
        20
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        svg::PathData::new(h_dir)
            .move_to(x, y)
            .vertical(20)
            .move_rel(10, -20)
            .vertical(20)
            .move_rel(-10, -10)
            .horizontal(20)
            .into_path()
            .debug("Start", x, y, self)
    }
}

/// A `Terminal`-symbol, drawn as a rectangle with rounded corners.
#[derive(Debug)]
pub struct Terminal {
    label: String,
    attributes: collections::HashMap<String, String>,
}

impl Terminal {
    pub fn new(label: String) -> Self {
        let mut t = Terminal {
            label,
            attributes: collections::HashMap::new(),
        };
        t.attributes
            .insert("class".to_owned(), "terminal".to_owned());
        t
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl RailroadNode for Terminal {
    fn entry_height(&self) -> i64 {
        11
    }
    fn height(&self) -> i64 {
        self.entry_height() * 2
    }
    fn width(&self) -> i64 {
        text_width(&self.label) as i64 * 8 + 20
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        let r = svg::Element::new("rect")
            .set("x", x)
            .set("y", y)
            .set("height", self.height())
            .set("width", self.width())
            .set("rx", 10)
            .set("ry", 10);
        let t = svg::Element::new("text")
            .set("x", x + self.width() / 2)
            .set("y", y + self.entry_height() + 5)
            .text(&self.label);
        svg::Element::new("g")
            .debug("terminal", x, y, self)
            .set_all(self.attributes.iter())
            .add(r)
            .add(t)
    }
}

/// A `NonTerminal`, drawn as a rectangle.
#[derive(Debug)]
pub struct NonTerminal {
    label: String,
    attributes: collections::HashMap<String, String>,
}

impl NonTerminal {
    pub fn new(label: String) -> Self {
        let mut nt = NonTerminal {
            label,
            attributes: collections::HashMap::new(),
        };
        nt.attributes
            .insert("class".to_owned(), "nonterminal".to_owned());
        nt
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl RailroadNode for NonTerminal {
    fn entry_height(&self) -> i64 {
        11
    }
    fn height(&self) -> i64 {
        self.entry_height() * 2
    }
    fn width(&self) -> i64 {
        text_width(&self.label) as i64 * 8 + 20
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("g")
            .debug("NonTerminal", x, y, self)
            .set_all(self.attributes.iter())
            .add(
                svg::Element::new("rect")
                    .set("x", x)
                    .set("y", y)
                    .set("height", self.height())
                    .set("width", self.width()),
            )
            .add(
                svg::Element::new("text")
                    .set("x", x + self.width() / 2)
                    .set("y", y + self.entry_height() + 5)
                    .text(&self.label),
            )
    }
}

/// Wraps another element to make that element logically optional.
///
/// Draws a separate path above, which skips the given element.
#[derive(Debug)]
pub struct Optional<T: RailroadNode> {
    inner: T,
    attributes: collections::HashMap<String, String>,
}

impl<T: RailroadNode> Optional<T> {
    pub fn new(inner: T) -> Self {
        let mut o = Optional {
            inner,
            attributes: collections::HashMap::new(),
        };
        o.attributes
            .insert("class".to_owned(), "optional".to_owned());
        o
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl<T: RailroadNode> AsRef<T> for Optional<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T: RailroadNode> RailroadNode for Optional<T> {
    fn entry_height(&self) -> i64 {
        ARC_RADIUS + cmp::max(ARC_RADIUS, self.inner.entry_height())
    }

    fn height(&self) -> i64 {
        self.entry_height() + self.inner.height_below_entry()
    }

    fn width(&self) -> i64 {
        ARC_RADIUS * 2 + self.inner.width() + ARC_RADIUS * 2
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let i = self.inner.draw(
            x + ARC_RADIUS * 2,
            y + self.entry_height() - self.inner.entry_height(),
            h_dir,
        );

        let v = svg::PathData::new(h_dir)
            .move_to(x, y + self.entry_height())
            .horizontal(ARC_RADIUS * 2)
            .move_rel(-ARC_RADIUS * 2, 0)
            .arc(ARC_RADIUS, svg::Arc::WestToNorth)
            .vertical(cmp::min(0, -self.inner.entry_height() + ARC_RADIUS))
            .arc(ARC_RADIUS, svg::Arc::SouthToEast)
            .horizontal(self.inner.width())
            .arc(ARC_RADIUS, svg::Arc::WestToSouth)
            .vertical(cmp::max(0, self.inner.entry_height() - ARC_RADIUS))
            .arc(ARC_RADIUS, svg::Arc::NorthToEast)
            .horizontal(-ARC_RADIUS * 2)
            .into_path();

        svg::Element::new("g")
            .debug("Optional", x, y, self)
            .set_all(self.attributes.iter())
            .add(v)
            .add(i)
    }
}

/// A vertical group of elements, drawn from top to bottom.
///
/// Also see `Sequence` for a horizontal group of elements.
#[derive(Debug)]
pub struct Stack {
    children: Vec<Box<dyn RailroadNode>>,
    left_padding: i64,
    right_padding: i64,
    spacing: i64,
    attributes: collections::HashMap<String, String>,
}

impl Stack {
    pub fn new(children: Vec<Box<dyn RailroadNode>>) -> Self {
        let mut s = Stack {
            children,
            left_padding: 10,
            right_padding: 10,
            spacing: ARC_RADIUS,
            attributes: collections::HashMap::new(),
        };
        s.attributes.insert("class".to_owned(), "stack".to_owned());
        s
    }

    pub fn push(&mut self, child: impl RailroadNode + 'static) -> &mut Self {
        self.children.push(Box::new(child));
        self
    }

    pub fn into_inner(self) -> Vec<Box<dyn RailroadNode>> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn padded_height(&self, child: &dyn RailroadNode, next_child: &dyn RailroadNode) -> i64 {
        child.entry_height()
            + cmp::max(child.height_below_entry() + self.spacing, ARC_RADIUS * 2)
            + ARC_RADIUS
            + cmp::max(0, ARC_RADIUS - next_child.entry_height())
    }

    fn left_padding(&self) -> i64 {
        if self.children.len() > 1 {
            cmp::max(self.left_padding, ARC_RADIUS)
        } else {
            0
        }
    }

    fn right_padding(&self) -> i64 {
        if self.children.len() > 1 {
            cmp::max(self.right_padding, ARC_RADIUS * 2)
        } else {
            0
        }
    }
}

impl ::std::iter::FromIterator<Box<dyn RailroadNode>> for Stack {
    fn from_iter<T: IntoIterator<Item = Box<dyn RailroadNode>>>(iter: T) -> Self {
        Stack::new(iter.into_iter().collect())
    }
}

impl RailroadNode for Stack {
    fn entry_height(&self) -> i64 {
        self.children.get(0).entry_height()
    }

    fn height(&self) -> i64 {
        self.children
            .iter()
            .zip(self.children.iter().skip(1))
            .map(|(c, nc)| self.padded_height(&c, &nc))
            .sum::<i64>()
            + self.children.last().height()
    }

    fn width(&self) -> i64 {
        let l = self.left_padding() + self.children.max_width() + self.right_padding();
        // If the final upwards connector touches the downward ones, add some space
        if self
            .children
            .iter()
            .rev()
            .skip(1)
            .rev()
            .any(|c| c.width() >= self.children.last().width())
        {
            l + ARC_RADIUS
        } else {
            l
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter()).add(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(self.left_padding())
                .into_path(),
        );

        // Draw all the children but the last
        let mut running_y = y;
        for (child, next_child) in self.children.iter().zip(self.children.iter().skip(1)) {
            g.push(
                svg::PathData::new(h_dir)
                    .move_to(
                        x + self.left_padding() + child.width(),
                        running_y + child.exit_height(),
                    )
                    .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                    .vertical(cmp::max(
                        0,
                        child.height_below_entry() + self.spacing - ARC_RADIUS * 2,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::NorthToWest)
                    .horizontal(-child.width())
                    .arc(ARC_RADIUS, svg::Arc::EastToSouth)
                    .vertical(cmp::max(0, next_child.entry_height() - ARC_RADIUS))
                    .vertical(cmp::max(
                        0,
                        (self.spacing - ARC_RADIUS * 2) / 2 + (self.spacing - ARC_RADIUS * 2) % 2,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                    .horizontal(self.left_padding() - ARC_RADIUS)
                    .into_path(),
            );
            g.push(child.draw(x + self.left_padding(), running_y, h_dir));
            running_y += self.padded_height(&child, &next_child);
        }

        // Draw the last (possibly only) child and its connectors
        if let Some(child) = self.children.last() {
            if self.children.len() > 1 {
                g.push(
                    svg::PathData::new(h_dir)
                        .move_to(
                            x + self.left_padding() + child.width(),
                            running_y + child.exit_height(),
                        )
                        .horizontal(
                            self.width() - child.width() - self.left_padding() - ARC_RADIUS * 2,
                        )
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(
                            -self.height()
                                + child.height_below_entry()
                                + ARC_RADIUS * 2
                                + self.exit_height(),
                        )
                        .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                        .into_path(),
                );
            }
            g.push(child.draw(x + self.left_padding(), running_y, h_dir));
        }

        g.debug("Stack", x, y, self)
    }
}

/// A container of elements, drawn vertically, where exactly one element has to be picked
///
/// Use `Empty` as one of the elements to make the entire `Choice` optional (a shorthand for
/// `Optional(Choice(..))`.
#[derive(Debug)]
pub struct Choice {
    children: Vec<Box<dyn RailroadNode>>,
    spacing: i64,
    attributes: collections::HashMap<String, String>,
}

impl ::std::iter::FromIterator<Box<dyn RailroadNode>> for Choice {
    fn from_iter<T: IntoIterator<Item = Box<dyn RailroadNode>>>(iter: T) -> Self {
        Choice::new(iter.into_iter().collect())
    }
}

impl Choice {
    pub fn new(children: Vec<Box<dyn RailroadNode>>) -> Self {
        let mut c = Choice {
            children,
            spacing: 10,
            attributes: collections::HashMap::new(),
        };
        c.attributes.insert("class".to_owned(), "choice".to_owned());
        c
    }

    pub fn push(&mut self, child: impl RailroadNode + 'static) -> &mut Self {
        self.children.push(Box::new(child));
        self
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn inner_padding(&self) -> i64 {
        if self.children.len() > 1 {
            ARC_RADIUS * 2
        } else {
            0
        }
    }

    pub fn into_inner(self) -> Vec<Box<dyn RailroadNode>> {
        self.children
    }

    fn padded_height(&self, child: &dyn RailroadNode) -> i64 {
        cmp::max(ARC_RADIUS, child.entry_height()) + child.height_below_entry() + self.spacing
    }
}

impl Default for Choice {
    fn default() -> Self {
        Choice::new(vec![])
    }
}

impl RailroadNode for Choice {
    fn entry_height(&self) -> i64 {
        self.children.get(0).entry_height()
    }

    fn height(&self) -> i64 {
        if self.children.is_empty() {
            0
        } else if self.children.len() == 1 {
            self.children.total_height()
        } else {
            self.entry_height()
                + cmp::max(
                    ARC_RADIUS,
                    self.spacing + self.children[0].height_below_entry(),
                )
                + self
                    .children
                    .iter()
                    .skip(1)
                    .map(|c| self.padded_height(&c))
                    .sum::<i64>()
                - self.spacing
        }
    }

    fn width(&self) -> i64 {
        if self.children.len() > 1 {
            self.inner_padding() + self.children.max_width() + self.inner_padding()
        } else {
            self.children.max_width()
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());

        // The top, horizontal connectors
        g.push(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(self.inner_padding())
                .move_rel(self.children.get(0).width(), 0)
                .horizontal(self.width() - self.inner_padding() - self.children.get(0).width())
                .into_path(),
        );

        // The first child is simply drawn in-line
        if let Some(child) = self.children.get(0) {
            g.push(child.draw(x + self.inner_padding(), y, h_dir));
        }

        // If there are more children, we draw all kinds of things
        if self.children.len() > 1 {
            // The downward arcs
            g.push(
                svg::PathData::new(h_dir)
                    .move_to(x, y + self.entry_height())
                    .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                    .vertical(cmp::max(
                        0,
                        self.children[0].height_below_entry() + self.spacing - ARC_RADIUS,
                    ))
                    .move_rel(self.width() - ARC_RADIUS * 2, 0)
                    .vertical(-cmp::max(
                        0,
                        self.children[0].height_below_entry() + self.spacing - ARC_RADIUS,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                    .into_path(),
            );

            // The downward connectors, drawn individually
            //let mut running_y = y + self.padded_height(&self.children.get(0));
            let mut running_y = y
                + self.entry_height()
                + cmp::max(
                    ARC_RADIUS,
                    self.spacing + self.children[0].height_below_entry(),
                );
            for child in self.children.iter().skip(1).rev().skip(1).rev() {
                let z = self.padded_height(&child);
                let zz = cmp::max(0, child.entry_height() - ARC_RADIUS);
                let z = z - zz;
                g.push(
                    svg::PathData::new(h_dir)
                        .move_to(x + ARC_RADIUS, running_y + zz)
                        .vertical(z)
                        .move_rel(self.width() - ARC_RADIUS * 2, 0)
                        .vertical(-z)
                        .into_path(),
                );
                running_y += z + zz;
            }

            // The children and arcs around them
            let mut running_y = y
                + self.entry_height()
                + cmp::max(
                    ARC_RADIUS,
                    self.spacing + self.children[0].height_below_entry(),
                );
            for child in self.children.iter().skip(1) {
                g.push(
                    svg::PathData::new(h_dir)
                        .move_to(x + ARC_RADIUS, running_y)
                        .vertical(cmp::max(0, child.entry_height() - ARC_RADIUS))
                        .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                        .move_rel(child.width(), 0)
                        .horizontal(self.children.max_width() - child.width())
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(-cmp::max(0, child.entry_height() - ARC_RADIUS))
                        .into_path(),
                );
                g.push(child.draw(
                    x + ARC_RADIUS * 2,
                    running_y + cmp::max(0, ARC_RADIUS - child.entry_height()),
                    h_dir,
                ));
                running_y += self.padded_height(&child);
            }
        }

        g.debug("Choice", x, y, self)
    }
}

/// Wraps one element by providing a backwards-path through another element.
#[derive(Debug)]
pub struct Repeat<I: RailroadNode, R: RailroadNode> {
    inner: I,
    repeat: R,
    spacing: i64,
    attributes: collections::HashMap<String, String>,
}

impl<I, R> Repeat<I, R>
where
    I: RailroadNode,
    R: RailroadNode,
{
    pub fn new(inner: I, repeat: R) -> Self {
        let mut r = Repeat {
            inner,
            repeat,
            spacing: 10,
            attributes: collections::HashMap::new(),
        };
        r.attributes.insert("class".to_owned(), "repeat".to_owned());
        r
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn height_between_entries(&self) -> i64 {
        cmp::max(
            ARC_RADIUS * 2,
            self.inner.height_below_entry() + self.spacing + self.repeat.entry_height(),
        )
    }
}

impl<I, R> RailroadNode for Repeat<I, R>
where
    I: RailroadNode,
    R: RailroadNode,
{
    fn entry_height(&self) -> i64 {
        self.inner.entry_height()
    }

    fn height(&self) -> i64 {
        self.inner.entry_height() + self.height_between_entries() + self.repeat.height_below_entry()
    }

    fn width(&self) -> i64 {
        ARC_RADIUS + cmp::max(self.repeat.width(), self.inner.width()) + ARC_RADIUS
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());

        g.push(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(ARC_RADIUS)
                .move_rel(self.inner.width(), 0)
                .horizontal(cmp::max(
                    ARC_RADIUS,
                    self.repeat.width() - self.inner.width() + ARC_RADIUS,
                ))
                .move_rel(-ARC_RADIUS, 0)
                .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                .vertical(self.height_between_entries() - ARC_RADIUS * 2)
                .arc(ARC_RADIUS, svg::Arc::NorthToWest)
                .move_rel(-self.repeat.width(), 0)
                .horizontal(cmp::min(0, self.repeat.width() - self.inner.width()))
                .arc(ARC_RADIUS, svg::Arc::EastToNorth)
                .vertical(-self.height_between_entries() + ARC_RADIUS * 2)
                .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                .into_path(),
        )
        .push(self.repeat.draw(
            x + self.width() - self.repeat.width() - ARC_RADIUS,
            y + self.height() - self.repeat.height_below_entry() - self.repeat.entry_height(),
            h_dir.invert(),
        ));
        g.push(self.inner.draw(x + ARC_RADIUS, y, h_dir));
        g.debug("Repeat", x, y, self)
    }
}

/// A rectangle drawn with the given dimensions, used for visual debugging
#[derive(Debug)]
#[doc(hidden)]
pub struct Debug {
    entry_height: i64,
    height: i64,
    width: i64,
    attributes: collections::HashMap<String, String>,
}

impl Debug {
    pub fn new(entry_height: i64, height: i64, width: i64) -> Self {
        assert!(entry_height < height);
        let mut d = Debug {
            entry_height,
            height,
            width,
            attributes: collections::HashMap::new(),
        };

        d.attributes.insert("class".to_owned(), "debug".to_owned());
        d.attributes.insert(
            "style".to_owned(),
            "fill: hsla(0, 100%, 90%, 0.9); stroke-width: 2; stroke: red".to_owned(),
        );
        d
    }
}

impl RailroadNode for Debug {
    fn entry_height(&self) -> i64 {
        self.entry_height
    }
    fn height(&self) -> i64 {
        self.height
    }
    fn width(&self) -> i64 {
        self.width
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("rect")
            .set("x", x)
            .set("y", y)
            .set("height", self.height())
            .set("width", self.width())
            .set_all(self.attributes.iter())
            .debug("Debug", x, y, self)
    }
}

/// A dummy-element which has no size and draws nothing.
///
/// This can be used in conjunction with `Choice` (to indicate that one of the options
/// is blank, a shorthand for an `Optional(Choice)`), `Repeat` (if there are
/// zero-or-more repetitions or if there is no joining element), or `LabeledBox`
/// (if the label should be empty).
#[derive(Debug)]
pub struct Empty;

impl RailroadNode for Empty {
    fn entry_height(&self) -> i64 {
        0
    }
    fn height(&self) -> i64 {
        0
    }
    fn width(&self) -> i64 {
        0
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("g").debug("Empty", x, y, self)
    }
}

/// A box drawn around the given element and a label placed inside the box, above the element.
///
/// You may want to use `Comment` or `Empty` for the label.
#[derive(Debug)]
pub struct LabeledBox<T: RailroadNode, U: RailroadNode> {
    inner: T,
    label: U,
    spacing: i64,
    padding: i64,
    attributes: collections::HashMap<String, String>,
}

impl<T: RailroadNode> LabeledBox<T, Empty> {
    /// Construct a box with a label set to `Empty`
    pub fn without_label(inner: T) -> Self {
        LabeledBox::new(inner, Empty)
    }
}

impl<T: RailroadNode, U: RailroadNode> LabeledBox<T, U> {
    pub fn new(inner: T, label: U) -> Self {
        let mut l = LabeledBox {
            inner,
            label,
            spacing: 8,
            padding: 8,
            attributes: collections::HashMap::new(),
        };
        l.attributes
            .insert("class".to_owned(), "labeledbox".to_owned());
        l
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn spacing(&self) -> i64 {
        if self.label.height() > 0 {
            self.spacing
        } else {
            0
        }
    }

    fn padding(&self) -> i64 {
        if self.label.height() + self.inner.height() + self.label.width() + self.inner.width() > 0 {
            self.padding
        } else {
            0
        }
    }
}

impl<T: RailroadNode, U: RailroadNode> RailroadNode for LabeledBox<T, U> {
    fn entry_height(&self) -> i64 {
        self.padding() + self.label.height() + self.spacing() + self.inner.entry_height()
    }

    fn height(&self) -> i64 {
        self.padding() + self.label.height() + self.spacing() + self.inner.height() + self.padding()
    }

    fn width(&self) -> i64 {
        self.padding() + cmp::max(self.inner.width(), self.label.width()) + self.padding()
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        svg::Element::new("g")
            .add(
                svg::Element::new("rect")
                    .set("x", x)
                    .set("y", y)
                    .set("height", self.height())
                    .set("width", self.width()),
            )
            .add(
                svg::PathData::new(h_dir)
                    .move_to(x, y + self.entry_height())
                    .horizontal(self.padding())
                    .move_rel(self.inner.width(), 0)
                    .horizontal(self.width() - self.inner.width() - self.padding())
                    .into_path(),
            )
            .add(
                self.label
                    .draw(x + self.padding(), y + self.padding(), h_dir),
            )
            .add(self.inner.draw(
                x + self.padding(),
                y + self.padding() + self.label.height() + self.spacing(),
                h_dir,
            ))
            .set_all(self.attributes.iter())
            .debug("LabeledBox", x, y, self)
    }
}

/// A label / verbatim text, drawn in-line
#[derive(Debug)]
pub struct Comment {
    text: String,
    attributes: collections::HashMap<String, String>,
}

impl Comment {
    pub fn new(text: String) -> Self {
        let mut c = Comment {
            text,
            attributes: collections::HashMap::new(),
        };
        c.attributes
            .insert("class".to_owned(), "comment".to_owned());
        c
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl RailroadNode for Comment {
    fn entry_height(&self) -> i64 {
        10
    }
    fn height(&self) -> i64 {
        20
    }
    fn width(&self) -> i64 {
        text_width(&self.text) as i64 * 7 + 10
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("text")
            .set_all(self.attributes.iter())
            .set("x", x + self.width() / 2)
            .set("y", y + 15)
            .text(&self.text)
            .debug("Comment", x, y, self)
    }
}

#[derive(Debug)]
pub struct Diagram<T: RailroadNode> {
    root: T,
    extra_attributes: collections::HashMap<String, String>,
    extra_elements: Vec<svg::Element>,
    left_padding: i64,
    right_padding: i64,
    top_padding: i64,
    bottom_padding: i64,
}

impl<T: RailroadNode> Diagram<T> {
    /// Create a diagram using the given root-element.
    ///
    /// ```
    /// let mut seq = railroad::Sequence::default();
    /// seq.push(Box::new(railroad::Start))
    ///    .push(Box::new(railroad::Terminal::new("Foobar".to_owned())))
    ///    .push(Box::new(railroad::End));
    /// let mut dia = railroad::Diagram::new(seq);
    /// dia.add_element(railroad::svg::Element::new("style")
    ///                 .set("type", "text/css")
    ///                 .raw_text(railroad::DEFAULT_CSS));
    /// println!("{}", dia);
    /// ```
    pub fn new(root: T) -> Self {
        Diagram {
            root,
            extra_attributes: collections::HashMap::new(),
            extra_elements: vec![],
            left_padding: 10,
            right_padding: 10,
            top_padding: 10,
            bottom_padding: 10,
        }
    }

    /// Create a diagram which has this library's default CSS style included.
    pub fn with_default_css(root: T) -> Self {
        let mut dia = Self::new(root);
        dia.add_default_css();
        dia
    }

    /// Add the default CSS as an additional `<style>` element.
    pub fn add_default_css(&mut self) {
        self.add_element(
            svg::Element::new("style")
                .set("type", "text/css")
                .raw_text(DEFAULT_CSS),
        );
    }

    /// Set an attribute on the <svg>-tag.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl ToString) -> &mut Self {
        self.extra_attributes.insert(key.into(), value.to_string());
        self
    }

    /// Add an additional `svg::Element` which is written before the root-element
    pub fn add_element(&mut self, e: svg::Element) -> &mut Self {
        self.extra_elements.push(e);
        self
    }

    pub fn write(&self, writer: &mut impl io::Write) -> io::Result<()> {
        write!(writer, "{}", self.draw(0, 0, HDir::LTR))
    }

    /// Return the root-element this diagram's root element
    pub fn into_inner(self) -> T {
        self.root
    }
}

impl<T: RailroadNode> RailroadNode for Diagram<T> {
    fn entry_height(&self) -> i64 {
        0
    }

    fn height(&self) -> i64 {
        self.top_padding + self.root.height() + self.bottom_padding
    }

    fn width(&self) -> i64 {
        self.left_padding + self.root.width() + self.right_padding
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut e = svg::Element::new("svg")
            .set("xmlns", "http://www.w3.org/2000/svg")
            .set("xmlns:xlink", "http://www.w3.org/1999/xlink")
            .set("class", "railroad")
            .set("viewBox", format!("0 0 {} {}", self.width(), self.height()));
        for (k, v) in &self.extra_attributes {
            e = e.set(k.clone(), v.clone());
        }
        for extra_ele in self.extra_elements.iter().cloned() {
            e = e.add(extra_ele);
        }
        e.add(
            self.root
                .draw(x + self.left_padding, y + self.top_padding, h_dir),
        )
    }
}

impl<T: RailroadNode> fmt::Display for Diagram<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.draw(0, 0, HDir::LTR))
    }
}
