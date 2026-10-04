use std::{
    cmp,
    collections::{self, HashMap},
    fmt,
};

use crate::{
    ARC_RADIUS, Empty, HDir, Node, NodeGeometry, RenderBackend, draw_group_with_geometry,
    render_group_with_geometry, svg,
};

/// Uniformly scale a child.
///
/// A zero factor collapses the node to zero dimensions and draws nothing.
///
/// For positive factors, [`Scale::new`] rounds the target width to the nearest
/// positive integer (ties upward), then derives a uniform scale from that width.
/// It shifts the child down by less than one unit so its rail matches the
/// rounded-up entry height.
///
/// [`Scale::new_precise`] uses the requested scale and adds no corrective
/// translation. Its integer geometry is rounded upward, so rendered rails may
/// differ from reported connecting coordinates by less than one unit.
#[derive(Debug, Clone)]
pub struct Scale<N> {
    inner: N,
    requested_scale: f64,
    precise: bool,
    attributes: HashMap<String, String>,
}

struct ScaleLayout {
    width: i64,
    height: i64,
    entry_height: i64,
    factor: f64,
    vertical_offset: f64,
}

impl<N> Scale<N> {
    /// Scale `inner`, adjusting the factor and position to align connecting rails.
    ///
    /// # Panics
    /// Panics if `scale` is negative or not finite. Geometry computation
    /// panics if the scaled dimensions cannot be represented by [`NodeGeometry`].
    ///
    /// # Example
    /// ```rust
    /// use railroad::{Node, Scale, SimpleStart};
    ///
    /// let unscaled = SimpleStart;
    /// assert_eq!(unscaled.width(), 15);
    /// assert_eq!(unscaled.entry_height(), 5);
    /// assert_eq!(unscaled.height(), 10);
    ///
    /// let scaled = Scale::new(unscaled, 0.75);
    /// // The target width is 11.25, rounded to 11; the effective factor is 11 / 15.
    /// assert_eq!(scaled.width(), 11);
    /// assert_eq!(scaled.requested_scale(), 0.75);
    /// assert_eq!(scaled.effective_scale(), 11.0 / 15.0);
    /// // Each half is now 11 / 3 units tall, rounded upward to 4.
    /// // The child shifts down by 1 / 3 units, with another 1 / 3 below it.
    /// assert_eq!(scaled.entry_height(), 4);
    /// assert_eq!(scaled.height(), 8);
    /// ```
    #[must_use]
    pub fn new(inner: N, scale: f64) -> Self {
        let mut node = Self::new_precise(inner, scale);
        node.precise = false;
        node
    }

    /// Scale `inner` by exactly the requested factor, without corrective translation.
    ///
    /// Width, height, and entry height are rounded upward for integer layout.
    ///
    /// # Panics
    /// Panics if `scale` is negative or not finite. Geometry computation
    /// panics if the scaled dimensions cannot be represented by [`NodeGeometry`].
    ///
    /// # Example
    /// ```rust
    /// use railroad::{Node, Scale, Terminal};
    ///
    /// let node = Scale::new_precise(Terminal::new("abc".to_owned()), 0.5);
    /// assert_eq!(node.requested_scale(), node.effective_scale());
    /// assert_eq!(node.height(), 11);
    /// assert_eq!(node.entry_height(), 6); // The rendered rail remains at 5.5.
    /// ```
    #[must_use]
    pub fn new_precise(inner: N, scale: f64) -> Self {
        assert!(
            scale.is_finite() && scale >= 0.0,
            "scale must be finite and nonnegative"
        );
        Self {
            inner,
            requested_scale: scale,
            precise: true,
            attributes: HashMap::from([("class".to_owned(), "scale".to_owned())]),
        }
    }

    /// Return the factor supplied to the constructor.
    #[must_use]
    pub fn requested_scale(&self) -> f64 {
        self.requested_scale
    }

    /// Return the wrapped child.
    #[must_use]
    pub fn into_inner(self) -> N {
        self.inner
    }

    /// Return the entry for `key` in the outer `<g>` element's attributes.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn integer_dimension(value: f64) -> i64 {
        assert!(
            value.is_finite() && value >= 0.0 && value < i64::MAX as f64,
            "scaled dimension exceeds NodeGeometry's range"
        );
        value as i64
    }

    fn scaled_width(&self, width: i64) -> i64 {
        let target = self.requested_scale * width as f64;
        if self.precise {
            Self::integer_dimension(target.ceil())
        } else if width == 0 {
            0
        } else {
            Self::integer_dimension(target.round().max(1.0))
        }
    }

    fn factor_for_width(&self, natural_width: i64, scaled_width: i64) -> f64 {
        if self.precise || natural_width == 0 {
            self.requested_scale
        } else {
            scaled_width as f64 / natural_width as f64
        }
    }

    // Use integer ratios for adjusted geometry so an integral result such as
    // (7 / 25) * 25 cannot accidentally ceil to 8 due to floating-point noise.
    fn adjusted_dimension(value: i64, width: i64, natural_width: i64) -> (i64, f64) {
        let numerator = value as u128 * width as u128;
        let denominator = natural_width as u128;
        let remainder = numerator % denominator;
        let dimension = i64::try_from(numerator.div_ceil(denominator))
            .expect("scaled dimension exceeds NodeGeometry's range");
        let padding = if remainder == 0 {
            0.0
        } else {
            (denominator - remainder) as f64 / denominator as f64
        };
        (dimension, padding)
    }

    fn layout(&self, child: &NodeGeometry) -> ScaleLayout {
        assert!(
            child.width >= 0 && child.entry_height >= 0 && child.height >= child.entry_height,
            "Scale requires nonnegative child dimensions and an entry within its height"
        );
        let width = self.scaled_width(child.width);
        let factor = self.factor_for_width(child.width, width);
        let (entry_height, vertical_offset, height) = if self.precise {
            (
                Self::integer_dimension((factor * child.entry_height as f64).ceil()),
                0.0,
                Self::integer_dimension((factor * child.height as f64).ceil()),
            )
        } else {
            let (entry, padding, below) = if child.width == 0 {
                let scaled_entry = factor * child.entry_height as f64;
                let entry = Self::integer_dimension(scaled_entry.ceil());
                (
                    entry,
                    entry as f64 - scaled_entry,
                    Self::integer_dimension((factor * child.height_below_entry() as f64).ceil()),
                )
            } else {
                let (entry, padding) =
                    Self::adjusted_dimension(child.entry_height, width, child.width);
                let (below, _) =
                    Self::adjusted_dimension(child.height_below_entry(), width, child.width);
                (entry, padding, below)
            };
            (
                entry,
                padding,
                entry
                    .checked_add(below)
                    .expect("scaled height exceeds NodeGeometry's range"),
            )
        };
        ScaleLayout {
            width,
            height,
            entry_height,
            factor,
            vertical_offset,
        }
    }

    fn emit<B: RenderBackend>(
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
        let child = &geo.children[0];
        let layout = self.layout(child);
        // Keep the integer placement separate from the fractional correction.
        let transform = format!(
            "translate({x} {y}) matrix({s} 0 0 {s} 0 {dy})",
            s = layout.factor,
            dy = layout.vertical_offset,
        );
        backend.push_transformed_child(&self.inner, &transform, h_dir, child)
    }
}

impl<N: Node> Scale<N> {
    /// Return the uniform factor used to render the child.
    ///
    /// For a zero factor, [`Scale::new_precise`], and zero-width children, this is
    /// the requested factor. Otherwise it is derived from the nearest positive
    /// integral width.
    #[must_use]
    pub fn effective_scale(&self) -> f64 {
        if self.requested_scale == 0.0 {
            return self.requested_scale;
        }
        let width = self.inner.width();
        self.factor_for_width(width, self.scaled_width(width))
    }
}

impl<N: Node> Node for Scale<N> {
    fn entry_height(&self) -> i64 {
        self.compute_geometry().entry_height
    }

    fn height(&self) -> i64 {
        self.compute_geometry().height
    }

    fn width(&self) -> i64 {
        if self.requested_scale == 0.0 {
            return 0;
        }
        self.scaled_width(self.inner.width())
    }

    fn compute_geometry(&self) -> NodeGeometry {
        if self.requested_scale == 0.0 {
            return Empty.compute_geometry();
        }
        let child = self.inner.compute_geometry();
        let layout = self.layout(&child);
        NodeGeometry {
            width: layout.width,
            height: layout.height,
            entry_height: layout.entry_height,
            children: vec![child],
        }
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        self.draw_with_geometry(x, y, h_dir, &self.compute_geometry())
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        if self.requested_scale == 0.0 {
            return svg::Element::new("g");
        }
        draw_group_with_geometry(&self.attributes, "Scale", x, y, geo, |backend| {
            self.emit(backend, x, y, h_dir, geo)
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
        if self.requested_scale == 0.0 {
            return Ok(());
        }
        render_group_with_geometry(out, &self.attributes, "Scale", x, y, geo, |backend| {
            self.emit(backend, x, y, h_dir, geo)
        })
    }
}

/// Horizontal placement inside an [`Alignment`], relative to the reading direction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    /// Place the child on the left in LTR and on the right in RTL.
    #[default]
    Start,
    /// Center the child, leaving any odd spare pixel on the physical right.
    Centered,
    /// Place the child on the right in LTR and on the left in RTL.
    End,
}

/// Vertical placement inside an [`Alignment`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    /// Place the child at the top.
    #[default]
    Top,
    /// Center the child, leaving any odd spare pixel at the bottom.
    Centered,
    /// Place the child at the bottom.
    Bottom,
}

/// Reserve minimum dimensions and align a child within them without resizing it.
///
/// Zero, negative values, and values below the child's natural size do not enlarge
/// that dimension. Horizontal start/end placement follows [`svg::HDir`]. Vertical
/// placement shifts the child's entry height by the same amount as its drawing;
/// aligning bounding boxes does not necessarily align their connecting paths.
///
/// # Example
/// ```rust
/// use railroad::*;
///
/// let labels = ["expr", "statement", "item"].map(|s| Comment::new(s.to_owned()));
/// let column_width = labels.iter().map(Node::width).max().unwrap_or(0);
/// let rows = labels.into_iter().map(|label| {
///     HorizontalGrid::<Box<dyn Node>>::new(vec![
///         Box::new(Alignment::new(
///             label, column_width, 0,
///             HorizontalAlignment::Start, VerticalAlignment::Top, false,
///         )),
///         Box::new(NonTerminal::new("body".to_owned())),
///     ])
/// });
/// let diagram = Diagram::new(rows.collect::<VerticalGrid<_>>());
/// assert!(diagram.to_string().contains("statement"));
/// ```
#[derive(Debug, Clone)]
pub struct Alignment<N> {
    inner: N,
    min_width: i64,
    min_height: i64,
    horizontal: HorizontalAlignment,
    vertical: VerticalAlignment,
    connect_rails: bool,
    attributes: HashMap<String, String>,
}

impl<N> Alignment<N> {
    /// Wrap `inner` with the given minimum dimensions and alignment.
    ///
    /// Zero and negative values have no effect on the child's natural size.
    ///
    /// If `connect_rails` is `true`, draw connecting paths across the horizontal
    /// padding at the child's entry height. Otherwise, leave the padding blank.
    #[must_use]
    pub fn new(
        inner: N,
        min_width: i64,
        min_height: i64,
        horizontal: HorizontalAlignment,
        vertical: VerticalAlignment,
        connect_rails: bool,
    ) -> Self {
        Self {
            inner,
            min_width,
            min_height,
            horizontal,
            vertical,
            connect_rails,
            attributes: HashMap::from([("class".to_owned(), "alignment".to_owned())]),
        }
    }

    /// Return the wrapped child.
    #[must_use]
    pub fn into_inner(self) -> N {
        self.inner
    }

    /// Return the entry for `key` in the outer `<g>` element's attributes.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
    }

    fn vertical_offset(&self, extra_height: i64) -> i64 {
        match self.vertical {
            VerticalAlignment::Top => 0,
            VerticalAlignment::Centered => extra_height / 2,
            VerticalAlignment::Bottom => extra_height,
        }
    }

    fn emit<B: RenderBackend>(
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
        let child_geo = &geo.children[0];
        let extra_width = geo.width - child_geo.width;
        let dx = match (self.horizontal, h_dir) {
            (HorizontalAlignment::Start, HDir::LTR) | (HorizontalAlignment::End, HDir::RTL) => 0,
            (HorizontalAlignment::Centered, _) => extra_width / 2,
            (HorizontalAlignment::End, HDir::LTR) | (HorizontalAlignment::Start, HDir::RTL) => {
                extra_width
            }
        };
        let dy = geo.entry_height - child_geo.entry_height;
        if self.connect_rails {
            for (offset, length) in [(0, dx), (dx + child_geo.width, extra_width - dx)] {
                if length > 0 {
                    let path = svg::PathData::new(h_dir).move_to(x + offset, y + geo.entry_height);
                    // A zero-height child still supports a rail, but not arrowheads
                    // that protrude above or below the advertised rectangle.
                    let path = if geo.entry_height >= svg::PathData::PADDING
                        && geo.height_below_entry() >= svg::PathData::PADDING
                    {
                        path.horizontal(length)
                    } else {
                        path.line_rel(length, 0)
                    };
                    backend.push_path(path)?;
                }
            }
        }
        backend.push_child(&self.inner, x + dx, y + dy, h_dir, child_geo)
    }
}

impl<N: Node> Node for Alignment<N> {
    fn entry_height(&self) -> i64 {
        let child_height = self.inner.height();
        let height = child_height.max(self.min_height);
        self.inner.entry_height() + self.vertical_offset(height - child_height)
    }

    fn height(&self) -> i64 {
        self.inner.height().max(self.min_height)
    }

    fn width(&self) -> i64 {
        self.inner.width().max(self.min_width)
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        self.draw_with_geometry(x, y, h_dir, &self.compute_geometry())
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let child_geo = self.inner.compute_geometry();
        let height = child_geo.height.max(self.min_height);
        NodeGeometry {
            entry_height: child_geo.entry_height + self.vertical_offset(height - child_geo.height),
            height,
            width: child_geo.width.max(self.min_width),
            children: vec![child_geo],
        }
    }

    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Alignment", x, y, geo, |backend| {
            self.emit(backend, x, y, h_dir, geo)
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
        render_group_with_geometry(out, &self.attributes, "Alignment", x, y, geo, |backend| {
            self.emit(backend, x, y, h_dir, geo)
        })
    }
}

/// Browsing contexts for [`Link`].
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

/// A node wrapped in a clickable link to a URI.
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
    /// The URI is XML-escaped when written to the SVG anchor attribute.
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

    /// Return the entry for `key` in the outer `<a>` element's attributes.
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

/// A node with a bypass path above it, allowing it to be skipped.
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

    /// Return the entry for `key` in the outer `<g>` element's attributes.
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
        svg::PathData::PADDING + ARC_RADIUS + cmp::max(ARC_RADIUS, self.inner.entry_height())
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
        let entry_height =
            svg::PathData::PADDING + ARC_RADIUS + cmp::max(ARC_RADIUS, inner_geo.entry_height);
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

/// A node with a return path below it, allowing one or more occurrences.
///
/// The main path passes through `inner` in the reading direction. The return
/// path passes through `repeat` in the opposite direction before returning to
/// `inner`. Use [`Empty`] for `repeat` when no separator is needed, or wrap the
/// repetition in [`Optional`] to allow zero occurrences.
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
    /// // One or more items with no separator.
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

    /// Return the entry for `key` in the outer `<g>` element's attributes.
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
        let padding = svg::PathData::PADDING;
        let inner_geo = &geo.children[0];
        let repeat_geo = &geo.children[1];
        let height_between = cmp::max(
            ARC_RADIUS * 2,
            inner_geo.height_below_entry() + self.spacing + repeat_geo.entry_height,
        );

        backend.push_path(
            svg::PathData::new(h_dir)
                .move_to(x, y + geo.entry_height)
                .horizontal(padding)
                .horizontal(ARC_RADIUS)
                .move_rel(inner_geo.width, 0)
                .horizontal(cmp::max(
                    ARC_RADIUS,
                    repeat_geo.width - inner_geo.width + ARC_RADIUS,
                ))
                .horizontal(padding)
                .move_rel(-ARC_RADIUS - padding, 0)
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
            x + geo.width - repeat_geo.width - ARC_RADIUS - padding,
            y + geo.height - repeat_geo.height_below_entry() - repeat_geo.entry_height - padding,
            h_dir.invert(),
            repeat_geo,
        )?;
        backend.push_child(
            &self.inner,
            x + ARC_RADIUS + padding,
            y + padding,
            h_dir,
            inner_geo,
        )
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
        svg::PathData::PADDING + self.inner.entry_height()
    }

    fn height(&self) -> i64 {
        self.entry_height()
            + self.height_between_entries()
            + self.repeat.height_below_entry()
            + svg::PathData::PADDING
    }

    fn width(&self) -> i64 {
        svg::PathData::PADDING * 2
            + ARC_RADIUS
            + cmp::max(self.repeat.width(), self.inner.width())
            + ARC_RADIUS
    }

    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        let mut g = svg::Element::new("g").set_all(self.attributes.iter());

        g.push(
            svg::PathData::new(h_dir)
                .move_to(x, y + self.entry_height())
                .horizontal(svg::PathData::PADDING)
                .horizontal(ARC_RADIUS)
                .move_rel(self.inner.width(), 0)
                .horizontal(cmp::max(
                    ARC_RADIUS,
                    self.repeat.width() - self.inner.width() + ARC_RADIUS,
                ))
                .horizontal(svg::PathData::PADDING)
                .move_rel(-ARC_RADIUS - svg::PathData::PADDING, 0)
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
            x + self.width() - self.repeat.width() - ARC_RADIUS - svg::PathData::PADDING,
            y + self.height()
                - self.repeat.height_below_entry()
                - self.repeat.entry_height()
                - svg::PathData::PADDING,
            h_dir.invert(),
        ));
        g.push(self.inner.draw(
            x + ARC_RADIUS + svg::PathData::PADDING,
            y + svg::PathData::PADDING,
            h_dir,
        ));
        g.debug("Repeat", x, y, self)
    }

    fn compute_geometry(&self) -> NodeGeometry {
        let inner_geo = self.inner.compute_geometry();
        let repeat_geo = self.repeat.compute_geometry();
        let height_between = cmp::max(
            ARC_RADIUS * 2,
            inner_geo.height_below_entry() + self.spacing + repeat_geo.entry_height,
        );
        let entry_height = svg::PathData::PADDING + inner_geo.entry_height;
        let height = entry_height
            + height_between
            + repeat_geo.height_below_entry()
            + svg::PathData::PADDING;
        let width = svg::PathData::PADDING * 2
            + ARC_RADIUS
            + cmp::max(repeat_geo.width, inner_geo.width)
            + ARC_RADIUS;
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

/// A box around a node, with a label above the node inside the box.
///
/// Use [`crate::Comment`] for a text label or [`Empty`] for no label.
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
    /// Equivalent to `LabeledBox::new(inner, Empty)`.
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

    /// Return the entry for `key` in the outer `<g>` element's attributes.
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
