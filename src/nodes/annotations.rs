use std::{
    collections::{self, HashMap},
    fmt,
};

use crate::{
    HDir, LabeledBox, Node, NodeGeometry, RenderBackend, draw_group_with_geometry,
    render_group_with_geometry, svg,
};

const HALF_WIDTH: i64 = 24;
const HALF_HEIGHT: i64 = 16;
const BODY_TOP: i64 = HALF_HEIGHT * 2 + 16;

#[derive(Debug, Clone, Copy)]
enum Direction {
    None,
    Ahead,
    Behind,
}

/// A checkpoint connected to a detached, centered [`LabeledBox`].
///
/// The label and body are entirely caller-supplied: this node imposes no grammar
/// semantics.
///
/// ```rust
/// use railroad::*;
/// let node = Annotation::new(LabeledBox::new(
///     NonTerminal::new("statement".to_owned()),
///     Comment::new("Only at top level".to_owned()),
/// ));
/// assert!(Diagram::new(node).to_string().contains("Only at top level"));
/// ```
///
/// Negative lookahead is one possible recipe:
///
/// ```rust
/// use railroad::*;
/// let node = Sequence::<Box<dyn Node>>::new(vec![
///     Box::new(Annotation::new_ahead(LabeledBox::new(
///         Terminal::new("\"".to_owned()),
///         Comment::new("Must not match ahead; consumes no input".to_owned()),
///     ))),
///     Box::new(NonTerminal::new("ASCII".to_owned())),
/// ]);
/// assert!(Diagram::new(node).to_string().contains("Must not match ahead"));
/// ```
#[derive(Debug, Clone)]
pub struct Annotation<N, L> {
    inner: LabeledBox<N, L>,
    direction: Direction,
    attributes: HashMap<String, String>,
}

impl<N, L> Annotation<N, L> {
    /// Draw a plain `!` checkpoint with the supplied box beneath it.
    #[must_use]
    pub fn new(inner: LabeledBox<N, L>) -> Self {
        Self::with_direction(inner, Direction::None)
    }

    /// Draw `↝`, referring ahead in the local reading direction.
    ///
    /// In [`svg::HDir::RTL`], the marker is mirrored to `↜`.
    #[must_use]
    pub fn new_ahead(inner: LabeledBox<N, L>) -> Self {
        Self::with_direction(inner, Direction::Ahead)
    }

    /// Draw `↜`, referring behind, opposite to the local reading direction.
    ///
    /// In [`svg::HDir::RTL`], the marker is mirrored to `↝`. The box's own
    /// sequence still reads in the local reading direction; its children are not reversed.
    #[must_use]
    pub fn new_behind(inner: LabeledBox<N, L>) -> Self {
        Self::with_direction(inner, Direction::Behind)
    }

    fn with_direction(inner: LabeledBox<N, L>, direction: Direction) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert("class".to_owned(), "annotation".to_owned());
        Self {
            inner,
            direction,
            attributes,
        }
    }

    /// Return the supplied box, including any custom label and attributes.
    pub fn into_inner(self) -> LabeledBox<N, L> {
        self.inner
    }

    /// Access an attribute on the outer SVG group.
    pub fn attr(&mut self, key: String) -> collections::hash_map::Entry<'_, String, String> {
        self.attributes.entry(key)
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
        L: Node,
    {
        let cx = x + geo.width / 2;
        let rail_y = y + geo.entry_height;
        let body_geo = &geo.children[0];
        backend.push_path_with_class(
            svg::PathData::new(h_dir)
                .move_to(x, rail_y)
                .horizontal(geo.width / 2 - HALF_WIDTH)
                .move_to(cx + HALF_WIDTH, rail_y)
                .horizontal(geo.width - geo.width / 2 - HALF_WIDTH),
            "annotation-rail",
        )?;
        backend.push_path_with_class(
            svg::PathData::new(h_dir)
                .move_to(cx, y)
                .line_rel(HALF_WIDTH, HALF_HEIGHT)
                .line_rel(-HALF_WIDTH, HALF_HEIGHT)
                .line_rel(-HALF_WIDTH, -HALF_HEIGHT)
                .close(),
            "annotation-marker",
        )?;
        backend.push_path_with_class(
            svg::PathData::new(h_dir)
                .move_to(cx, y + HALF_HEIGHT * 2)
                .line_rel(0, BODY_TOP - HALF_HEIGHT * 2),
            "annotation-connector",
        )?;

        let direction = match self.direction {
            Direction::None => None,
            Direction::Ahead => Some(h_dir),
            Direction::Behind => Some(h_dir.invert()),
        };
        if let Some(dir) = direction {
            let sign = if dir == HDir::LTR { 1 } else { -1 };
            // Two smooth lobes reproduce ↝ / ↜ without depending on font coverage.
            backend.push_path_with_class(
                svg::PathData::new(dir)
                    .move_to(cx - sign * 8, rail_y)
                    .cubic_rel(sign * 2, -4, sign * 5, -4, sign * 7, 0)
                    .cubic_rel(sign * 2, 4, sign * 4, 0, sign * 6, 0)
                    .line_rel(sign * 3, 0)
                    // Keep both wings joined at the tip, separate from the shaft.
                    .move_rel(-sign * 4, -4)
                    .line_rel(sign * 4, 4)
                    .line_rel(-sign * 4, 4),
                "annotation-direction",
            )?;
        } else {
            backend.push_text_with_class(cx, rail_y + 5, "!", "annotation-symbol")?;
        }
        backend.push_child(
            &self.inner,
            x + (geo.width - body_geo.width) / 2,
            y + BODY_TOP,
            h_dir,
            body_geo,
        )
    }
}

impl<N: Node, L: Node> Node for Annotation<N, L> {
    fn entry_height(&self) -> i64 {
        HALF_HEIGHT
    }
    fn height(&self) -> i64 {
        BODY_TOP + self.inner.height()
    }
    fn width(&self) -> i64 {
        self.inner.width().max(HALF_WIDTH * 2 + 16)
    }
    fn compute_geometry(&self) -> NodeGeometry {
        let inner = self.inner.compute_geometry();
        NodeGeometry {
            entry_height: HALF_HEIGHT,
            height: BODY_TOP + inner.height,
            width: inner.width.max(HALF_WIDTH * 2 + 16),
            children: vec![inner],
        }
    }
    fn draw(&self, x: i64, y: i64, h_dir: HDir) -> svg::Element {
        self.draw_with_geometry(x, y, h_dir, &self.compute_geometry())
    }
    fn draw_with_geometry(&self, x: i64, y: i64, h_dir: HDir, geo: &NodeGeometry) -> svg::Element {
        draw_group_with_geometry(&self.attributes, "Annotation", x, y, geo, |backend| {
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
        render_group_with_geometry(out, &self.attributes, "Annotation", x, y, geo, |backend| {
            self.emit(backend, x, y, h_dir, geo)
        })
    }
}
