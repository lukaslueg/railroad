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
                    .horizontal(left_p - ARC_RADIUS)
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
