use std::{
    cmp,
    collections::{self, HashMap},
    fmt, iter,
};

use crate::{
    ARC_RADIUS, HDir, Node, NodeGeometry, RenderBackend, draw_class_group_with_geometry,
    draw_group_with_geometry, render_class_group_with_geometry, render_group_with_geometry, svg,
};

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
        self.children
            .iter()
            .map(Node::entry_height)
            .max()
            .unwrap_or_default()
    }

    fn height(&self) -> i64 {
        self.children
            .iter()
            .map(Node::entry_height)
            .max()
            .unwrap_or_default()
            + self
                .children
                .iter()
                .map(Node::height_below_entry)
                .max()
                .unwrap_or_default()
    }

    fn width(&self) -> i64 {
        let l = self.children.len();
        if l > 1 {
            self.children.iter().map(Node::width).sum::<i64>()
                + (i64::try_from(l).unwrap() - 1) * self.spacing
        } else {
            self.children.iter().map(Node::width).sum()
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
                    .arc(ARC_RADIUS, svg::Arc::NorthToEast),
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
}

impl<N> Default for Stack<N> {
    fn default() -> Self {
        Self {
            children: Vec::default(),
            left_padding: ARC_RADIUS * 2,
            right_padding: ARC_RADIUS * 2,
            spacing: 10,
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
        self.children.first().map(Node::entry_height).unwrap_or(0)
    }

    fn height(&self) -> i64 {
        self.children
            .windows(2)
            .map(|w| self.padded_height(&w[0], &w[1]))
            .sum::<i64>()
            + self.children.last().map(Node::height).unwrap_or(0)
    }

    fn width(&self) -> i64 {
        let left_p = self.left_padding();
        let max_width = self.children.iter().map(Node::width).max().unwrap_or(0);
        let last_width = self.children.last().map(Node::width).unwrap_or(0);
        let base_width = left_p + max_width + self.right_padding;
        let needs_extra = self
            .children
            .iter()
            .rev()
            .skip(1)
            .rev()
            .any(|c| c.width() >= last_width);
        if needs_extra {
            base_width + ARC_RADIUS
        } else {
            base_width
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let left_p = self.left_padding();
        let mut g = svg::Element::new("g").set_all(self.attributes.iter()).add(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(left_p)
                .into_path(),
        );

        let mut running_y = y;
        let n = self.children.len();
        for i in 0..n.saturating_sub(1) {
            let child = &self.children[i];
            let next_child = &self.children[i + 1];
            g.push(
                svg::PathData::new(h_dir)
                    .move_to(x + left_p + child.width(), running_y + child.entry_height())
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
                    .into_path(),
            );
            g.push(child.draw(x + left_p, running_y, h_dir));
            running_y += self.padded_height(child, next_child);
        }

        if let Some(last_child) = self.children.last() {
            if self.children.len() > 1 {
                g.push(
                    svg::PathData::new(h_dir)
                        .move_to(
                            x + left_p + last_child.width(),
                            running_y + last_child.entry_height(),
                        )
                        .horizontal(self.width() - last_child.width() - left_p - ARC_RADIUS * 2)
                        .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                        .vertical(
                            -self.height()
                                + last_child.height_below_entry()
                                + ARC_RADIUS * 2
                                + self.entry_height(),
                        )
                        .arc(ARC_RADIUS, svg::Arc::SouthToEast)
                        .into_path(),
                );
            }
            g.push(last_child.draw(x + left_p, running_y, h_dir));
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
        let base_width = left_p + max_width + self.right_padding;
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
            self.children.iter().map(Node::height).sum()
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
            self.inner_padding()
                + self.children.iter().map(Node::width).max().unwrap_or(0)
                + self.inner_padding()
        } else {
            self.children.iter().map(Node::width).max().unwrap_or(0)
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());

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

        if let Some(child) = self.children.first() {
            g.push(child.draw(x + self.inner_padding(), y, h_dir));
        }

        if self.children.len() > 1 {
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
                        .horizontal(
                            self.children.iter().map(Node::width).max().unwrap_or(0)
                                - child.width(),
                        )
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

#[derive(Debug, Clone)]
struct MultiChoiceColumnLayout {
    flat_start: usize,
    flat_end: usize,
    x_offset: i64,
    y_offset: i64,
    width: i64,
    height: i64,
    entry_height: i64,
    max_child_width: i64,
    child_y_offsets: Vec<i64>,
}

#[derive(Debug, Clone)]
struct MultiChoiceLayout {
    columns: Vec<MultiChoiceColumnLayout>,
    top_padding: i64,
    exit_gutter: i64,
}

/// A multi-column container where exactly one child alternative has to be picked.
///
/// `MultiChoice` is a column-first generalization of [`Choice`]. Each inner
/// vector is drawn as one vertical column of alternatives.
#[derive(Debug, Clone)]
pub struct MultiChoice<N> {
    columns: Vec<Vec<N>>,
    spacing: i64,
    column_spacing: i64,
    attributes: HashMap<String, String>,
}

impl<N> MultiChoice<N> {
    /// Create a `MultiChoice` from ordered columns of alternatives.
    ///
    /// Empty columns are ignored for layout. With no non-empty columns, or with
    /// one non-empty column, the node uses `Choice`-compatible geometry.
    #[must_use]
    pub fn new(columns: Vec<Vec<N>>) -> Self {
        let mut c = Self {
            columns,
            ..Self::default()
        };
        c.attributes
            .insert("class".to_owned(), "multichoice".to_owned());
        c
    }

    /// Append a column of alternatives.
    pub fn push_column(&mut self, column: Vec<N>) {
        self.columns.push(column);
    }

    /// Access an attribute on the main SVG-element that will be drawn.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    /// Unwrap this multi-choice, returning the columns in order.
    #[must_use]
    pub fn into_inner(self) -> Vec<Vec<N>> {
        self.columns
    }

    fn active_column_count(&self) -> usize {
        self.columns.iter().filter(|c| !c.is_empty()).count()
    }

    fn choice_inner_padding(child_count: usize) -> i64 {
        if child_count > 1 { ARC_RADIUS * 2 } else { 0 }
    }

    fn choice_column_height(children: &[NodeGeometry], spacing: i64) -> i64 {
        if children.is_empty() {
            0
        } else if children.len() == 1 {
            children[0].height
        } else {
            let first = &children[0];
            first.entry_height
                + cmp::max(ARC_RADIUS, spacing + first.height_below_entry())
                + children
                    .iter()
                    .skip(1)
                    .map(|g| {
                        cmp::max(ARC_RADIUS, g.entry_height) + g.height_below_entry() + spacing
                    })
                    .sum::<i64>()
                - spacing
        }
    }

    fn choice_child_y_offsets(children: &[NodeGeometry], spacing: i64) -> Vec<i64> {
        if children.is_empty() {
            Vec::new()
        } else if children.len() == 1 {
            vec![0]
        } else {
            let mut offsets = Vec::with_capacity(children.len());
            offsets.push(0);
            let first = &children[0];
            let mut running_y =
                first.entry_height + cmp::max(ARC_RADIUS, spacing + first.height_below_entry());
            for child in children.iter().skip(1) {
                offsets.push(running_y + cmp::max(0, ARC_RADIUS - child.entry_height));
                running_y +=
                    cmp::max(ARC_RADIUS, child.entry_height) + child.height_below_entry() + spacing;
            }
            offsets
        }
    }

    fn build_layout(&self, child_geometries: &[NodeGeometry]) -> MultiChoiceLayout {
        let active_count = self.active_column_count();
        let mut top_padding = if active_count > 1 { ARC_RADIUS * 2 } else { 0 };
        let exit_gutter = if active_count > 1 { ARC_RADIUS } else { 0 };
        let mut flat_start = 0;
        let mut x_offset = 0;
        let mut columns = Vec::with_capacity(active_count);

        for column in &self.columns {
            let flat_end = flat_start + column.len();
            let column_geometries = &child_geometries[flat_start..flat_end];
            if !column_geometries.is_empty() {
                let max_child_width = column_geometries.iter().map(|g| g.width).max().unwrap_or(0);
                let width = if active_count > 1 {
                    ARC_RADIUS * 2 + max_child_width + ARC_RADIUS
                } else {
                    let inner_padding = Self::choice_inner_padding(column_geometries.len());
                    inner_padding + max_child_width + inner_padding
                };
                let height = Self::choice_column_height(column_geometries, self.spacing);
                let entry_height = column_geometries[0].entry_height;
                let child_y_offsets = Self::choice_child_y_offsets(column_geometries, self.spacing);
                columns.push(MultiChoiceColumnLayout {
                    flat_start,
                    flat_end,
                    x_offset,
                    y_offset: top_padding,
                    width,
                    height,
                    entry_height,
                    max_child_width,
                    child_y_offsets,
                });
                x_offset += width + self.column_spacing;
            }
            flat_start = flat_end;
        }

        if columns.len() > 1 {
            let top_entry_clearance = cmp::max(0, ARC_RADIUS - columns[0].entry_height);
            if top_entry_clearance > 0 {
                top_padding += top_entry_clearance;
                for column in &mut columns {
                    column.y_offset += top_entry_clearance;
                }
            }

            let node_entry_y = top_padding + columns[0].entry_height;
            for column in columns.iter_mut().skip(1) {
                let column_entry_y = column.y_offset + column.entry_height;
                let entry_gap = column_entry_y - node_entry_y;
                if entry_gap != 0 && entry_gap < ARC_RADIUS * 2 {
                    column.y_offset += ARC_RADIUS * 2 - entry_gap;
                }
            }
        }

        MultiChoiceLayout {
            columns,
            top_padding,
            exit_gutter,
        }
    }

    fn emit_sectioned_vertical<B: RenderBackend>(
        backend: &mut B,
        h_dir: HDir,
        x: i64,
        start_y: i64,
        section_ends: &[i64],
        final_y: i64,
    ) -> fmt::Result {
        let mut running_y = start_y;
        for section_end in section_ends {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(x, running_y)
                    .vertical(section_end - running_y),
            )?;
            running_y = *section_end;
        }
        if final_y > running_y {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(x, running_y)
                    .vertical(final_y - running_y),
            )?;
        }
        Ok(())
    }

    fn emit_sectioned_horizontal<B: RenderBackend>(
        backend: &mut B,
        h_dir: HDir,
        y: i64,
        start_x: i64,
        section_ends: &[i64],
        final_x: i64,
    ) -> fmt::Result {
        let mut running_x = start_x;
        for section_end in section_ends {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(running_x, y)
                    .horizontal(section_end - running_x),
            )?;
            running_x = *section_end;
        }
        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(running_x, y)
                .horizontal(final_x - running_x),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_incoming_column_spine<B: RenderBackend>(
        backend: &mut B,
        h_dir: HDir,
        column_x: i64,
        branch_x: i64,
        column_entry_y: i64,
        row_branch_ys: &[i64],
        spine_bottom_y: i64,
        is_first_active_column: bool,
    ) -> fmt::Result {
        if is_first_active_column {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(column_x, column_entry_y)
                    .arc(ARC_RADIUS, svg::Arc::WestToSouth),
            )?;
            Self::emit_sectioned_vertical(
                backend,
                h_dir,
                branch_x,
                column_entry_y + ARC_RADIUS,
                row_branch_ys,
                spine_bottom_y,
            )
        } else {
            Self::emit_sectioned_vertical(
                backend,
                h_dir,
                branch_x,
                column_entry_y - ARC_RADIUS,
                row_branch_ys,
                spine_bottom_y,
            )
        }
    }

    /// Emit all branches and their connecting routes through the shared backend.
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
        let layout = self.build_layout(&geo.children);

        // Empty case: preserve Choice-compatible geometry and draw a zero-length path.
        if layout.columns.is_empty() {
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(x, y + geo.entry_height)
                    .horizontal(geo.width),
            )?;
            return Ok(());
        }

        let active_count = layout.columns.len();
        let exit_x = x + geo.width;
        let exit_y = y + geo.entry_height;
        let route_y = y + geo.height - ARC_RADIUS;
        let final_join_x = exit_x - ARC_RADIUS * 2;
        let final_spine_x = exit_x - ARC_RADIUS;
        let mut underpass_join_xs = Vec::new();
        let mut final_merge_starts = Vec::new();
        let mut flat_index = 0;

        if active_count > 1 {
            let top_y = y + ARC_RADIUS;
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(x, exit_y)
                    .arc(ARC_RADIUS, svg::Arc::WestToNorth)
                    .vertical(top_y - exit_y + ARC_RADIUS * 2)
                    .arc(ARC_RADIUS, svg::Arc::SouthToEast),
            )?;

            if let Some((last_column, earlier_columns)) = layout.columns[1..].split_last() {
                let section_ends: Vec<i64> = earlier_columns
                    .iter()
                    .map(|column_layout| x + column_layout.x_offset)
                    .collect();
                Self::emit_sectioned_horizontal(
                    backend,
                    h_dir,
                    top_y,
                    x + ARC_RADIUS * 2,
                    &section_ends,
                    x + last_column.x_offset,
                )?;
            }
        }

        for column in &self.columns {
            let flat_end = flat_index + column.len();
            let Some(column_layout) = layout
                .columns
                .iter()
                .find(|layout| layout.flat_start == flat_index && layout.flat_end == flat_end)
            else {
                flat_index = flat_end;
                continue;
            };

            let column_x = x + column_layout.x_offset;
            let column_y = y + column_layout.y_offset;
            let column_entry_y = column_y + column_layout.entry_height;
            let left_padding = if active_count > 1 {
                ARC_RADIUS * 2
            } else {
                Self::choice_inner_padding(column.len())
            };
            let branch_x = column_x + cmp::min(ARC_RADIUS, left_padding);
            let child_x = column_x + left_padding;
            let is_final_column =
                column_layout.flat_start == layout.columns.last().unwrap().flat_start;
            let route_x = column_x + column_layout.width;
            let mut column_merge_starts = Vec::new();

            // First active column: the node entry splits downward directly.
            if column_layout.flat_start == layout.columns[0].flat_start {
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(x, exit_y)
                        .horizontal(child_x - x),
                )?;
            // Later columns: route from the node entry above the first column, then branch down.
            } else {
                let top_y = y + ARC_RADIUS;
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(branch_x - ARC_RADIUS, top_y)
                        .arc(ARC_RADIUS, svg::Arc::WestToSouth)
                        .vertical(column_entry_y - top_y - ARC_RADIUS * 2)
                        .arc(ARC_RADIUS, svg::Arc::NorthToEast),
                )?;
            }

            // Multi-row column: add the vertical branch spine, mirroring Choice-style rows.
            if column.len() > 1 {
                let last_child_geo = &geo.children[flat_end - 1];
                let last_child_y = column_y + column_layout.child_y_offsets[column.len() - 1];
                let spine_bottom_y = last_child_y + last_child_geo.entry_height - ARC_RADIUS;
                let row_branch_ys: Vec<i64> = (1..column.len())
                    .map(|row_index| {
                        let child_geo = &geo.children[flat_index + row_index];
                        let child_y = column_y + column_layout.child_y_offsets[row_index];
                        child_y + child_geo.entry_height - ARC_RADIUS
                    })
                    .collect();
                Self::emit_incoming_column_spine(
                    backend,
                    h_dir,
                    column_x,
                    branch_x,
                    column_entry_y,
                    &row_branch_ys,
                    spine_bottom_y,
                    column_layout.flat_start == layout.columns[0].flat_start,
                )?;
            }

            for (row_index, child) in column.iter().enumerate() {
                let child_geo = &geo.children[flat_index + row_index];
                let child_y = column_y + column_layout.child_y_offsets[row_index];
                let child_entry_y = child_y + child_geo.entry_height;

                // Non-first rows enter from the column branch spine.
                if row_index > 0 {
                    backend.push_path(
                        svg::PathData::new(h_dir)
                            .move_to(branch_x, child_entry_y - ARC_RADIUS)
                            .arc(ARC_RADIUS, svg::Arc::NorthToEast)
                            .horizontal(child_x - branch_x - ARC_RADIUS),
                    )?;
                }

                backend.push_child(child, child_x, child_y, h_dir, child_geo)?;

                let child_right_x = child_x + child_geo.width;
                let padded_right_x = child_x + column_layout.max_child_width;

                // Single active column: exit exactly like Choice, straight to this node's end.
                if active_count == 1 {
                    // First row: the main Choice-compatible path exits straight through.
                    if row_index == 0 {
                        backend.push_path(
                            svg::PathData::new(h_dir)
                                .move_to(child_right_x, child_entry_y)
                                .horizontal(exit_x - child_right_x),
                        )?;
                    // Lower rows: curve upward into the shared right-side exit.
                    } else {
                        backend.push_path(
                            svg::PathData::new(h_dir)
                                .move_to(child_right_x, child_entry_y)
                                .horizontal(padded_right_x - child_right_x)
                                .arc(ARC_RADIUS, svg::Arc::WestToNorth),
                        )?;
                        final_merge_starts.push(child_entry_y - ARC_RADIUS);
                    }
                // Final column in a multi-column node: alternatives merge into the final exit.
                } else if is_final_column {
                    // Row already on the node exit line: keep the final exit straight.
                    if child_entry_y == exit_y {
                        backend.push_path(
                            svg::PathData::new(h_dir)
                                .move_to(child_right_x, child_entry_y)
                                .horizontal(padded_right_x - child_right_x)
                                .horizontal(exit_x - padded_right_x),
                        )?;
                    // Vertically offset rows: curve into the shared final upward connector.
                    } else {
                        backend.push_path(
                            svg::PathData::new(h_dir)
                                .move_to(child_right_x, child_entry_y)
                                .horizontal(padded_right_x - child_right_x)
                                .arc(ARC_RADIUS, svg::Arc::WestToNorth),
                        )?;
                        final_merge_starts.push(child_entry_y - ARC_RADIUS);
                    }
                // Earlier columns: exit right, merge down, route below later columns, then rise to exit.
                } else {
                    backend.push_path(
                        svg::PathData::new(h_dir)
                            .move_to(child_right_x, child_entry_y)
                            .horizontal(padded_right_x - child_right_x)
                            .arc(ARC_RADIUS, svg::Arc::WestToSouth),
                    )?;
                    column_merge_starts.push(child_entry_y + ARC_RADIUS);
                }
            }

            if active_count > 1 && !is_final_column {
                column_merge_starts.sort_unstable();
                column_merge_starts.dedup();
                if let Some((&first_start, rest)) = column_merge_starts.split_first() {
                    let mut running_y = first_start;
                    for next_y in rest {
                        backend.push_path(
                            svg::PathData::new(h_dir)
                                .move_to(route_x, running_y)
                                .vertical(next_y - running_y),
                        )?;
                        running_y = *next_y;
                    }
                    backend.push_path(
                        svg::PathData::new(h_dir)
                            .move_to(route_x, running_y)
                            .vertical(route_y - ARC_RADIUS - running_y)
                            .arc(ARC_RADIUS, svg::Arc::NorthToEast),
                    )?;
                    underpass_join_xs.push(route_x + ARC_RADIUS);
                }
            }

            flat_index = flat_end;
        }

        underpass_join_xs.sort_unstable();
        underpass_join_xs.dedup();
        if let Some((&first_join_x, rest)) = underpass_join_xs.split_first() {
            let mut running_x = first_join_x;
            for next_x in rest {
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(running_x, route_y)
                        .horizontal(next_x - running_x),
                )?;
                running_x = *next_x;
            }
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(running_x, route_y)
                    .horizontal(final_join_x - running_x)
                    .arc(ARC_RADIUS, svg::Arc::WestToNorth),
            )?;
            final_merge_starts.push(route_y - ARC_RADIUS);
        }

        final_merge_starts.sort_unstable();
        final_merge_starts.dedup();
        final_merge_starts.retain(|start_y| *start_y >= exit_y + ARC_RADIUS);
        if let Some((&lowest_start, rest)) = final_merge_starts.split_last() {
            let mut running_y = lowest_start;
            for next_y in rest.iter().rev() {
                backend.push_path(
                    svg::PathData::new(h_dir)
                        .move_to(final_spine_x, running_y)
                        .vertical(next_y - running_y),
                )?;
                running_y = *next_y;
            }
            backend.push_path(
                svg::PathData::new(h_dir)
                    .move_to(final_spine_x, running_y)
                    .vertical(exit_y + ARC_RADIUS - running_y)
                    .arc(ARC_RADIUS, svg::Arc::SouthToEast),
            )?;
        }

        Ok(())
    }
}

impl<N> iter::FromIterator<Vec<N>> for MultiChoice<N> {
    fn from_iter<T: IntoIterator<Item = Vec<N>>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<N> Default for MultiChoice<N> {
    fn default() -> Self {
        Self {
            columns: Vec::default(),
            spacing: 10,
            column_spacing: ARC_RADIUS,
            attributes: HashMap::default(),
        }
    }
}

impl<N> Node for MultiChoice<N>
where
    N: Node,
{
    fn entry_height(&self) -> i64 {
        self.compute_geometry().entry_height
    }

    fn height(&self) -> i64 {
        self.compute_geometry().height
    }

    fn width(&self) -> i64 {
        self.compute_geometry().width
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let geo = self.compute_geometry();
        self.draw_with_geometry(x, y, h_dir, &geo)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let children: Vec<NodeGeometry> = self
            .columns
            .iter()
            .flat_map(|column| column.iter().map(|child| child.compute_geometry()))
            .collect();
        let layout = self.build_layout(&children);

        // Empty case: match empty Choice geometry.
        if layout.columns.is_empty() {
            return NodeGeometry {
                entry_height: 0,
                height: 0,
                width: 0,
                children,
            };
        }

        // Single-column case: keep geometry compatible with Choice.
        if layout.columns.len() == 1 {
            let column = &layout.columns[0];
            return NodeGeometry {
                entry_height: column.entry_height,
                height: column.height,
                width: column.width,
                children,
            };
        }

        // Multi-column case: reserve top space for cross-column entry routes and
        // a right gutter for alternatives that must route around later columns.
        let width = layout.columns.last().map_or(0, |column| {
            column.x_offset + column.width + layout.exit_gutter
        });
        let max_column_bottom = layout
            .columns
            .iter()
            .map(|column| column.y_offset + column.height)
            .max()
            .unwrap_or(0);
        NodeGeometry {
            entry_height: layout.top_padding + layout.columns[0].entry_height,
            height: max_column_bottom + ARC_RADIUS * 2,
            width,
            children,
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "MultiChoice", x, y, geo, |backend| {
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
        render_group_with_geometry(out, &self.attributes, "MultiChoice", x, y, geo, |backend| {
            self.emit_with_geometry(backend, x, y, h_dir, geo)
        })
    }
}
