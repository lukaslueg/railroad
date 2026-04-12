// MIT License
//
// Copyright (c) Lukas Lueg (lukas.lueg@gmail.com)
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
//! Using this library, diagrams are created by primitives which implemented `Node`.
//! Primitives are combined into more complex strctures by wrapping simple elements into more
//! complex ones.
//!
//! ```rust
//! use railroad::*;
//!
//! // This diagram will be a (horizontal) sequence of simple elements
//! let mut seq: Sequence<Box<dyn Node>> = Sequence::default();
//! seq.push(Box::new(Start))
//!    .push(Box::new(Terminal::new("BEGIN".to_owned())))
//!    .push(Box::new(NonTerminal::new("syntax".to_owned())))
//!    .push(Box::new(End));
//!
//! // The library only computes the diagram's geometry; we use CSS for layout.
//! let mut dia = Diagram::new_with_stylesheet(seq, &Stylesheet::Light);
//!
//! // A `Node`'s `fmt::Display` is its SVG.
//! println!("<html>{}</html>", dia);
//!
//! // For direct streaming, render into `svg::Renderer`.
//! let mut streamed = String::new();
//! let mut renderer = svg::Renderer::new(&mut streamed);
//! dia.render(&mut renderer, 0, 0, svg::HDir::LTR).unwrap();
//! assert!(streamed.starts_with("<svg"));
//! ```
//!
//! ## Implementing custom nodes
//!
//! Downstream crates can implement [`Node`] directly for custom primitives.
//! The main rule is simple: a node must only draw within the geometry it
//! advertises. If a node reports `width()`, `height()`, and `entry_height()`,
//! its drawing must stay inside that box and keep its connecting path at
//! `y + entry_height()`.
//!
//! For simple leaf nodes, implementing `entry_height()`, `height()`, `width()`,
//! and [`Node::draw`] is usually enough; the provided geometry-aware methods are
//! correct by default. For composite nodes that position child nodes, override
//! [`Node::compute_geometry`] and usually also [`Node::draw_with_geometry`] and
//! [`Node::render_with_geometry`] so child geometry is computed once and reused
//! during rendering.

use std::{
    collections::{self, HashMap},
    fmt, io,
};

pub mod notactuallysvg;
pub use crate::notactuallysvg as svg;
use crate::svg::HDir;
mod nodes;
pub use crate::nodes::containers::{Choice, Sequence, Stack};
pub use crate::nodes::grids::{HorizontalGrid, VerticalGrid};
pub use crate::nodes::text::{Comment, NonTerminal, Terminal};
pub use crate::nodes::wrappers::{LabeledBox, Link, LinkTarget, Optional, Repeat};

#[cfg(feature = "resvg")]
pub mod render;

#[cfg(feature = "resvg")]
pub use resvg;

#[doc = include_str!("../README.md")]
#[allow(dead_code)]
type _READMETEST = ();

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

/// Pre-defined stylesheets
/// ```rust
/// use railroad::*;
///
/// let mut seq: Sequence::<Box<dyn Node>> = Sequence::default();
/// seq.push(Box::new(Start))
///    .push(Box::new(Terminal::new("Foobar".to_owned())))
///    .push(Box::new(End));
///
/// let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::Light);
/// println!("{}", dia);
/// ```
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[non_exhaustive]
pub enum Stylesheet {
    /// The default stylesheet
    #[default]
    Light,
    Dark,
    /// Variation of the `Light`-theme, compatible with what can be rendered when using `resvg`.
    LightRendersafe,
    /// Variation of the `Dark`-theme, compatible with what can be rendered when using `resvg`.
    DarkRendersafe,
}

impl Stylesheet {
    /// Switch this stylesheet to it's "dark" variant, preserving render-safety.
    #[must_use]
    pub const fn to_dark(&self) -> Self {
        match self {
            Self::Light | Self::Dark => Self::Dark,
            Self::LightRendersafe | Self::DarkRendersafe => Self::DarkRendersafe,
        }
    }

    /// Switch this stylesheet to it's "light" variant, preserving render-safety.
    #[must_use]
    pub const fn to_light(&self) -> Self {
        match self {
            Self::Light | Self::Dark => Self::Light,
            Self::LightRendersafe | Self::DarkRendersafe => Self::LightRendersafe,
        }
    }

    /// Returns `True` if this stylesheet is of a "light" variant.
    #[must_use]
    pub const fn is_light(&self) -> bool {
        matches!(self, Self::Light | Self::LightRendersafe)
    }

    /// The CSS for this stylesheet.
    #[must_use]
    pub const fn stylesheet(self) -> &'static str {
        match self {
            Self::Light => include_str!("stylesheet_light.css"),
            Self::Dark => include_str!("stylesheet_dark.css"),
            Self::LightRendersafe => include_str!("stylesheet_light_safe.css"),
            Self::DarkRendersafe => include_str!("stylesheet_dark_safe.css"),
        }
    }
}

/// Default Cascading Style Sheets for the resuling SVG.
pub const DEFAULT_CSS: &str = Stylesheet::Light.stylesheet();

/// Pre-computed geometry for a node and its entire subtree.
///
/// This is a transient value created by [`Node::compute_geometry`] and passed into
/// [`Node::draw_with_geometry`]. It is never stored inside a node struct; it exists
/// only on the call stack during the draw phase and is dropped when drawing completes.
///
/// The `children` vec mirrors the order in which each composite node iterates its
/// children during drawing, so `children[i]` corresponds to the i-th child drawn.
/// For single-child wrappers (`Optional`, `Link`) `children[0]` is the inner node.
/// For `LabeledBox`, `children[0]` is the inner node and `children[1]` is the label.
/// For `Repeat`, `children[0]` is the inner node and `children[1]` is the repeat node.
/// Leaf nodes have an empty `children` vec.
#[derive(Debug, Clone)]
pub struct NodeGeometry {
    /// The vertical distance from this node's top edge to its connecting path.
    pub entry_height: i64,
    /// The total height of this node's bounding box.
    pub height: i64,
    /// The total width of this node's bounding box.
    pub width: i64,
    /// Pre-computed geometry for each child, in draw order.
    pub children: Vec<NodeGeometry>,
}

impl NodeGeometry {
    /// The vertical distance from the connecting path to the bottom of this node.
    ///
    /// Equivalent to `height - entry_height`.
    #[must_use]
    pub fn height_below_entry(&self) -> i64 {
        self.height - self.entry_height
    }
}

/// A diagram primitive that participates in layout and SVG generation.
///
/// Every `Node` advertises a rectangular geometry and a single horizontal entry
/// line inside that rectangle. Parent nodes use that geometry to position child
/// nodes, so correctness depends on each implementation keeping its drawing
/// inside the geometry it reports:
///
/// - `width()` and `height()` define the full bounding box,
/// - `entry_height()` defines the vertical offset of the connecting path,
/// - drawing at `(x, y)` must stay inside `x..x + width()` and `y..y + height()`,
/// - and the path that enters or leaves the node must be aligned with
///   `y + entry_height()`.
///
/// For simple leaf nodes, implementing [`Node::entry_height`], [`Node::height`],
/// [`Node::width`], and [`Node::draw`] is usually enough. The default
/// implementations of the geometry-aware methods are correct, just not always
/// optimal.
///
/// Composite nodes that contain child nodes should usually override
/// [`Node::compute_geometry`] so child geometry is computed once in a bottom-up
/// pass, then override [`Node::draw_with_geometry`] and often
/// [`Node::render_with_geometry`] to reuse that cached geometry during drawing.
pub trait Node {
    /// The vertical distance from this element's top to where the entering,
    /// connecting path is drawn.
    ///
    /// By convention, the path connecting primitives enters from the left.
    /// Parent nodes align children by placing their connecting path at
    /// `y + entry_height()`, so this value must match where the node actually
    /// expects its incoming and outgoing path segments.
    fn entry_height(&self) -> i64;

    /// This primitive's total height.
    ///
    /// Together with [`Node::width`], this defines the full bounding box the
    /// node may occupy when drawn.
    fn height(&self) -> i64;

    /// This primitive's total width.
    ///
    /// The node must not draw outside the horizontal range implied by this
    /// value when positioned by a parent node.
    fn width(&self) -> i64;

    /// The vertical distance from the height of the connecting path to the bottom.
    ///
    /// Equivalent to `height() - entry_height()`.
    ///
    /// This is a convenience method for parent nodes that need to align child
    /// nodes relative to the connecting path. Implementors normally should not
    /// override it unless they also change the meaning of the basic geometry
    /// methods.
    fn height_below_entry(&self) -> i64 {
        self.height() - self.entry_height()
    }

    /// Draw this element as an `svg::Element` at the given position and direction.
    ///
    /// The element must fit entirely within the bounding box defined by `(x, y)`,
    /// `width()`, and `height()`, with the connecting path at `y + entry_height()`.
    ///
    /// For many downstream leaf nodes, this is the only drawing method that must
    /// be implemented directly. The default geometry-aware methods delegate back
    /// to it.
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element;

    /// Compute geometry for this node and its entire subtree in a single bottom-up pass.
    ///
    /// The returned [`NodeGeometry`] is a transient value intended to be passed to
    /// [`Node::draw_with_geometry`]; it is not stored inside the node.
    ///
    /// The default implementation is correct for leaf nodes because it simply
    /// records this node's advertised geometry and assumes there are no children.
    ///
    /// Composite nodes should override this to recurse into their children and
    /// store child geometry in [`NodeGeometry::children`]. That lets parent and
    /// child rendering share one cached geometry pass instead of repeatedly
    /// calling `entry_height()`, `height()`, and `width()` throughout the tree.
    fn compute_geometry(&self) -> NodeGeometry {
        NodeGeometry {
            entry_height: self.entry_height(),
            height: self.height(),
            width: self.width(),
            children: vec![],
        }
    }

    /// Draw this element using pre-computed geometry, avoiding redundant geometry
    /// recomputation for deeply nested structures.
    ///
    /// `geo` holds the cached dimensions for *this* node. Composite nodes should
    /// read child geometry from `geo.children[i]` and pass it to each child's
    /// `draw_with_geometry` call, rather than recomputing child geometry through
    /// repeated calls to `entry_height()`, `height()`, and `width()`.
    ///
    /// The default implementation falls back to [`Node::draw`], which is correct
    /// for all nodes. Leaf nodes usually do not need to override this. Composite
    /// nodes should usually override it so the cached geometry from
    /// [`Node::compute_geometry`] is actually used.
    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, _geo: &NodeGeometry) -> svg::Element {
        self.draw(x, y, h_dir)
    }

    /// Render this element directly into an SVG renderer.
    ///
    /// This is the streaming counterpart to [`Node::draw`]. The default
    /// implementation computes geometry once and forwards to
    /// [`Node::render_with_geometry`].
    ///
    /// Implementors typically do not override this method directly. Instead,
    /// override [`Node::render_with_geometry`] if a custom streaming
    /// implementation is worthwhile.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let node = Terminal::new("item".to_owned());
    /// let mut out = String::new();
    /// let mut renderer = svg::Renderer::new(&mut out);
    /// node.render(&mut renderer, 0, 0, svg::HDir::LTR).unwrap();
    /// assert!(out.contains("<text"));
    /// assert!(out.contains("item"));
    /// ```
    fn render(&self, out: &mut svg::Renderer<'_>, x: i64, y: i64, h_dir: HDir) -> fmt::Result {
        let geo = self.compute_geometry();
        self.render_with_geometry(out, x, y, h_dir, &geo)
    }

    /// Render this element using pre-computed geometry.
    ///
    /// Override this for node implementations that want to stream SVG directly
    /// without first materializing an intermediate [`svg::Element`] tree. The
    /// default implementation preserves compatibility by serializing the result of
    /// [`Node::draw_with_geometry`].
    ///
    /// Leaf nodes can often keep the default implementation. Composite nodes or
    /// performance-sensitive nodes should usually override this together with
    /// [`Node::draw_with_geometry`] so both rendering paths consume the same
    /// cached geometry instead of rebuilding equivalent intermediate structures.
    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        out.write_display(self.draw_with_geometry(x, y, h_dir, geo))
    }
}

impl fmt::Debug for dyn Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("entry_height", &self.entry_height())
            .field("height", &self.height())
            .field("width", &self.width())
            .finish()
    }
}

macro_rules! deref_impl {
    ($($sig:tt)+) => {
        impl $($sig)+ {
            fn entry_height(&self) -> i64 {
                (**self).entry_height()
            }

            fn height(&self) -> i64 {
                (**self).height()
            }

            fn width(&self) -> i64 {
                (**self).width()
            }

            fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
                (**self).draw(x, y, h_dir)
            }

            fn compute_geometry(&self) -> NodeGeometry {
                (**self).compute_geometry()
            }

            fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
                (**self).draw_with_geometry(x, y, h_dir, geo)
            }

            fn render_with_geometry(
                &self,
                out: &mut svg::Renderer<'_>,
                x: i64,
                y: i64,
                h_dir: HDir,
                geo: &NodeGeometry,
            ) -> fmt::Result {
                (**self).render_with_geometry(out, x, y, h_dir, geo)
            }
        }
    };
}
deref_impl!(<'a, N> Node for &'a N where N: Node + ?Sized);
deref_impl!(<'a, N> Node for &'a mut N where N: Node + ?Sized);
deref_impl!(<N> Node for Box<N> where N: Node + ?Sized);
deref_impl!(<N> Node for std::rc::Rc<N> where N: Node + ?Sized);
deref_impl!(<N> Node for std::sync::Arc<N> where N: Node + ?Sized);

#[cfg(feature = "visual-debug")]
fn add_debug_attrs(
    tag: &mut svg::StartTag<'_, '_>,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
) -> fmt::Result {
    tag.attr("railroad:type", name)?;
    tag.attr("railroad:x", x)?;
    tag.attr("railroad:y", y)?;
    tag.attr("railroad:entry_height", geo.entry_height)?;
    tag.attr("railroad:height", geo.height)?;
    tag.attr("railroad:width", geo.width)
}

#[cfg(not(feature = "visual-debug"))]
fn add_debug_attrs(
    _tag: &mut svg::StartTag<'_, '_>,
    _name: &str,
    _x: i64,
    _y: i64,
    _geo: &NodeGeometry,
) -> fmt::Result {
    Ok(())
}

#[cfg(feature = "visual-debug")]
fn write_debug_overlay(
    out: &mut svg::Renderer<'_>,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
) -> fmt::Result {
    out.path_with_class(
        &svg::PathData::new(HDir::LTR)
            .move_to(x, y)
            .horizontal(geo.width)
            .vertical(5)
            .move_rel(-geo.width, -5)
            .vertical(geo.height)
            .horizontal(5)
            .move_rel(-5, -geo.height)
            .move_rel(0, geo.entry_height)
            .horizontal(10),
        "debug",
    )
}

#[cfg(not(feature = "visual-debug"))]
fn write_debug_overlay(
    _out: &mut svg::Renderer<'_>,
    _x: i64,
    _y: i64,
    _geo: &NodeGeometry,
) -> fmt::Result {
    Ok(())
}

/// Internal rendering surface shared by the `svg::Element` and streaming backends.
///
/// Built-in nodes express their geometry-aware draw order in terms of this trait
/// so the crate can keep `draw_with_geometry()` and `render_with_geometry()`
/// behavior in sync without duplicating traversal logic.
trait RenderBackend {
    /// Append a path element to the current output.
    fn push_path(&mut self, path: svg::PathData) -> fmt::Result;

    /// Append an axis-aligned rectangle to the current output.
    fn push_rect(&mut self, x: i64, y: i64, width: i64, height: i64) -> fmt::Result;

    /// Append a rounded rectangle to the current output.
    fn push_rounded_rect(
        &mut self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        radius: i64,
    ) -> fmt::Result;

    /// Append a centered text element at the given coordinates.
    fn push_text(&mut self, x: i64, y: i64, text: &str) -> fmt::Result;

    /// Append a child node using cached geometry.
    fn push_child<N: Node + ?Sized>(
        &mut self,
        child: &N,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result;
}

/// `RenderBackend` implementation that accumulates child `svg::Element`s.
///
/// This powers the compatibility `draw_with_geometry()` path.
#[derive(Default)]
struct ElementBackend {
    children: Vec<svg::Element>,
}

impl ElementBackend {
    /// Wrap the accumulated children in a `<g>` element with debug metadata.
    ///
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use railroad::{NodeGeometry, notactuallysvg as svg};
    /// # use railroad::HDir;
    /// let mut backend = ElementBackend::default();
    /// backend.push_path(svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(10)).unwrap();
    /// let group = backend.finish_group(
    ///     &HashMap::new(),
    ///     "demo",
    ///     0,
    ///     0,
    ///     &NodeGeometry { entry_height: 0, height: 0, width: 0, children: vec![] },
    /// );
    /// assert!(group.to_string().starts_with("<g"));
    /// ```
    fn finish_group(
        self,
        attrs: &HashMap<String, String>,
        name: &str,
        x: i64,
        y: i64,
        geo: &NodeGeometry,
    ) -> svg::Element {
        let mut group = svg::Element::new("g").set_all(attrs.iter());
        for child in self.children {
            group.push(child);
        }
        group.debug_with_geometry(name, x, y, geo)
    }
}

impl RenderBackend for ElementBackend {
    fn push_path(&mut self, path: svg::PathData) -> fmt::Result {
        self.children.push(path.into_path());
        Ok(())
    }

    fn push_rect(&mut self, x: i64, y: i64, width: i64, height: i64) -> fmt::Result {
        self.children.push(
            svg::Element::new("rect")
                .set("x", &x)
                .set("y", &y)
                .set("height", &height)
                .set("width", &width),
        );
        Ok(())
    }

    fn push_rounded_rect(
        &mut self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        radius: i64,
    ) -> fmt::Result {
        self.children.push(
            svg::Element::new("rect")
                .set("x", &x)
                .set("y", &y)
                .set("height", &height)
                .set("width", &width)
                .set("rx", &radius)
                .set("ry", &radius),
        );
        Ok(())
    }

    fn push_text(&mut self, x: i64, y: i64, text: &str) -> fmt::Result {
        self.children.push(
            svg::Element::new("text")
                .set("x", &x)
                .set("y", &y)
                .text(text),
        );
        Ok(())
    }

    fn push_child<N: Node + ?Sized>(
        &mut self,
        child: &N,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        self.children
            .push(child.draw_with_geometry(x, y, h_dir, geo));
        Ok(())
    }
}

/// `RenderBackend` implementation that streams directly into `svg::Renderer`.
struct RendererBackend<'a, 'b> {
    out: &'a mut svg::Renderer<'b>,
}

impl RenderBackend for RendererBackend<'_, '_> {
    fn push_path(&mut self, path: svg::PathData) -> fmt::Result {
        self.out.path(&path)
    }

    fn push_rect(&mut self, x: i64, y: i64, width: i64, height: i64) -> fmt::Result {
        let mut rect = self.out.start_element("rect")?;
        rect.attr("x", x)?;
        rect.attr("y", y)?;
        rect.attr("height", height)?;
        rect.attr("width", width)?;
        rect.finish_empty()
    }

    fn push_rounded_rect(
        &mut self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        radius: i64,
    ) -> fmt::Result {
        let mut rect = self.out.start_element("rect")?;
        rect.attr("x", x)?;
        rect.attr("y", y)?;
        rect.attr("height", height)?;
        rect.attr("width", width)?;
        rect.attr("rx", radius)?;
        rect.attr("ry", radius)?;
        rect.finish_empty()
    }

    fn push_text(&mut self, x: i64, y: i64, text: &str) -> fmt::Result {
        self.out.text_element("text", text, |tag| {
            tag.attr("x", x)?;
            tag.attr("y", y)
        })
    }

    fn push_child<N: Node + ?Sized>(
        &mut self,
        child: &N,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        child.render_with_geometry(self.out, x, y, h_dir, geo)
    }
}

/// Build a debug-aware `<g>` element from a shared emit closure.
///
/// This is the `svg::Element` counterpart to `render_group_with_geometry`.
///
/// ```ignore
/// # use std::collections::HashMap;
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let group = draw_group_with_geometry(
///     &HashMap::new(),
///     "demo",
///     0,
///     0,
///     &NodeGeometry { entry_height: 0, height: 0, width: 10, children: vec![] },
///     |backend| backend.push_path(svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(10)),
/// );
/// assert!(group.to_string().contains("<path"));
/// ```
fn draw_group_with_geometry(
    attrs: &HashMap<String, String>,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    emit: impl FnOnce(&mut ElementBackend) -> fmt::Result,
) -> svg::Element {
    let mut backend = ElementBackend::default();
    emit(&mut backend).expect("element backend is infallible");
    backend.finish_group(attrs, name, x, y, geo)
}

/// Stream a debug-aware `<g>` element from a shared emit closure.
///
/// This helper keeps the streaming wrapper logic identical across nodes.
///
/// ```ignore
/// # use std::{collections::HashMap, fmt};
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let mut out = String::new();
/// let mut renderer = svg::Renderer::new(&mut out);
/// render_group_with_geometry(
///     &mut renderer,
///     &HashMap::new(),
///     "demo",
///     0,
///     0,
///     &NodeGeometry { entry_height: 0, height: 0, width: 10, children: vec![] },
///     |backend| backend.push_path(svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(10)),
/// ).unwrap();
/// assert!(out.contains("<g"));
/// ```
fn render_group_with_geometry(
    out: &mut svg::Renderer<'_>,
    attrs: &HashMap<String, String>,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    emit: impl FnOnce(&mut RendererBackend<'_, '_>) -> fmt::Result,
) -> fmt::Result {
    let mut group = out.start_element("g")?;
    group.attr_hashmap(attrs)?;
    add_debug_attrs(&mut group, name, x, y, geo)?;
    group.finish()?;

    let mut backend = RendererBackend { out };
    emit(&mut backend)?;
    write_debug_overlay(backend.out, x, y, geo)?;
    backend.out.end_element("g")
}

/// Build a debug-aware `<g class="...">` wrapper from a shared emit closure.
///
/// ```ignore
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let group = draw_class_group_with_geometry(
///     "demo",
///     "Demo",
///     0,
///     0,
///     &NodeGeometry { entry_height: 0, height: 0, width: 10, children: vec![] },
///     |backend| backend.push_path(svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(10)),
/// );
/// assert!(group.to_string().contains("class=\"demo\""));
/// ```
fn draw_class_group_with_geometry(
    class: &str,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    emit: impl FnOnce(&mut ElementBackend) -> fmt::Result,
) -> svg::Element {
    let mut backend = ElementBackend::default();
    emit(&mut backend).expect("element backend is infallible");

    let mut group = svg::Element::new("g").set("class", &class);
    for child in backend.children {
        group.push(child);
    }
    group.debug_with_geometry(name, x, y, geo)
}

/// Stream a debug-aware `<g class="...">` wrapper from a shared emit closure.
///
/// ```ignore
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let mut out = String::new();
/// let mut renderer = svg::Renderer::new(&mut out);
/// render_class_group_with_geometry(
///     &mut renderer,
///     "demo",
///     "Demo",
///     0,
///     0,
///     &NodeGeometry { entry_height: 0, height: 0, width: 10, children: vec![] },
///     |backend| backend.push_path(svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(10)),
/// ).unwrap();
/// assert!(out.contains("class=\"demo\""));
/// ```
fn render_class_group_with_geometry(
    out: &mut svg::Renderer<'_>,
    class: &str,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    emit: impl FnOnce(&mut RendererBackend<'_, '_>) -> fmt::Result,
) -> fmt::Result {
    let mut group = out.start_element("g")?;
    group.attr("class", class)?;
    add_debug_attrs(&mut group, name, x, y, geo)?;
    group.finish()?;

    let mut backend = RendererBackend { out };
    emit(&mut backend)?;
    write_debug_overlay(backend.out, x, y, geo)?;
    backend.out.end_element("g")
}

/// Attach cached-geometry debug metadata to a leaf path in the element backend.
///
/// ```ignore
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let geo = NodeGeometry { entry_height: 10, height: 20, width: 20, children: vec![] };
/// let path = draw_debug_path(
///     "Start",
///     0,
///     0,
///     &geo,
///     svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(20),
/// );
/// assert!(path.to_string().contains("railroad:type=\"Start\""));
/// ```
fn draw_debug_path(
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    path: svg::PathData,
) -> svg::Element {
    path.into_path().debug_with_geometry(name, x, y, geo)
}

/// Attach cached-geometry debug metadata to a leaf path in the streaming backend.
///
/// ```ignore
/// # use railroad::{NodeGeometry, notactuallysvg as svg, HDir};
/// let geo = NodeGeometry { entry_height: 10, height: 20, width: 20, children: vec![] };
/// let mut out = String::new();
/// let mut renderer = svg::Renderer::new(&mut out);
/// render_debug_path(
///     &mut renderer,
///     "Start",
///     0,
///     0,
///     &geo,
///     svg::PathData::new(HDir::LTR).move_to(0, 0).horizontal(20),
/// ).unwrap();
/// assert!(out.contains("railroad:type=\"Start\""));
/// ```
fn render_debug_path(
    out: &mut svg::Renderer<'_>,
    name: &str,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    path: svg::PathData,
) -> fmt::Result {
    let mut tag = out.start_element("path")?;
    tag.attr("d", path)?;
    add_debug_attrs(&mut tag, name, x, y, geo)?;
    tag.finish_empty()?;
    write_debug_overlay(out, x, y, geo)
}

/// Emit a boxed text node shared by `Terminal` and `NonTerminal`.
///
/// `rounded` selects between the rounded terminal box and the square
/// non-terminal box.
///
/// ```ignore
/// # use railroad::{NodeGeometry, notactuallysvg as svg};
/// let mut backend = ElementBackend::default();
/// emit_text_box(
///     &mut backend,
///     0,
///     0,
///     &NodeGeometry { entry_height: 11, height: 22, width: 60, children: vec![] },
///     "item",
///     true,
/// ).unwrap();
/// assert_eq!(backend.children.len(), 2);
/// ```
fn emit_text_box<B: RenderBackend>(
    backend: &mut B,
    x: i64,
    y: i64,
    geo: &NodeGeometry,
    label: &str,
    rounded: bool,
) -> fmt::Result {
    if rounded {
        backend.push_rounded_rect(x, y, geo.width, geo.height, 10)?;
    } else {
        backend.push_rect(x, y, geo.width, geo.height)?;
    }
    backend.push_text(x + geo.width / 2, y + geo.entry_height + 5, label)
}

/// Convenience aggregation helpers for iterators and collections of [`Node`]s.
///
/// `NodeCollection` is implemented for any `IntoIterator<Item = N>` where `N`
/// implements [`Node`]. It is mainly a small ergonomic helper for container
/// nodes that need to aggregate child geometry without spelling out the same
/// iterator expressions repeatedly.
///
/// The methods consume `self`, so they work naturally on iterators as well as on
/// owned collections. When called on borrowed collections such as `slice.iter()`
/// or `vec.iter()`, the iterator items are references, and the blanket `Node`
/// implementations for references keep the methods usable.
///
/// # Example
/// ```rust
/// use railroad::{End, NodeCollection, SimpleStart, Start};
///
/// let nodes = [Start, Start];
/// assert!(nodes.iter().max_entry_height() > 0);
///
/// let widths = vec![SimpleStart, SimpleStart];
/// assert!(widths.iter().total_width() > 0);
///
/// let exits: Vec<Box<dyn railroad::Node>> = vec![Box::new(End), Box::new(SimpleStart)];
/// assert!(exits.iter().max_height_below_entry() > 0);
/// ```
pub trait NodeCollection {
    /// Return the maximum [`Node::entry_height`] in the collection.
    ///
    /// This is commonly used by horizontal container nodes that align several
    /// children to the same connecting path.
    ///
    /// # Example
    /// ```rust
    /// use railroad::{Comment, NodeCollection, Start};
    ///
    /// let nodes: Vec<Box<dyn railroad::Node>> =
    ///     vec![Box::new(Comment::new("note".to_owned())), Box::new(Start)];
    /// assert!(nodes.iter().max_entry_height() > 0);
    /// ```
    fn max_entry_height(self) -> i64;

    /// Return the maximum [`Node::height`] in the collection.
    fn max_height(self) -> i64;

    /// Return the maximum [`Node::height_below_entry`] in the collection.
    ///
    /// This is useful when children are aligned by their connecting path and the
    /// parent needs enough space below that path for the deepest child.
    ///
    /// # Example
    /// ```rust
    /// use railroad::{Comment, NodeCollection, Terminal};
    ///
    /// let nodes: Vec<Box<dyn railroad::Node>> = vec![
    ///     Box::new(Comment::new("note".to_owned())),
    ///     Box::new(Terminal::new("token".to_owned())),
    /// ];
    /// assert!(nodes.iter().max_height_below_entry() > 0);
    /// ```
    fn max_height_below_entry(self) -> i64;

    /// Return the maximum [`Node::width`] in the collection.
    fn max_width(self) -> i64;

    /// Return the sum of all [`Node::width`] values in the collection.
    ///
    /// This is typically used by horizontal container nodes before adding their
    /// own inter-child spacing.
    ///
    /// # Example
    /// ```rust
    /// use railroad::{NodeCollection, SimpleStart, Start};
    ///
    /// let nodes = [Start, Start];
    /// assert!(nodes.iter().total_width() > 0);
    ///
    /// let mixed = [SimpleStart, SimpleStart];
    /// assert!(mixed.iter().total_width() > 0);
    /// ```
    fn total_width(self) -> i64;

    /// Return the sum of all [`Node::height`] values in the collection.
    fn total_height(self) -> i64;
}

impl<I, N> NodeCollection for I
where
    I: IntoIterator<Item = N>,
    N: Node,
{
    fn max_height_below_entry(self) -> i64 {
        self.into_iter()
            .map(|n| n.height_below_entry())
            .max()
            .unwrap_or_default()
    }

    fn max_entry_height(self) -> i64 {
        self.into_iter()
            .map(|n| n.entry_height())
            .max()
            .unwrap_or_default()
    }

    fn max_height(self) -> i64 {
        self.into_iter()
            .map(|n| n.height())
            .max()
            .unwrap_or_default()
    }

    fn max_width(self) -> i64 {
        self.into_iter()
            .map(|n| n.width())
            .max()
            .unwrap_or_default()
    }

    fn total_width(self) -> i64 {
        self.into_iter().map(|n| n.width()).sum()
    }

    fn total_height(self) -> i64 {
        self.into_iter().map(|n| n.height()).sum()
    }
}

/// A symbol indicating the logical end of a syntax-diagram via two vertical bars.
#[derive(Debug, Clone, Default)]
pub struct End;

impl Node for End {
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
        draw_debug_path(
            "End",
            x,
            y,
            &self.compute_geometry(),
            svg::PathData::new(h_dir)
                .move_to(x, y + 10)
                .horizontal(20)
                .move_rel(-10, -10)
                .vertical(20)
                .move_rel(10, -20)
                .vertical(20),
        )
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_debug_path(
            out,
            "End",
            x,
            y,
            geo,
            svg::PathData::new(h_dir)
                .move_to(x, y + 10)
                .horizontal(20)
                .move_rel(-10, -10)
                .vertical(20)
                .move_rel(10, -20)
                .vertical(20),
        )
    }
}

/// A symbol indicating the logical start of a syntax-diagram via a circle
#[derive(Debug, Clone, Default)]
pub struct SimpleStart;

impl Node for SimpleStart {
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
        draw_debug_path(
            "SimpleStart",
            x,
            y,
            &self.compute_geometry(),
            svg::PathData::new(h_dir)
                .move_to(x, y + 5)
                .arc(5, svg::Arc::SouthToEast)
                .arc(5, svg::Arc::WestToSouth)
                .arc(5, svg::Arc::NorthToWest)
                .arc(5, svg::Arc::EastToNorth)
                .move_rel(10, 0)
                .horizontal(5),
        )
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_debug_path(
            out,
            "SimpleStart",
            x,
            y,
            geo,
            svg::PathData::new(h_dir)
                .move_to(x, y + 5)
                .arc(5, svg::Arc::SouthToEast)
                .arc(5, svg::Arc::WestToSouth)
                .arc(5, svg::Arc::NorthToWest)
                .arc(5, svg::Arc::EastToNorth)
                .move_rel(10, 0)
                .horizontal(5),
        )
    }
}

/// A symbol indicating the logical end of a syntax-diagram via a circle
#[derive(Debug, Clone, Default)]
pub struct SimpleEnd;

impl Node for SimpleEnd {
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
        draw_debug_path(
            "SimpleEnd",
            x,
            y,
            &self.compute_geometry(),
            svg::PathData::new(h_dir)
                .move_to(x, y + 5)
                .horizontal(5)
                .arc(5, svg::Arc::SouthToEast)
                .arc(5, svg::Arc::WestToSouth)
                .arc(5, svg::Arc::NorthToWest)
                .arc(5, svg::Arc::EastToNorth),
        )
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_debug_path(
            out,
            "SimpleEnd",
            x,
            y,
            geo,
            svg::PathData::new(h_dir)
                .move_to(x, y + 5)
                .horizontal(5)
                .arc(5, svg::Arc::SouthToEast)
                .arc(5, svg::Arc::WestToSouth)
                .arc(5, svg::Arc::NorthToWest)
                .arc(5, svg::Arc::EastToNorth),
        )
    }
}

/// A symbol indicating the logical start of a syntax-diagram via two vertical bars.
#[derive(Debug, Clone, Default)]
pub struct Start;

impl Node for Start {
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
        draw_debug_path(
            "Start",
            x,
            y,
            &self.compute_geometry(),
            svg::PathData::new(h_dir)
                .move_to(x, y)
                .vertical(20)
                .move_rel(10, -20)
                .vertical(20)
                .move_rel(-10, -10)
                .horizontal(20),
        )
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_debug_path(
            out,
            "Start",
            x,
            y,
            geo,
            svg::PathData::new(h_dir)
                .move_to(x, y)
                .vertical(20)
                .move_rel(10, -20)
                .vertical(20)
                .move_rel(-10, -10)
                .horizontal(20),
        )
    }
}

/// A rectangle drawn with the given dimensions, used for visual debugging
#[derive(Debug)]
#[doc(hidden)]
pub struct Debug {
    entry_height: i64,
    height: i64,
    width: i64,
    attributes: HashMap<String, String>,
}

impl Debug {
    #[must_use]
    /// # Panics
    /// If `entry_height` is not smaller than `height`
    pub fn new(entry_height: i64, height: i64, width: i64) -> Self {
        assert!(entry_height < height);
        let mut d = Self {
            entry_height,
            height,
            width,
            attributes: HashMap::default(),
        };

        d.attributes.insert("class".to_owned(), "debug".to_owned());
        d.attributes.insert(
            "style".to_owned(),
            "fill: hsla(0, 100%, 90%, 0.9); stroke-width: 2; stroke: red".to_owned(),
        );
        d
    }
}

impl Node for Debug {
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
            .set("x", &x)
            .set("y", &y)
            .set("height", &self.height())
            .set("width", &self.width())
            .set_all(self.attributes.iter())
            .debug("Debug", x, y, self)
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        _h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let mut rect = out.start_element("rect")?;
        rect.attr("x", x)?;
        rect.attr("y", y)?;
        rect.attr("height", geo.height)?;
        rect.attr("width", geo.width)?;
        rect.attr_hashmap(&self.attributes)?;
        add_debug_attrs(&mut rect, "Debug", x, y, geo)?;
        rect.finish_empty()?;
        write_debug_overlay(out, x, y, geo)
    }
}

/// A dummy-element which has no size and draws nothing.
///
/// This can be used in conjunction with `Choice` (to indicate that one of the options
/// is blank, a shorthand for an `Optional(Choice)`), `Repeat` (if there are
/// zero-or-more repetitions or if there is no joining element), or `LabeledBox`
/// (if the label should be empty).
#[derive(Debug, Clone, Default)]
pub struct Empty;

impl Node for Empty {
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

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        _h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let mut g = out.start_element("g")?;
        add_debug_attrs(&mut g, "Empty", x, y, geo)?;
        g.finish()?;
        write_debug_overlay(out, x, y, geo)?;
        out.end_element("g")
    }
}

/// The top-level container that renders a node tree as a complete SVG document.
///
/// `Diagram` wraps a root [`Node`], computes its geometry, and emits a
/// self-contained `<svg>` element. CSS stylesheets and arbitrary extra SVG
/// elements can be injected before drawing.
///
/// The `fmt::Display` implementation (and [`Diagram::write`]) both use the
/// two-phase geometry caching pipeline internally, so rendering is O(n) in
/// the number of nodes.
///
/// # Example
/// ```rust
/// use railroad::*;
///
/// let dia = Diagram::new_with_stylesheet(
///     Sequence::new(vec![Box::new(Start) as Box<dyn Node>, Box::new(End)]),
///     &Stylesheet::Light,
/// );
/// let svg = dia.to_string();
/// assert!(svg.starts_with("<svg"));
/// ```
#[derive(Debug, Clone)]
pub struct Diagram<N> {
    root: N,
    extra_attributes: HashMap<String, String>,
    extra_elements: Vec<svg::Element>,
    left_padding: i64,
    right_padding: i64,
    top_padding: i64,
    bottom_padding: i64,
}

impl<N: Node> Diagram<N> {
    /// Create a diagram using the given root-element.
    ///
    /// ```
    /// use railroad::*;
    ///
    /// let mut seq: Sequence::<Box<dyn Node>> = Sequence::default();
    /// seq.push(Box::new(Start))
    ///    .push(Box::new(Terminal::new("Foobar".to_owned())))
    ///    .push(Box::new(End));
    ///
    /// let mut dia = Diagram::new(seq);
    /// println!("{}", dia);
    /// ```
    pub fn new(root: N) -> Self {
        Self {
            root,
            extra_attributes: HashMap::default(),
            extra_elements: Vec::default(),
            left_padding: 10,
            right_padding: 10,
            top_padding: 10,
            bottom_padding: 10,
        }
    }

    /// Create a diagram using the given root-element, adding the given stylesheet.
    ///
    /// ```rust
    /// use railroad::*;
    ///
    /// let mut seq: Sequence::<Box<dyn Node>> = Sequence::default();
    /// seq.push(Box::new(Start))
    ///    .push(Box::new(Terminal::new("Foobar".to_owned())))
    ///    .push(Box::new(End));
    ///
    /// let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::Light);
    /// println!("{}", dia);
    /// ```
    pub fn new_with_stylesheet(root: N, style: &Stylesheet) -> Self {
        let mut dia = Self::new(root);
        dia.add_stylesheet(style);
        dia
    }

    /// Create a diagram which has this library's default CSS style included.
    pub fn with_default_css(root: N) -> Self {
        let mut dia = Self::new(root);
        dia.add_default_css();
        dia
    }

    /// Add the CSS for `style` as an additional `<style>` element.
    pub fn add_stylesheet(&mut self, style: &Stylesheet) {
        self.add_css(style.stylesheet());
    }

    /// Add the default CSS as an additional `<style>` element.
    pub fn add_default_css(&mut self) {
        self.add_css(DEFAULT_CSS);
    }

    /// Add the given CSS as an additional `<style>` element.
    pub fn add_css(&mut self, css: &str) {
        self.add_element(
            svg::Element::new("style")
                .set("type", "text/css")
                .raw_text(css),
        );
    }

    /// Set an attribute on the `<svg>`-tag.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.extra_attributes.entry(key)
    }

    /// Add an additional `svg::Element` which is written before the root-element
    pub fn add_element(&mut self, e: svg::Element) -> &mut Self {
        self.extra_elements.push(e);
        self
    }

    /// Write this diagram's SVG-code to the given writer.
    ///
    /// # Errors
    /// Returns errors in the underlying writer.
    pub fn write(&self, writer: &mut impl io::Write) -> io::Result<()> {
        writer.write_all(self.to_string().as_bytes())
    }

    /// Unwrap this diagram, returning the root node.
    pub fn into_inner(self) -> N {
        self.root
    }
}

impl<N> Default for Diagram<N>
where
    N: Default,
{
    fn default() -> Self {
        Self {
            root: Default::default(),
            extra_attributes: HashMap::default(),
            extra_elements: Vec::default(),
            left_padding: 10,
            right_padding: 10,
            top_padding: 10,
            bottom_padding: 10,
        }
    }
}

impl<N> Node for Diagram<N>
where
    N: Node,
{
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
        let geo = self.compute_geometry();
        self.draw_with_geometry(x, y, h_dir, &geo)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let root_geo = self.root.compute_geometry();
        let height = self.top_padding + root_geo.height + self.bottom_padding;
        let width = self.left_padding + root_geo.width + self.right_padding;
        NodeGeometry {
            entry_height: 0,
            height,
            width,
            children: vec![root_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        let mut e = svg::Element::new("svg")
            .set("xmlns", "http://www.w3.org/2000/svg")
            .set("xmlns:xlink", "http://www.w3.org/1999/xlink")
            .set("class", "railroad")
            .set("viewBox", &format!("0 0 {} {}", geo.width, geo.height));

        #[cfg(feature = "visual-debug")]
        {
            e = e.set("xmlns:railroad", "http://www.github.com/lukaslueg/railroad");
        }
        for (k, v) in &self.extra_attributes {
            e = e.set(&k, &v);
        }
        for extra_ele in self.extra_elements.iter().cloned() {
            e = e.add(extra_ele);
        }
        e.add(
            svg::Element::new("rect")
                .set("width", "100%")
                .set("height", "100%")
                .set("class", "railroad_canvas"),
        )
        .add(self.root.draw_with_geometry(
            x + self.left_padding,
            y + self.top_padding,
            h_dir,
            &geo.children[0],
        ))
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let mut svg_tag = out.start_element("svg")?;
        svg_tag.attr("xmlns", "http://www.w3.org/2000/svg")?;
        svg_tag.attr("xmlns:xlink", "http://www.w3.org/1999/xlink")?;
        svg_tag.attr("class", "railroad")?;
        svg_tag.attr("viewBox", format_args!("0 0 {} {}", geo.width, geo.height))?;
        #[cfg(feature = "visual-debug")]
        svg_tag.attr("xmlns:railroad", "http://www.github.com/lukaslueg/railroad")?;
        svg_tag.attr_hashmap(&self.extra_attributes)?;
        svg_tag.finish()?;

        for extra in &self.extra_elements {
            out.write_display(extra)?;
        }

        let mut rect = out.start_element("rect")?;
        rect.attr("width", "100%")?;
        rect.attr("height", "100%")?;
        rect.attr("class", "railroad_canvas")?;
        rect.finish_empty()?;

        self.root.render_with_geometry(
            out,
            x + self.left_padding,
            y + self.top_padding,
            h_dir,
            &geo.children[0],
        )?;
        out.end_element("svg")
    }
}

impl<N> fmt::Display for Diagram<N>
where
    N: Node,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let geo = self.compute_geometry();
        let mut renderer = svg::Renderer::new(f);
        self.render_with_geometry(&mut renderer, 0, 0, HDir::LTR, &geo)
    }
}

#[cfg(test)]
#[cfg(not(feature = "visual-debug"))]
mod tests_without_visual_debug {
    use super::*;
    use std::cell::Cell;

    /// A counting wrapper that increments a counter on every geometry call.
    struct CountingNode<'a> {
        inner: Box<dyn Node>,
        calls: &'a Cell<usize>,
    }

    impl Node for CountingNode<'_> {
        fn entry_height(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.entry_height()
        }
        fn height(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.height()
        }
        fn width(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.width()
        }
        fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
            self.inner.draw(x, y, h_dir)
        }
        // compute_geometry / draw_with_geometry use the default impls, which
        // call entry_height/height/width exactly once each → O(n) total.
    }

    /// Verify that drawing via compute_geometry + draw_with_geometry calls each
    /// leaf node's geometry methods exactly 3 times (once per entry_height /
    /// height / width) regardless of tree depth.
    #[test]
    fn geometry_cache_linear_calls() {
        let calls = Cell::new(0usize);
        let leaf = CountingNode {
            inner: Box::new(Terminal::new("leaf".to_owned())),
            calls: &calls,
        };

        // Wrap in two levels of Sequence: [[leaf]]
        let inner_seq: Sequence<Box<dyn Node>> =
            Sequence::new(vec![Box::new(leaf) as Box<dyn Node>]);
        let outer_seq: Sequence<Box<dyn Node>> =
            Sequence::new(vec![Box::new(inner_seq) as Box<dyn Node>]);

        let geo = outer_seq.compute_geometry();
        let _ = outer_seq.draw_with_geometry(0, 0, HDir::LTR, &geo);

        // entry_height + height + width = 3 calls, regardless of nesting depth
        assert_eq!(
            calls.get(),
            3,
            "each leaf geometry method must be called exactly once"
        );
    }
}

#[cfg(test)]
#[cfg(feature = "visual-debug")]
mod tests_with_visual_debug {
    use super::*;
    use std::cell::Cell;

    /// A counting wrapper that increments a counter on every geometry call.
    struct CountingNode<'a> {
        inner: Box<dyn Node>,
        calls: &'a Cell<usize>,
    }

    impl Node for CountingNode<'_> {
        fn entry_height(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.entry_height()
        }
        fn height(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.height()
        }
        fn width(&self) -> i64 {
            self.calls.set(self.calls.get() + 1);
            self.inner.width()
        }
        fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
            self.inner.draw(x, y, h_dir)
        }
        // compute_geometry / draw_with_geometry use the default impls, which
        // call entry_height/height/width exactly once each when cached geometry
        // is threaded through the visual-debug path correctly.
    }

    #[test]
    fn visual_debug_geometry_cache_linear_calls() {
        let calls = Cell::new(0usize);
        let leaf = CountingNode {
            inner: Box::new(Terminal::new("leaf".to_owned())),
            calls: &calls,
        };

        let mut nested: Box<dyn Node> = Box::new(leaf);
        for _ in 0..8 {
            nested = Box::new(Sequence::new(vec![nested]));
        }

        let geo = nested.compute_geometry();
        let _ = nested.draw_with_geometry(0, 0, HDir::LTR, &geo);

        assert_eq!(
            calls.get(),
            3,
            "visual-debug must not trigger extra geometry calls during draw_with_geometry"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_impl() {
        let s = Sequence::new(vec![
            Box::new(SimpleStart) as Box<dyn Node>,
            Box::new(SimpleEnd),
        ]);
        assert_eq!(
            "Sequence { children: [Node { entry_height: 5, height: 10, width: 15 }, Node { entry_height: 5, height: 10, width: 15 }], spacing: 10 }",
            format!("{:?}", &s)
        );
        assert_eq!(
            "Node { entry_height: 5, height: 10, width: 40 }",
            format!("{:?}", &s as &dyn Node)
        );
    }

    /// Helper: build a nested Sequence tree of the given depth and width.
    fn make_deep_seq(depth: usize, width: usize) -> Box<dyn Node> {
        if depth == 0 {
            Box::new(Terminal::new("x".to_owned()))
        } else {
            let children: Vec<Box<dyn Node>> = (0..width)
                .map(|_| make_deep_seq(depth - 1, width))
                .collect();
            Box::new(Sequence::new(children))
        }
    }

    /// draw_with_geometry produces the same SVG as the original draw path.
    #[test]
    fn geometry_cache_regression() {
        let root = make_deep_seq(3, 3);
        let dia_a = Diagram::new(make_deep_seq(3, 3));
        let dia_b = Diagram::new(make_deep_seq(3, 3));

        // draw() uses the two-phase path internally; draw_with_geometry is its
        // implementation.  We verify that `format!` output (which calls draw)
        // is stable across two calls (no hidden mutable state).
        let svg_a = format!("{}", dia_a);
        let svg_b = format!("{}", dia_b);
        assert_eq!(svg_a, svg_b, "SVG output must be deterministic");

        // Also check that width/height/entry_height are consistent with the
        // geometry cache.
        let geo = root.compute_geometry();
        assert_eq!(geo.entry_height, root.entry_height());
        assert_eq!(geo.height, root.height());
        assert_eq!(geo.width, root.width());
    }

    #[test]
    fn diagram_write_matches_display() {
        let diagram = Diagram::new(make_deep_seq(2, 3));
        let displayed = format!("{}", diagram);

        let mut written = Vec::new();
        diagram.write(&mut written).unwrap();

        assert_eq!(String::from_utf8(written).unwrap(), displayed);
    }

    const PAYLOADS: &[&str] = &[
        r#""><script>alert(1)</script>"#,
        r#"' onload='alert(1)"#,
        r#"foo & bar"#,
        r#"</style><script>bad</script>"#,
        r#"foo"bar"#,
    ];

    fn assert_no_payload(svg: &str, payload: &str) {
        assert!(
            !svg.contains(payload),
            "raw payload {payload:?} found in SVG output"
        );
    }

    /// Terminal and NonTerminal labels are rendered as SVG text content.
    #[test]
    fn terminal_label_no_injection() {
        for payload in PAYLOADS {
            let svg = format!("{}", Diagram::new(Terminal::new(payload.to_string())));
            assert_no_payload(&svg, payload);
            let svg = format!("{}", Diagram::new(NonTerminal::new(payload.to_string())));
            assert_no_payload(&svg, payload);
        }
    }

    /// Comment text is rendered as SVG text content.
    #[test]
    fn comment_text_no_injection() {
        for payload in PAYLOADS {
            let svg = format!("{}", Diagram::new(Comment::new(payload.to_string())));
            assert_no_payload(&svg, payload);
        }
    }

    /// Link URIs end up in an xlink:href attribute.
    #[test]
    fn link_uri_no_injection() {
        for payload in PAYLOADS {
            let node = Link::new(Empty, payload.to_string());
            let svg = format!("{}", Diagram::new(node));
            assert_no_payload(&svg, payload);
        }
    }

    /// User-supplied attribute keys and values (via `.attr()`) must be escaped.
    #[test]
    fn node_attr_no_injection() {
        for payload in PAYLOADS {
            // Dangerous value
            let mut t = Terminal::new("x".to_owned());
            t.attr("data-x".to_owned()).or_insert(payload.to_string());
            let svg = format!("{}", Diagram::new(t));
            assert_no_payload(&svg, payload);

            // Dangerous key
            let mut t = Terminal::new("x".to_owned());
            t.attr(payload.to_string()).or_insert("value".to_owned());
            let svg = format!("{}", Diagram::new(t));
            assert_no_payload(&svg, payload);
        }
    }
}
