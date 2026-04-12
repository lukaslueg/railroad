use std::{
    collections::{self, HashMap},
    fmt,
};

use crate::{
    HDir, Node, NodeGeometry, RenderBackend, emit_text_box, render_group_with_geometry, svg,
    text_width,
};

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

/// A label / verbatim text drawn inline on the connecting path.
///
/// Useful as a label for [`crate::LabeledBox`] or as a lightweight annotation
/// within a [`crate::Sequence`].
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
        crate::add_debug_attrs(&mut text, "Comment", x, y, geo)?;
        text.finish()?;
        out.write_text(&self.text)?;
        out.end_element("text")?;
        crate::write_debug_overlay(out, x, y, geo)
    }
}
