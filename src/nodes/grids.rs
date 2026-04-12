use std::{
    cmp,
    collections::{self, HashMap},
    fmt, iter,
};

use crate::{
    HDir, Node, NodeGeometry, RenderBackend, draw_group_with_geometry, render_group_with_geometry,
};

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
    /// any path; use [`crate::Stack`] for connected vertical sequences.
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
        h_dir: crate::svg::HDir,
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
            spacing: crate::ARC_RADIUS,
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
        self.children.iter().map(Node::height).sum::<i64>()
            + ((cmp::max(1, i64::try_from(self.children.len()).unwrap()) - 1) * self.spacing)
    }

    fn width(&self) -> i64 {
        self.children.iter().map(Node::width).max().unwrap_or(0)
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> crate::svg::Element {
        let mut g = crate::svg::Element::new("g").set_all(self.attributes.iter());
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

    fn draw_with_geometry(
        &self,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> crate::svg::Element {
        draw_group_with_geometry(&self.attributes, "VerticalGrid", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut crate::svg::Renderer<'_>,
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
    /// any path; use [`crate::Sequence`] for connected horizontal sequences.
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
            spacing: crate::ARC_RADIUS,
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
        self.children.iter().map(Node::height).max().unwrap_or(0)
    }

    fn width(&self) -> i64 {
        self.children.iter().map(Node::width).sum::<i64>()
            + ((cmp::max(1, i64::try_from(self.children.len()).unwrap()) - 1) * self.spacing)
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> crate::svg::Element {
        let mut g = crate::svg::Element::new("g").set_all(self.attributes.iter());
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

    fn draw_with_geometry(
        &self,
        x: i64,
        y: i64,
        h_dir: HDir,
        geo: &NodeGeometry,
    ) -> crate::svg::Element {
        draw_group_with_geometry(&self.attributes, "HorizontalGrid", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }

    fn render_with_geometry(
        &self,
        out: &mut crate::svg::Renderer<'_>,
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
