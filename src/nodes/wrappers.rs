use std::{
    cmp,
    collections::{self, HashMap},
    fmt,
};

use crate::{
    ARC_RADIUS, Empty, HDir, Node, NodeGeometry, RenderBackend, draw_group_with_geometry,
    render_group_with_geometry, svg,
};

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
        let mut backend = crate::ElementBackend::default();
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
        crate::add_debug_attrs(&mut a, "Link", x, y, geo)?;
        a.finish()?;
        self.emit_with_geometry(&mut crate::RendererBackend { out }, x, y, h_dir, geo)?;
        crate::write_debug_overlay(out, x, y, geo)?;
        out.end_element("a")
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

/// A box drawn around the given element and a label placed inside the box, above the element.
///
/// You may want to use `crate::Comment` or `Empty` for the label.
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
