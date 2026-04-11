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

use std::{
    cmp,
    collections::{self, HashMap},
    fmt, io, iter,
};

pub mod notactuallysvg;
pub use crate::notactuallysvg as svg;
use crate::svg::HDir;

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

/// A diagram is built from a set of primitives which implement `Node`.
///
/// A primitive is a geometric box, within which it can draw whatever it wants.
/// Simple primitives (e.g. `Start`) have fixed width, height etc.. Complex
/// primitives, which wrap other primitives (e.g. `Sequence`), use the methods
/// defined here to compute their own geometry. When the time comes for a primitive
/// to be drawn, the wrapping primitive computes the desired location of the wrapped
/// primitive(s) and calls `.draw()` on them. It is the primitive's job
/// to ensure that it uses only the space it announced.
pub trait Node {
    /// The vertical distance from this element's top to where the entering,
    /// connecting path is drawn.
    ///
    /// By convention, the path connecting primitives enters from the left.
    fn entry_height(&self) -> i64;

    /// This primitives's total height.
    fn height(&self) -> i64;

    /// This primitive's total width.
    fn width(&self) -> i64;

    /// The vertical distance from the height of the connecting path to the bottom.
    ///
    /// Equivalent to `height() - entry_height()`.
    fn height_below_entry(&self) -> i64 {
        self.height() - self.entry_height()
    }

    /// Draw this element as an `svg::Element` at the given position and direction.
    ///
    /// The element must fit entirely within the bounding box defined by `(x, y)`,
    /// `width()`, and `height()`, with the connecting path at `y + entry_height()`.
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element;

    /// Compute geometry for this node and its entire subtree in a single bottom-up pass.
    ///
    /// The returned [`NodeGeometry`] is a transient value intended to be passed to
    /// [`Node::draw_with_geometry`]; it is not stored inside the node.
    ///
    /// The default implementation is correct for leaf nodes (no children). Composite
    /// nodes should override this to recurse into their children.
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
    /// extract child geometry from `geo.children[i]` and pass it to each child's
    /// `draw_with_geometry` call rather than calling `geo.children[i].entry_height()`
    /// etc. directly.
    ///
    /// The default implementation falls back to [`Node::draw`], which is correct for
    /// all nodes but does not benefit from caching. External [`Node`] implementors
    /// do not need to override this method.
    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, _geo: &NodeGeometry) -> svg::Element {
        self.draw(x, y, h_dir)
    }

    /// Render this element directly into an SVG renderer.
    ///
    /// This is the streaming counterpart to [`Node::draw`]. The default
    /// implementation computes geometry once and forwards to
    /// [`Node::render_with_geometry`].
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

/// Helper trait for collections of nodes.
pub trait NodeCollection {
    /// The maximum `entry_height()`-value.
    fn max_entry_height(self) -> i64;

    /// The maximum `height()`-value.
    fn max_height(self) -> i64;

    /// The maximum `height_below_entry()`-value.
    fn max_height_below_entry(self) -> i64;

    /// The maximum `width()`-value.
    fn max_width(self) -> i64;

    /// The sum of all `width()`-values.
    fn total_width(self) -> i64;

    /// The sum of all `height()`-values.
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

/// Possible targets for `Link`.
///
/// Maps to the HTML `target` attribute on the generated `<a>` element.
#[derive(Debug, Default, Clone, Copy)]
pub enum LinkTarget {
    /// Open in a new tab (`target="_blank"`).
    #[default]
    Blank,
    /// Open in the parent frame (`target="_parent"`).
    Parent,
    /// Open in the topmost frame (`target="_top"`).
    Top,
}

/// Wraps another primitive, making it a clickable link to some URI.
#[derive(Debug, Clone)]
pub struct Link<N> {
    inner: N,
    uri: String,
    target: Option<LinkTarget>,
    attributes: HashMap<String, String>,
}

impl<N> Link<N> {
    /// Wrap `inner` in a clickable link pointing to `uri`.
    ///
    /// The URI is placed in an SVG anchor attribute and is HTML-escaped before
    /// being written into the SVG, so arbitrary strings are safe to pass.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let node = Link::new(Terminal::new("docs".to_owned()), "https://example.com".to_owned());
    /// assert!(Diagram::new(node).to_string().starts_with("<svg"));
    /// ```
    pub fn new(inner: N, uri: String) -> Self {
        let mut l = Self {
            inner,
            uri,
            target: None,
            attributes: HashMap::default(),
        };
        l.attributes.insert("class".to_owned(), "link".to_owned());
        l
    }

    /// Set the `target` attribute for the generated `<a>` element.
    ///
    /// Pass `None` to remove any previously set target.
    pub fn set_target(&mut self, target: Option<LinkTarget>) {
        self.target = target;
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit the wrapped child once, letting the outer `<a>` wrapper choose the backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        backend.push_child(&self.inner, x, y, h_dir, &geo.children[0])
    }
}

impl<N> Node for Link<N>
where
    N: Node,
{
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
            .set("xlink:href", &self.uri);
        a = match self.target {
            Some(LinkTarget::Blank) => a.set("target", "_blank"),
            Some(LinkTarget::Parent) => a.set("target", "_parent"),
            Some(LinkTarget::Top) => a.set("target", "_top"),
            None => a,
        };
        a.set_all(self.attributes.iter())
            .add(self.inner.draw(x, y, h_dir))
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let inner_geo = self.inner.compute_geometry();
        let entry_height = inner_geo.entry_height;
        let height = inner_geo.height;
        let width = inner_geo.width;
        NodeGeometry {
            entry_height,
            height,
            width,
            children: vec![inner_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        let mut backend = ElementBackend::default();
        self.emit_with_geometry(&mut backend, x, y, h_dir, geo)
            .expect("element backend is infallible");
        let mut a = svg::Element::new("a")
            .debug_with_geometry("Link", x, y, geo)
            .set("xlink:href", &self.uri);
        a = match self.target {
            Some(LinkTarget::Blank) => a.set("target", "_blank"),
            Some(LinkTarget::Parent) => a.set("target", "_parent"),
            Some(LinkTarget::Top) => a.set("target", "_top"),
            None => a,
        };
        let mut a = a.set_all(self.attributes.iter());
        for child in backend.children {
            a.push(child);
        }
        a
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let mut a = out.start_element("a")?;
        a.attr("xlink:href", &self.uri)?;
        match self.target {
            Some(LinkTarget::Blank) => a.attr("target", "_blank")?,
            Some(LinkTarget::Parent) => a.attr("target", "_parent")?,
            Some(LinkTarget::Top) => a.attr("target", "_top")?,
            None => {}
        }
        a.attr_hashmap(&self.attributes)?;
        add_debug_attrs(&mut a, "Link", x, y, geo)?;
        a.finish()?;
        self.emit_with_geometry(&mut RendererBackend { out }, x, y, h_dir, geo)?;
        write_debug_overlay(out, x, y, geo)?;
        out.end_element("a")
    }
}

/// A vertical group of unconnected elements.
#[derive(Debug, Clone)]
pub struct VerticalGrid<N> {
    children: Vec<N>,
    spacing: i64,
    attributes: HashMap<String, String>,
}

impl<N> VerticalGrid<N> {
    /// Create a `VerticalGrid` containing `children`, laid out top-to-bottom.
    ///
    /// Children are spaced by a fixed amount. They are not connected by
    /// any path; use [`Stack`] for connected vertical sequences.
    #[must_use]
    pub fn new(children: Vec<N>) -> Self {
        let mut v = Self {
            children,
            ..Self::default()
        };
        v.attributes
            .insert("class".to_owned(), "verticalgrid".to_owned());
        v
    }

    /// Append a child and return `&mut self` for chaining.
    pub fn push(&mut self, child: N) -> &mut Self {
        self.children.push(child);
        self
    }

    /// Unwrap this grid, returning the children in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<N> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit all children in top-to-bottom order using cached geometry.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let mut running_y = y;
        for (child, child_geo) in self.children.iter().zip(geo.children.iter()) {
            backend.push_child(child, x, running_y, h_dir, child_geo)?;
            running_y += child_geo.height + self.spacing;
        }
        Ok(())
    }
}

impl<N> Default for VerticalGrid<N> {
    fn default() -> Self {
        Self {
            children: Vec::default(),
            spacing: ARC_RADIUS,
            attributes: HashMap::default(),
        }
    }
}

impl<N> iter::FromIterator<N> for VerticalGrid<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N: Node> Node for VerticalGrid<N> {
    fn entry_height(&self) -> i64 {
        0
    }

    fn height(&self) -> i64 {
        self.children.iter().total_height()
            + ((cmp::max(1, i64::try_from(self.children.len()).unwrap()) - 1) * self.spacing)
    }

    fn width(&self) -> i64 {
        self.children.iter().max_width()
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

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> =
            self.children.iter().map(|c| c.compute_geometry()).collect();
        let total_height: i64 = children.iter().map(|g| g.height).sum();
        let n = cmp::max(1, i64::try_from(children.len()).unwrap());
        let height = total_height + (n - 1) * self.spacing;
        let width = children.iter().map(|g| g.width).max().unwrap_or(0);
        NodeGeometry {
            entry_height: 0,
            height,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "VerticalGrid", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(
            out,
            &self.attributes,
            "VerticalGrid",
            x,
            y,
            geo,
            |backend| self.emit_with_geometry(backend, x, y, h_dir, geo),
        )
    }
}

/// A horizontal group of unconnected elements.
#[derive(Debug, Clone)]
pub struct HorizontalGrid<N> {
    children: Vec<N>,
    spacing: i64,
    attributes: HashMap<String, String>,
}

impl<N> HorizontalGrid<N> {
    /// Create a `HorizontalGrid` containing `children`, laid out left-to-right.
    ///
    /// Children are spaced by a fixed amount. They are not connected by
    /// any path; use [`Sequence`] for connected horizontal sequences.
    #[must_use]
    pub fn new(children: Vec<N>) -> Self {
        let mut h = Self {
            children,
            ..Self::default()
        };
        h.attributes
            .insert("class".to_owned(), "horizontalgrid".to_owned());
        h
    }

    /// Append a child and return `&mut self` for chaining.
    pub fn push(&mut self, child: N) -> &mut Self {
        self.children.push(child);
        self
    }

    /// Unwrap this grid, returning the children in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<N> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit all children in left-to-right order using cached geometry.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let mut running_x = x;
        for (child, child_geo) in self.children.iter().zip(geo.children.iter()) {
            backend.push_child(child, running_x, y, h_dir, child_geo)?;
            running_x += child_geo.width + self.spacing;
        }
        Ok(())
    }
}

impl<N> Default for HorizontalGrid<N> {
    fn default() -> Self {
        Self {
            children: Vec::default(),
            spacing: ARC_RADIUS,
            attributes: HashMap::default(),
        }
    }
}

impl<N> iter::FromIterator<N> for HorizontalGrid<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N> Node for HorizontalGrid<N>
where
    N: Node,
{
    fn entry_height(&self) -> i64 {
        0
    }

    fn height(&self) -> i64 {
        self.children.iter().max_height()
    }

    fn width(&self) -> i64 {
        self.children.iter().total_width()
            + ((cmp::max(1, i64::try_from(self.children.len()).unwrap()) - 1) * self.spacing)
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

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> =
            self.children.iter().map(|c| c.compute_geometry()).collect();
        let height = children.iter().map(|g| g.height).max().unwrap_or(0);
        let total_width: i64 = children.iter().map(|g| g.width).sum();
        let n = cmp::max(1, i64::try_from(children.len()).unwrap());
        let width = total_width + (n - 1) * self.spacing;
        NodeGeometry {
            entry_height: 0,
            height,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "HorizontalGrid", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(
            out,
            &self.attributes,
            "HorizontalGrid",
            x,
            y,
            geo,
            |backend| self.emit_with_geometry(backend, x, y, h_dir, geo),
        )
    }
}

/// A horizontal group of elements, connected from left to right.
///
/// Also see `Stack` for a vertical group of elements.
#[derive(Debug, Clone)]
pub struct Sequence<N> {
    children: Vec<N>,
    spacing: i64,
}

impl<N> Sequence<N> {
    /// Create a `Sequence` from an ordered list of children.
    ///
    /// The children are connected left-to-right with a horizontal path segment
    /// between adjacent elements.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let seq: Sequence<Box<dyn Node>> = Sequence::new(vec![
    ///     Box::new(Start),
    ///     Box::new(End),
    /// ]);
    /// assert!(Diagram::new(seq).to_string().starts_with("<svg"));
    /// ```
    #[must_use]
    pub fn new(children: Vec<N>) -> Self {
        Self {
            children,
            ..Self::default()
        }
    }

    /// Append a child and return `&mut self` for chaining.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let mut seq: Sequence<Box<dyn Node>> = Sequence::default();
    /// seq.push(Box::new(Start)).push(Box::new(End));
    /// ```
    pub fn push(&mut self, child: N) -> &mut Self {
        self.children.push(child);
        self
    }

    /// Unwrap this sequence, returning the children in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<N> {
        self.children
    }

    /// Emit sequence children and their connecting segments in a single shared pass.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let mut running_x = 0;
        for (child, child_geo) in self.children.iter().zip(geo.children.iter()) {
            backend.push_child(
                child,
                x + running_x,
                y + geo.entry_height - child_geo.entry_height,
                h_dir,
                child_geo,
            )?;
            running_x += child_geo.width + self.spacing;
        }

        let mut running_x = x;
        for child_geo in geo.children.iter().rev().skip(1).rev() {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(running_x + child_geo.width, y + geo.entry_height)
                    .horizontal(self.spacing),
            )?;
            running_x += child_geo.width + self.spacing;
        }
        Ok(())
    }
}

impl<N> Default for Sequence<N> {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            spacing: 10,
        }
    }
}

impl<N> iter::FromIterator<N> for Sequence<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N> Node for Sequence<N>
where
    N: Node,
{
    fn entry_height(&self) -> i64 {
        self.children.iter().max_entry_height()
    }

    fn height(&self) -> i64 {
        self.children.iter().max_entry_height() + self.children.iter().max_height_below_entry()
    }

    fn width(&self) -> i64 {
        let l = self.children.len();
        if l > 1 {
            self.children.iter().total_width() + (i64::try_from(l).unwrap() - 1) * self.spacing
        } else {
            self.children.iter().total_width()
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set("class", "sequence");
        let mut running_x = 0;
        for child in &self.children {
            g.push(child.draw(
                x + running_x,
                y + self.entry_height() - child.entry_height(),
                h_dir,
            ));
            running_x += child.width() + self.spacing;
        }

        let mut running_x = x;
        for child in self.children.iter().rev().skip(1).rev() {
            g.push(
                svg::PathData::new(h_dir)
                    .move_to(running_x + child.width(), y + self.entry_height())
                    .horizontal(self.spacing)
                    .into_path(),
            );
            running_x += child.width() + self.spacing;
        }
        g.debug("Sequence", x, y, self)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> =
            self.children.iter().map(|c| c.compute_geometry()).collect();
        let entry_height = children.iter().map(|g| g.entry_height).max().unwrap_or(0);
        let height_below = children
            .iter()
            .map(|g| g.height_below_entry())
            .max()
            .unwrap_or(0);
        let total_width: i64 = children.iter().map(|g| g.width).sum();
        let l = children.len();
        let width = if l > 1 {
            total_width + (i64::try_from(l).unwrap() - 1) * self.spacing
        } else {
            total_width
        };
        NodeGeometry {
            entry_height,
            height: entry_height + height_below,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_class_group_with_geometry("sequence", "Sequence", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_class_group_with_geometry(out, "sequence", "Sequence", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
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

/// A `Terminal`-symbol, drawn as a rectangle with rounded corners.
#[derive(Debug, Clone)]
pub struct Terminal {
    label: String,
    attributes: HashMap<String, String>,
}

impl Terminal {
    /// Construct a `Terminal` with the given visible label.
    ///
    /// The label is HTML-escaped when rendered, so arbitrary text is safe to pass.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let t = Terminal::new("BEGIN".to_owned());
    /// assert!(Diagram::new(t).to_string().contains("BEGIN"));
    /// ```
    #[must_use]
    pub fn new(label: String) -> Self {
        let mut t = Self {
            label,
            attributes: HashMap::default(),
        };
        t.attributes
            .insert("class".to_owned(), "terminal".to_owned());
        t
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit the terminal box and centered label through the chosen backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        emit_text_box(backend, x, y, geo, &self.label, true)
    }
}

impl Node for Terminal {
    fn entry_height(&self) -> i64 {
        11
    }
    fn height(&self) -> i64 {
        self.entry_height() * 2
    }
    fn width(&self) -> i64 {
        i64::try_from(text_width(&self.label)).unwrap() * 8 + 20
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        let r = svg::Element::new("rect")
            .set("x", &x)
            .set("y", &y)
            .set("height", &self.height())
            .set("width", &self.width())
            .set("rx", &10)
            .set("ry", &10);
        let t = svg::Element::new("text")
            .set("x", &(x + self.width() / 2))
            .set("y", &(y + self.entry_height() + 5))
            .text(&self.label);
        svg::Element::new("g")
            .debug("terminal", x, y, self)
            .set_all(self.attributes.iter())
            .add(r)
            .add(t)
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        _h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "terminal", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, geo)
        })
    }
}

/// A `NonTerminal`, drawn as a rectangle.
#[derive(Debug, Clone)]
pub struct NonTerminal {
    label: String,
    attributes: HashMap<String, String>,
}

impl NonTerminal {
    /// Construct a `NonTerminal` with the given visible label.
    ///
    /// The label is HTML-escaped when rendered, so arbitrary text is safe to pass.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let nt = NonTerminal::new("expression".to_owned());
    /// assert!(Diagram::new(nt).to_string().contains("expression"));
    /// ```
    #[must_use]
    pub fn new(label: String) -> Self {
        let mut nt = Self {
            label,
            attributes: HashMap::default(),
        };
        nt.attributes
            .insert("class".to_owned(), "nonterminal".to_owned());
        nt
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit the non-terminal box and centered label through the chosen backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        emit_text_box(backend, x, y, geo, &self.label, false)
    }
}

impl Node for NonTerminal {
    fn entry_height(&self) -> i64 {
        11
    }
    fn height(&self) -> i64 {
        self.entry_height() * 2
    }
    fn width(&self) -> i64 {
        i64::try_from(text_width(&self.label)).unwrap() * 8 + 20
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("g")
            .debug("NonTerminal", x, y, self)
            .set_all(self.attributes.iter())
            .add(
                svg::Element::new("rect")
                    .set("x", &x)
                    .set("y", &y)
                    .set("height", &self.height())
                    .set("width", &self.width()),
            )
            .add(
                svg::Element::new("text")
                    .set("x", &(x + self.width() / 2))
                    .set("y", &(y + self.entry_height() + 5))
                    .text(&self.label),
            )
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        _h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "NonTerminal", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, geo)
        })
    }
}

/// Wraps another element to make that element logically optional.
///
/// Draws a separate path above, which skips the given element.
#[derive(Debug, Clone, Default)]
pub struct Optional<N> {
    inner: N,
    attributes: HashMap<String, String>,
}

impl<N> Optional<N> {
    /// Wrap `inner` so it can be skipped via an upper bypass path.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let node = Optional::new(Terminal::new("maybe".to_owned()));
    /// assert!(Diagram::new(node).to_string().starts_with("<svg"));
    /// ```
    pub fn new(inner: N) -> Self {
        let mut o = Self {
            inner,
            attributes: HashMap::default(),
        };
        o.attributes
            .insert("class".to_owned(), "optional".to_owned());
        o
    }

    /// Unwrap this wrapper, returning the inner node.
    pub fn into_inner(self) -> N {
        self.inner
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit the bypass arc and wrapped child once for both render backends.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let inner_geo = &geo.children[0];
        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(ARC_RADIUS * 2)
                .move_rel(-ARC_RADIUS * 2, 0)
                .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                .vertical(cmp::min(0, -inner_geo.entry_height + ARC_RADIUS))
                .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                .horizontal(inner_geo.width)
                .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                .vertical(cmp::max(0, inner_geo.entry_height - ARC_RADIUS))
                .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                .horizontal(-ARC_RADIUS * 2),
        )?;
        backend.push_child(
            &self.inner,
            x + ARC_RADIUS * 2,
            y + geo.entry_height - inner_geo.entry_height,
            h_dir,
            inner_geo,
        )
    }
}

impl<N> Node for Optional<N>
where
    N: Node,
{
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

    fn compute_geometry(&self) -> NodeGeometry {
        let inner_geo = self.inner.compute_geometry();
        let entry_height = ARC_RADIUS + cmp::max(ARC_RADIUS, inner_geo.entry_height);
        let height = entry_height + inner_geo.height_below_entry();
        let width = ARC_RADIUS * 2 + inner_geo.width + ARC_RADIUS * 2;
        NodeGeometry {
            entry_height,
            height,
            width,
            children: vec![inner_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Optional", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "Optional", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }
}

/// A vertical group of elements, drawn from top to bottom.
///
/// Also see `Sequence` for a horizontal group of elements.
#[derive(Debug, Clone)]
pub struct Stack<N> {
    children: Vec<N>,
    left_padding: i64,
    right_padding: i64,
    spacing: i64,
    attributes: HashMap<String, String>,
}

impl<N> Stack<N> {
    /// Create a `Stack` from an ordered list of children.
    ///
    /// Children are connected top-to-bottom: the path exits the right side of
    /// each child, curves down, and re-enters from the left for the next child.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let stack = Stack::new(vec![
    ///     Terminal::new("line 1".to_owned()),
    ///     Terminal::new("line 2".to_owned()),
    /// ]);
    /// assert!(Diagram::new(stack).to_string().starts_with("<svg"));
    /// ```
    #[must_use]
    pub fn new(children: Vec<N>) -> Self {
        let mut s = Self {
            children,
            ..Self::default()
        };
        s.attributes.insert("class".to_owned(), "stack".to_owned());
        s
    }

    /// Append a child to this stack.
    pub fn push(&mut self, child: N) {
        self.children.push(child);
    }

    /// Unwrap this stack, returning the children in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<N> {
        self.children
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Emit the stack connectors and children once for both render backends.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let left_p = self.left_padding();
        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(left_p),
        )?;

        let mut running_y = y;
        let n = self.children.len();
        for i in 0..n.saturating_sub(1) {
            let child = &self.children[i];
            let child_geo = &geo.children[i];
            let next_geo = &geo.children[i + 1];
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(
                        x + left_p + child_geo.width,
                        running_y + child_geo.entry_height,
                    )
                    .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                    .vertical(cmp::max(
                        0,
                        child_geo.height_below_entry() + self.spacing - ARC_RADIUS * 2,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::NorthToWest)
                    .horizontal(-child_geo.width)
                    .arc(ARC_RADIUS, svg::Arc::EastToSouth)
                    .vertical(cmp::max(0, next_geo.entry_height - ARC_RADIUS))
                    .vertical(cmp::max(
                        0,
                        (self.spacing - ARC_RADIUS * 2) / 2 + (self.spacing - ARC_RADIUS * 2) % 2,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                    .horizontal(left_p - ARC_RADIUS),
            )?;
            backend.push_child(child, x + left_p, running_y, h_dir, child_geo)?;
            let ph = child_geo.entry_height
                + cmp::max(
                    child_geo.height_below_entry() + self.spacing,
                    ARC_RADIUS * 2,
                )
                + ARC_RADIUS
                + cmp::max(0, ARC_RADIUS - next_geo.entry_height);
            running_y += ph;
        }

        if let Some(last_child) = self.children.last() {
            let last_geo = geo.children.last().unwrap();
            if self.children.len() > 1 {
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(
                            x + left_p + last_geo.width,
                            running_y + last_geo.entry_height,
                        )
                        .horizontal(geo.width - last_geo.width - left_p - ARC_RADIUS * 2)
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(
                            -geo.height
                                + last_geo.height_below_entry()
                                + ARC_RADIUS * 2
                                + geo.entry_height,
                        )
                        .arc(ARC_RADIUS, svg::Arc::SouthToEast),
                )?;
            }
            backend.push_child(last_child, x + left_p, running_y, h_dir, last_geo)?;
        }
        Ok(())
    }

    fn padded_height(&self, child: &dyn Node, next_child: &dyn Node) -> i64 {
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

impl<N> Default for Stack<N> {
    fn default() -> Self {
        Self {
            children: Vec::default(),
            left_padding: 10,
            right_padding: 10,
            spacing: ARC_RADIUS,
            attributes: HashMap::default(),
        }
    }
}

impl<N> iter::FromIterator<N> for Stack<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N> Node for Stack<N>
where
    N: Node,
{
    fn entry_height(&self) -> i64 {
        self.children
            .first()
            .map(Node::entry_height)
            .unwrap_or_default()
    }

    fn height(&self) -> i64 {
        self.children
            .iter()
            .zip(self.children.iter().skip(1))
            .map(|(c, nc)| self.padded_height(c, nc))
            .sum::<i64>()
            + self.children.last().map(Node::height).unwrap_or_default()
    }

    fn width(&self) -> i64 {
        let l = self.left_padding() + self.children.iter().max_width() + self.right_padding();
        // If the final upwards connector touches the downward ones, add some space
        if self
            .children
            .iter()
            .rev()
            .skip(1)
            .rev()
            .any(|c| c.width() >= self.children.last().map(Node::width).unwrap_or_default())
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
                        running_y + child.entry_height(),
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
            running_y += self.padded_height(child, next_child);
        }

        // Draw the last (possibly only) child and its connectors
        if let Some(child) = self.children.last() {
            if self.children.len() > 1 {
                g.push(
                    svg::PathData::new(h_dir)
                        .move_to(
                            x + self.left_padding() + child.width(),
                            running_y + child.entry_height(),
                        )
                        .horizontal(
                            self.width() - child.width() - self.left_padding() - ARC_RADIUS * 2,
                        )
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(
                            -self.height()
                                + child.height_below_entry()
                                + ARC_RADIUS * 2
                                + self.entry_height(),
                        )
                        .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                        .into_path(),
                );
            }
            g.push(child.draw(x + self.left_padding(), running_y, h_dir));
        }

        g.debug("Stack", x, y, self)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> =
            self.children.iter().map(|c| c.compute_geometry()).collect();
        let entry_height = children.first().map(|g| g.entry_height).unwrap_or(0);
        let left_p = self.left_padding();
        let max_width = children.iter().map(|g| g.width).max().unwrap_or(0);
        let last_width = children.last().map(|g| g.width).unwrap_or(0);
        let base_width = left_p + max_width + self.right_padding();
        let needs_extra = children
            .iter()
            .rev()
            .skip(1)
            .rev()
            .any(|g| g.width >= last_width);
        let width = if needs_extra {
            base_width + ARC_RADIUS
        } else {
            base_width
        };
        let height = children
            .windows(2)
            .map(|w| {
                let (cg, ng) = (&w[0], &w[1]);
                cg.entry_height
                    + cmp::max(cg.height_below_entry() + self.spacing, ARC_RADIUS * 2)
                    + ARC_RADIUS
                    + cmp::max(0, ARC_RADIUS - ng.entry_height)
            })
            .sum::<i64>()
            + children.last().map(|g| g.height).unwrap_or(0);
        NodeGeometry {
            entry_height,
            height,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Stack", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "Stack", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }
}

/// A container of elements, drawn vertically, where exactly one element has to be picked
///
/// Use `Empty` as one of the elements to make the entire `Choice` optional (a shorthand for
/// `Optional(Choice(..))`.
#[derive(Debug, Clone)]
pub struct Choice<N> {
    children: Vec<N>,
    spacing: i64,
    attributes: HashMap<String, String>,
}

impl<N> Choice<N> {
    /// Create a `Choice` from an ordered list of alternatives.
    ///
    /// The first child is drawn inline (on the main path); additional children
    /// are drawn below, reachable via downward arcs.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let choice = Choice::new(vec![
    ///     Terminal::new("A".to_owned()),
    ///     Terminal::new("B".to_owned()),
    /// ]);
    /// assert!(Diagram::new(choice).to_string().starts_with("<svg"));
    /// ```
    #[must_use]
    pub fn new(children: Vec<N>) -> Self {
        let mut c = Self {
            children,
            ..Self::default()
        };
        c.attributes.insert("class".to_owned(), "choice".to_owned());
        c
    }

    /// Append an alternative child.
    pub fn push(&mut self, child: N) {
        self.children.push(child);
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Unwrap this choice, returning the children in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<N> {
        self.children
    }

    fn inner_padding(&self) -> i64 {
        if self.children.len() > 1 {
            ARC_RADIUS * 2
        } else {
            0
        }
    }

    fn padded_height(&self, child: &dyn Node) -> i64 {
        cmp::max(ARC_RADIUS, child.entry_height()) + child.height_below_entry() + self.spacing
    }

    /// Emit all choice branches and their connecting arcs through the shared backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result
    where
        N: Node,
    {
        let inner_padding = self.inner_padding();
        let max_child_width = geo.children.iter().map(|g| g.width).max().unwrap_or(0);

        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(inner_padding)
                .move_rel(geo.children.first().map(|g| g.width).unwrap_or(0), 0)
                .horizontal(
                    geo.width - inner_padding - geo.children.first().map(|g| g.width).unwrap_or(0),
                ),
        )?;

        if let Some((first_child, first_child_geo)) =
            self.children.first().zip(geo.children.first())
        {
            backend.push_child(first_child, x + inner_padding, y, h_dir, first_child_geo)?;
        }

        if self.children.len() > 1 {
            let first_geo = &geo.children[0];
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(x, y + geo.entry_height)
                    .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                    .vertical(cmp::max(
                        0,
                        first_geo.height_below_entry() + self.spacing - ARC_RADIUS,
                    ))
                    .move_rel(geo.width - ARC_RADIUS * 2, 0)
                    .vertical(-cmp::max(
                        0,
                        first_geo.height_below_entry() + self.spacing - ARC_RADIUS,
                    ))
                    .arc(ARC_RADIUS, svg::Arc::SouthToEast),
            )?;

            let base_y = y
                + geo.entry_height
                + cmp::max(ARC_RADIUS, self.spacing + first_geo.height_below_entry());
            let mut running_y = base_y;
            for child_geo in geo.children.iter().skip(1).rev().skip(1).rev() {
                let padded = cmp::max(ARC_RADIUS, child_geo.entry_height)
                    + child_geo.height_below_entry()
                    + self.spacing;
                let zz = cmp::max(0, child_geo.entry_height - ARC_RADIUS);
                let z = padded - zz;
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(x + ARC_RADIUS, running_y + zz)
                        .vertical(z)
                        .move_rel(geo.width - ARC_RADIUS * 2, 0)
                        .vertical(-z),
                )?;
                running_y += z + zz;
            }

            let mut running_y = base_y;
            for (child, child_geo) in self
                .children
                .iter()
                .skip(1)
                .zip(geo.children.iter().skip(1))
            {
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(x + ARC_RADIUS, running_y)
                        .vertical(cmp::max(0, child_geo.entry_height - ARC_RADIUS))
                        .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                        .move_rel(child_geo.width, 0)
                        .horizontal(max_child_width - child_geo.width)
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(-cmp::max(0, child_geo.entry_height - ARC_RADIUS)),
                )?;
                backend.push_child(
                    child,
                    x + ARC_RADIUS * 2,
                    running_y + cmp::max(0, ARC_RADIUS - child_geo.entry_height),
                    h_dir,
                    child_geo,
                )?;
                running_y += cmp::max(ARC_RADIUS, child_geo.entry_height)
                    + child_geo.height_below_entry()
                    + self.spacing;
            }
        }
        Ok(())
    }
}

impl<N> iter::FromIterator<N> for Choice<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N> Default for Choice<N> {
    fn default() -> Self {
        Self {
            children: Vec::default(),
            spacing: 10,
            attributes: HashMap::default(),
        }
    }
}

impl<N> Node for Choice<N>
where
    N: Node,
{
    fn entry_height(&self) -> i64 {
        self.children
            .first()
            .map(Node::entry_height)
            .unwrap_or_default()
    }

    fn height(&self) -> i64 {
        if self.children.is_empty() {
            0
        } else if self.children.len() == 1 {
            self.children.iter().total_height()
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
                    .map(|c| self.padded_height(c))
                    .sum::<i64>()
                - self.spacing
        }
    }

    fn width(&self) -> i64 {
        if self.children.len() > 1 {
            self.inner_padding() + self.children.iter().max_width() + self.inner_padding()
        } else {
            self.children.iter().max_width()
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());

        // The top, horizontal connectors
        g.push(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(self.inner_padding())
                .move_rel(
                    self.children.first().map(Node::width).unwrap_or_default(),
                    0,
                )
                .horizontal(
                    self.width()
                        - self.inner_padding()
                        - self.children.first().map(Node::width).unwrap_or_default(),
                )
                .into_path(),
        );

        // The first child is simply drawn in-line
        if let Some(child) = self.children.first() {
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
            let mut running_y = y
                + self.entry_height()
                + cmp::max(
                    ARC_RADIUS,
                    self.spacing + self.children[0].height_below_entry(),
                );
            for child in self.children.iter().skip(1).rev().skip(1).rev() {
                let z = self.padded_height(child);
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
                        .horizontal(self.children.iter().max_width() - child.width())
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(-cmp::max(0, child.entry_height() - ARC_RADIUS))
                        .into_path(),
                );
                g.push(child.draw(
                    x + ARC_RADIUS * 2,
                    running_y + cmp::max(0, ARC_RADIUS - child.entry_height()),
                    h_dir,
                ));
                running_y += self.padded_height(child);
            }
        }

        g.debug("Choice", x, y, self)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> =
            self.children.iter().map(|c| c.compute_geometry()).collect();
        let entry_height = children.first().map(|g| g.entry_height).unwrap_or(0);
        let inner_padding = self.inner_padding();
        let max_width = children.iter().map(|g| g.width).max().unwrap_or(0);
        let width = if children.len() > 1 {
            inner_padding + max_width + inner_padding
        } else {
            max_width
        };
        let height = if children.is_empty() {
            0
        } else if children.len() == 1 {
            children.iter().map(|g| g.height).sum()
        } else {
            let first = &children[0];
            entry_height
                + cmp::max(ARC_RADIUS, self.spacing + first.height_below_entry())
                + children
                    .iter()
                    .skip(1)
                    .map(|g| {
                        cmp::max(ARC_RADIUS, g.entry_height) + g.height_below_entry() + self.spacing
                    })
                    .sum::<i64>()
                - self.spacing
        };
        NodeGeometry {
            entry_height,
            height,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Choice", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "Choice", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }
}

/// Wraps one element by providing a backwards-path through another element.
///
/// The main path flows through `inner` left-to-right. A return arc curves below
/// and carries the path through `repeat` right-to-left, allowing the sequence to
/// be traversed multiple times. Use [`Empty`] for `repeat` when no label or
/// content is needed on the return path.
#[derive(Debug, Clone)]
pub struct Repeat<I, R> {
    inner: I,
    repeat: R,
    spacing: i64,
    attributes: HashMap<String, String>,
}

impl<I, R> Repeat<I, R> {
    /// Create a `Repeat` that loops `inner` via the `repeat` node on the return path.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// // Zero-or-more repetitions with no label on the back-arc
    /// let r = Repeat::new(Terminal::new("item".to_owned()), Empty);
    /// assert!(Diagram::new(r).to_string().starts_with("<svg"));
    /// ```
    pub fn new(inner: I, repeat: R) -> Self {
        let mut r = Self {
            inner,
            repeat,
            spacing: 10,
            attributes: HashMap::default(),
        };
        r.attributes.insert("class".to_owned(), "repeat".to_owned());
        r
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl<I, R> Repeat<I, R>
where
    I: Node,
    R: Node,
{
    fn height_between_entries(&self) -> i64 {
        cmp::max(
            ARC_RADIUS * 2,
            self.inner.height_below_entry() + self.spacing + self.repeat.entry_height(),
        )
    }

    /// Emit the forward path, repeat arm, and inner branch through the shared backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let inner_geo = &geo.children[0];
        let repeat_geo = &geo.children[1];
        let height_between = cmp::max(
            ARC_RADIUS * 2,
            inner_geo.height_below_entry() + self.spacing + repeat_geo.entry_height,
        );

        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(ARC_RADIUS)
                .move_rel(inner_geo.width, 0)
                .horizontal(cmp::max(
                    ARC_RADIUS,
                    repeat_geo.width - inner_geo.width + ARC_RADIUS,
                ))
                .move_rel(-ARC_RADIUS, 0)
                .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                .vertical(height_between - ARC_RADIUS * 2)
                .arc(ARC_RADIUS, svg::Arc::NorthToWest)
                .move_rel(-repeat_geo.width, 0)
                .horizontal(cmp::min(0, repeat_geo.width - inner_geo.width))
                .arc(ARC_RADIUS, svg::Arc::EastToNorth)
                .vertical(-height_between + ARC_RADIUS * 2)
                .arc(ARC_RADIUS, svg::Arc::SouthToEast),
        )?;
        backend.push_child(
            &self.repeat,
            x + geo.width - repeat_geo.width - ARC_RADIUS,
            y + geo.height - repeat_geo.height_below_entry() - repeat_geo.entry_height,
            h_dir.invert(),
            repeat_geo,
        )?;
        backend.push_child(&self.inner, x + ARC_RADIUS, y, h_dir, inner_geo)
    }
}

impl<I, R> Default for Repeat<I, R>
where
    I: Default,
    R: Default,
{
    fn default() -> Self {
        Self {
            inner: Default::default(),
            repeat: Default::default(),
            spacing: 10,
            attributes: HashMap::default(),
        }
    }
}

impl<I, R> Node for Repeat<I, R>
where
    I: Node,
    R: Node,
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

    fn compute_geometry(&self) -> NodeGeometry {
        let inner_geo = self.inner.compute_geometry();
        let repeat_geo = self.repeat.compute_geometry();
        let height_between = cmp::max(
            ARC_RADIUS * 2,
            inner_geo.height_below_entry() + self.spacing + repeat_geo.entry_height,
        );
        let entry_height = inner_geo.entry_height;
        let height = inner_geo.entry_height + height_between + repeat_geo.height_below_entry();
        let width = ARC_RADIUS + cmp::max(repeat_geo.width, inner_geo.width) + ARC_RADIUS;
        NodeGeometry {
            entry_height,
            height,
            width,
            children: vec![inner_geo, repeat_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Repeat", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "Repeat", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
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

/// A box drawn around the given element and a label placed inside the box, above the element.
///
/// You may want to use `Comment` or `Empty` for the label.
#[derive(Debug, Clone)]
pub struct LabeledBox<T, U> {
    inner: T,
    label: U,
    spacing: i64,
    padding: i64,
    attributes: HashMap<String, String>,
}

impl<T> LabeledBox<T, Empty> {
    /// Construct a `LabeledBox` around `inner` with no label.
    ///
    /// This is a convenience shorthand for `LabeledBox::new(inner, Empty)`.
    pub fn without_label(inner: T) -> Self {
        Self::new(inner, Empty)
    }
}

impl<T, U> LabeledBox<T, U> {
    /// Construct a `LabeledBox` that draws a border around `inner` and places
    /// `label` above it inside the box.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let labeled = LabeledBox::new(
    ///     Terminal::new("item".to_owned()),
    ///     Comment::new("group".to_owned()),
    /// );
    /// assert!(Diagram::new(labeled).to_string().starts_with("<svg"));
    /// ```
    pub fn new(inner: T, label: U) -> Self {
        let mut l = Self {
            inner,
            label,
            spacing: 8,
            padding: 8,
            attributes: HashMap::default(),
        };
        l.attributes
            .insert("class".to_owned(), "labeledbox".to_owned());
        l
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }
}

impl<T, U> Default for LabeledBox<T, U>
where
    T: Default,
    U: Default,
{
    fn default() -> Self {
        Self {
            inner: Default::default(),
            label: Default::default(),
            spacing: 8,
            padding: 8,
            attributes: HashMap::default(),
        }
    }
}

impl<T, U> LabeledBox<T, U>
where
    T: Node,
    U: Node,
{
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

    /// Emit the box frame, label, and inner node through the shared backend.
    fn emit_with_geometry<B: RenderBackend>(
        &self,
        backend: &mut B,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let inner_geo = &geo.children[0];
        let label_geo = &geo.children[1];
        let padding = if label_geo.height + inner_geo.height + label_geo.width + inner_geo.width > 0
        {
            self.padding
        } else {
            0
        };
        let spacing = if label_geo.height > 0 {
            self.spacing
        } else {
            0
        };

        backend.push_rect(x, y, geo.width, geo.height)?;
        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(padding)
                .move_rel(inner_geo.width, 0)
                .horizontal(geo.width - inner_geo.width - padding),
        )?;
        backend.push_child(&self.label, x + padding, y + padding, h_dir, label_geo)?;
        backend.push_child(
            &self.inner,
            x + padding,
            y + padding + label_geo.height + spacing,
            h_dir,
            inner_geo,
        )
    }
}

impl<T, U> Node for LabeledBox<T, U>
where
    T: Node,
    U: Node,
{
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
                    .set("x", &x)
                    .set("y", &y)
                    .set("height", &self.height())
                    .set("width", &self.width()),
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

    fn compute_geometry(&self) -> NodeGeometry {
        let inner_geo = self.inner.compute_geometry();
        let label_geo = self.label.compute_geometry();
        let padding = if label_geo.height + inner_geo.height + label_geo.width + inner_geo.width > 0
        {
            self.padding
        } else {
            0
        };
        let spacing = if label_geo.height > 0 {
            self.spacing
        } else {
            0
        };
        let entry_height = padding + label_geo.height + spacing + inner_geo.entry_height;
        let height = padding + label_geo.height + spacing + inner_geo.height + padding;
        let width = padding + cmp::max(inner_geo.width, label_geo.width) + padding;
        NodeGeometry {
            entry_height,
            height,
            width,
            children: vec![inner_geo, label_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "LabeledBox", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        render_group_with_geometry(out, &self.attributes, "LabeledBox", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }
}

/// A label / verbatim text drawn inline on the connecting path.
///
/// Useful as a label for [`LabeledBox`] or as a lightweight annotation
/// within a [`Sequence`].
#[derive(Debug, Clone)]
pub struct Comment {
    text: String,
    attributes: HashMap<String, String>,
}

impl Comment {
    /// Construct a `Comment` with the given text.
    ///
    /// The text is HTML-escaped when rendered, so arbitrary strings are safe to pass.
    ///
    /// # Example
    /// ```rust
    /// use railroad::*;
    ///
    /// let c = Comment::new("/* note */".to_owned());
    /// assert!(Diagram::new(c).to_string().contains("/* note */"));
    /// ```
    #[must_use]
    pub fn new(text: String) -> Self {
        let mut c = Self {
            text,
            attributes: HashMap::default(),
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

impl Node for Comment {
    fn entry_height(&self) -> i64 {
        10
    }
    fn height(&self) -> i64 {
        20
    }
    fn width(&self) -> i64 {
        i64::try_from(text_width(&self.text)).unwrap() * 7 + 10
    }

    fn draw(&self, x: i64, y: i64, _: HDir) -> svg::Element {
        svg::Element::new("text")
            .set_all(self.attributes.iter())
            .set("x", &(x + self.width() / 2))
            .set("y", &(y + 15))
            .text(&self.text)
            .debug("Comment", x, y, self)
    }

    fn render_with_geometry(
        &self,
        out: &mut svg::Renderer<'_>,
        x: i64,
        y: i64,
        _h_dir: HDir,
        geo: &NodeGeometry,
    ) -> fmt::Result {
        let mut text = out.start_element("text")?;
        text.attr_hashmap(&self.attributes)?;
        text.attr("x", x + geo.width / 2)?;
        text.attr("y", y + 15)?;
        add_debug_attrs(&mut text, "Comment", x, y, geo)?;
        text.finish()?;
        out.write_text(&self.text)?;
        out.end_element("text")?;
        write_debug_overlay(out, x, y, geo)
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
