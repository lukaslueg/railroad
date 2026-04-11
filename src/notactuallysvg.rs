use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Write},
};

/// A shorthand to draw rounded corners, see [`PathData::arc`].
///
/// Each variant names the direction the path is traveling *before* the corner
/// (the compass point it comes from) and the direction *after* the corner
/// (the compass point it heads toward).
#[derive(Debug, Clone, Copy)]
pub enum Arc {
    /// Traveling east, turn to go north (curve up-left).
    EastToNorth,
    /// Traveling east, turn to go south (curve down-left).
    EastToSouth,
    /// Traveling north, turn to go east (curve right-down).
    NorthToEast,
    /// Traveling north, turn to go west (curve left-down).
    NorthToWest,
    /// Traveling south, turn to go east (curve right-up).
    SouthToEast,
    /// Traveling south, turn to go west (curve left-up).
    SouthToWest,
    /// Traveling west, turn to go north (curve up-right).
    WestToNorth,
    /// Traveling west, turn to go south (curve down-right).
    WestToSouth,
}

/// Selects the direction in which arrows on positive-direction horizontal
/// lines point.
///
/// `LTR` (left-to-right) is the default and suits most diagrams. `RTL` is used
/// for the return arc inside [`crate::Repeat`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum HDir {
    /// Arrows point rightward (the default).
    #[default]
    LTR,
    /// Arrows point leftward.
    RTL,
}

impl HDir {
    /// Invert the direction.
    #[must_use]
    pub fn invert(self) -> Self {
        match self {
            HDir::LTR => HDir::RTL,
            HDir::RTL => HDir::LTR,
        }
    }
}

/// A lightweight streaming SVG writer used by the render path.
///
/// `Renderer` writes directly into a [`fmt::Write`] sink and centralizes
/// element/tag emission, text escaping, and path serialization.
///
/// # Example
/// ```rust
/// use std::fmt;
/// use railroad::notactuallysvg as svg;
///
/// fn build(out: &mut dyn fmt::Write) -> fmt::Result {
///     let mut renderer = svg::Renderer::new(out);
///     let mut g = renderer.start_element("g")?;
///     g.attr("class", "demo")?;
///     g.finish()?;
///     renderer.text_element("text", "hello <world>", |t| {
///         t.attr("x", 10)?;
///         t.attr("y", 20)
///     })?;
///     renderer.end_element("g")
/// }
///
/// let mut out = String::new();
/// build(&mut out).unwrap();
/// assert!(out.contains("<g class=\"demo\">"));
/// assert!(out.contains("hello &lt;world&gt;"));
/// ```
pub struct Renderer<'a> {
    out: &'a mut dyn fmt::Write,
}

/// A builder for an element's opening tag.
///
/// Instances are created by [`Renderer::start_element`] and allow callers to
/// append attributes before completing the tag with [`StartTag::finish`] or
/// [`StartTag::finish_empty`].
pub struct StartTag<'a, 'b> {
    renderer: &'a mut Renderer<'b>,
}

struct EscapingWriter<'a> {
    out: &'a mut dyn fmt::Write,
}

impl<'a> Renderer<'a> {
    /// Create a renderer that writes SVG markup into `out`.
    pub fn new(out: &'a mut dyn fmt::Write) -> Self {
        Self { out }
    }

    /// Start an element opening tag.
    ///
    /// Returns [`fmt::Error`] if `name` is not a valid XML tag name according to
    /// this renderer's conservative validation rules.
    ///
    /// # Example
    /// ```rust
    /// use std::fmt;
    /// use railroad::notactuallysvg as svg;
    ///
    /// fn build(out: &mut dyn fmt::Write) -> fmt::Result {
    ///     let mut renderer = svg::Renderer::new(out);
    ///     let mut circle = renderer.start_element("circle")?;
    ///     circle.attr("r", 5)?;
    ///     circle.finish_empty()
    /// }
    ///
    /// let mut out = String::new();
    /// build(&mut out).unwrap();
    /// assert_eq!(out, "<circle r=\"5\"/>\n");
    /// ```
    pub fn start_element<'b>(&'b mut self, name: &str) -> Result<StartTag<'b, 'a>, fmt::Error> {
        validate_tag_name(name)?;
        self.out.write_char('<')?;
        self.out.write_str(name)?;
        Ok(StartTag { renderer: self })
    }

    /// Write a closing tag for `name`.
    ///
    /// Returns [`fmt::Error`] if `name` does not pass tag validation.
    pub fn end_element(&mut self, name: &str) -> fmt::Result {
        validate_tag_name(name)?;
        self.out.write_str("</")?;
        self.out.write_str(name)?;
        self.out.write_str(">\n")
    }

    /// Write text content with minimal XML escaping.
    pub fn write_text(&mut self, text: &str) -> fmt::Result {
        let mut escaping = EscapingWriter { out: self.out };
        escaping.write_str(text)
    }

    /// Write raw text without any escaping.
    ///
    /// Callers should only use this for trusted markup or CSS.
    pub fn write_raw(&mut self, text: &str) -> fmt::Result {
        self.out.write_str(text)
    }

    /// Write any [`fmt::Display`] value directly into the output stream.
    pub fn write_display(&mut self, display: impl fmt::Display) -> fmt::Result {
        write!(self.out, "{display}")
    }

    /// Write a `<path>` element whose `d` attribute comes from `path`.
    pub fn path(&mut self, path: &PathData) -> fmt::Result {
        let mut tag = self.start_element("path")?;
        tag.attr("d", path)?;
        tag.finish_empty()
    }

    /// Write a `<path>` element with an additional `class` attribute.
    pub fn path_with_class(&mut self, path: &PathData, class: &str) -> fmt::Result {
        let mut tag = self.start_element("path")?;
        tag.attr("d", path)?;
        tag.attr("class", class)?;
        tag.finish_empty()
    }

    /// Write a text-bearing element whose body is escaped.
    ///
    /// The `configure` callback may add attributes to the opening tag before the
    /// element is closed.
    ///
    /// # Example
    /// ```rust
    /// use std::fmt;
    /// use railroad::notactuallysvg as svg;
    ///
    /// fn build(out: &mut dyn fmt::Write) -> fmt::Result {
    ///     let mut renderer = svg::Renderer::new(out);
    ///     renderer.text_element("text", "a < b", |tag| {
    ///         tag.attr("x", 5)?;
    ///         tag.attr("y", 10)
    ///     })
    /// }
    ///
    /// let mut out = String::new();
    /// build(&mut out).unwrap();
    /// assert_eq!(out, "<text x=\"5\" y=\"10\">\na &lt; b</text>\n");
    /// ```
    pub fn text_element(
        &mut self,
        name: &str,
        text: &str,
        configure: impl FnOnce(&mut StartTag<'_, 'a>) -> fmt::Result,
    ) -> fmt::Result {
        let mut tag = self.start_element(name)?;
        configure(&mut tag)?;
        tag.finish()?;
        self.write_text(text)?;
        self.end_element(name)
    }

    /// Write a text-bearing element whose body is not escaped.
    ///
    /// This is intended for trusted raw SVG or CSS content.
    ///
    /// # Example
    /// ```rust
    /// use std::fmt;
    /// use railroad::notactuallysvg as svg;
    ///
    /// fn build(out: &mut dyn fmt::Write) -> fmt::Result {
    ///     let mut renderer = svg::Renderer::new(out);
    ///     renderer.raw_text_element("style", "text { fill: red < blue; }", |tag| {
    ///         tag.attr("type", "text/css")
    ///     })
    /// }
    ///
    /// let mut out = String::new();
    /// build(&mut out).unwrap();
    /// assert_eq!(out, "<style type=\"text/css\">\ntext { fill: red < blue; }</style>\n");
    /// ```
    pub fn raw_text_element(
        &mut self,
        name: &str,
        text: &str,
        configure: impl FnOnce(&mut StartTag<'_, 'a>) -> fmt::Result,
    ) -> fmt::Result {
        let mut tag = self.start_element(name)?;
        configure(&mut tag)?;
        tag.finish()?;
        self.write_raw(text)?;
        self.end_element(name)
    }
}

impl StartTag<'_, '_> {
    /// Add a single attribute to the opening tag.
    ///
    /// Both key and value are minimally XML-escaped before being written.
    pub fn attr(&mut self, key: impl fmt::Display, value: impl fmt::Display) -> fmt::Result {
        self.renderer.out.write_char(' ')?;
        {
            let mut escaping = EscapingWriter {
                out: self.renderer.out,
            };
            write!(&mut escaping, "{key}")?;
        }
        self.renderer.out.write_str("=\"")?;
        {
            let mut escaping = EscapingWriter {
                out: self.renderer.out,
            };
            write!(&mut escaping, "{value}")?;
        }
        self.renderer.out.write_char('"')
    }

    /// Add all attributes from a map in deterministic key order.
    pub fn attr_hashmap(&mut self, attrs: &HashMap<String, String>) -> fmt::Result {
        let mut attrs = attrs.iter().collect::<Vec<_>>();
        attrs.sort_by_key(|(k, _)| *k);
        for (key, value) in attrs {
            self.attr(key, value)?;
        }
        Ok(())
    }

    /// Finish the opening tag as a non-empty element.
    pub fn finish(self) -> fmt::Result {
        self.renderer.out.write_str(">\n")
    }

    /// Finish the opening tag as an empty element.
    ///
    /// # Example
    /// ```rust
    /// use std::fmt;
    /// use railroad::notactuallysvg as svg;
    ///
    /// fn build(out: &mut dyn fmt::Write) -> fmt::Result {
    ///     let mut renderer = svg::Renderer::new(out);
    ///     let mut rect = renderer.start_element("rect")?;
    ///     rect.attr("width", "100%")?;
    ///     rect.attr("height", "100%")?;
    ///     rect.finish_empty()
    /// }
    ///
    /// let mut out = String::new();
    /// build(&mut out).unwrap();
    /// assert_eq!(out, "<rect width=\"100%\" height=\"100%\"/>\n");
    /// ```
    pub fn finish_empty(self) -> fmt::Result {
        self.renderer.out.write_str("/>\n")
    }
}

impl fmt::Write for EscapingWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_escaped_minimal(self.out, s)
    }
}

fn validate_tag_name(name: &str) -> fmt::Result {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(fmt::Error);
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == ':') {
        return Err(fmt::Error);
    }
    if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')) {
        Ok(())
    } else {
        Err(fmt::Error)
    }
}

/// A builder for SVG path `d` attribute strings.
///
/// All drawing methods take `self` by value and return `self`, enabling
/// method chaining. Call [`PathData::into_path`] when done to obtain a
/// `<path>` [`Element`].
///
/// # Example
/// ```
/// use railroad::notactuallysvg as svg;
///
/// let path = svg::PathData::new(svg::HDir::LTR)
///     .move_to(0, 10)
///     .horizontal(40)
///     .into_path();
/// assert!(path.to_string().contains("M 0 10"));
/// ```
pub struct PathData {
    text: String,
    h_dir: HDir,
}

impl PathData {
    /// Construct an empty `PathData` with the given horizontal direction.
    #[must_use]
    pub fn new(h_dir: HDir) -> Self {
        Self {
            text: String::new(),
            h_dir,
        }
    }

    /// Consume this builder and return a `<path>` [`Element`] whose `d` attribute
    /// contains the accumulated path data.
    #[must_use]
    pub fn into_path(self) -> Element {
        Element::new("path").set("d", &self.text)
    }

    /// Move the cursor to the absolute position `(x, y)` without drawing.
    #[must_use]
    pub fn move_to(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " M {x} {y}").unwrap();
        self
    }

    /// Move the cursor by `(x, y)` relative to the current position without drawing.
    #[must_use]
    pub fn move_rel(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " m {x} {y}").unwrap();
        self
    }

    /// Draw a line from the cursor's current position to the relative offset `(x, y)`.
    #[must_use]
    pub fn line_rel(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " l {x} {y}").unwrap();
        self
    }

    /// Draw a horizontal segment of length `h` from the cursor's current position.
    ///
    /// For segments longer than 50 pixels an arrowhead is automatically added at
    /// the midpoint, pointing in the diagram's [`HDir`] direction.
    #[must_use]
    pub fn horizontal(mut self, h: i64) -> Self {
        write!(self.text, " h {h}").unwrap();
        // Add an arrow for long stretches
        match (h > 50, h < -50, self.h_dir) {
            (true, _, HDir::LTR) => self
                .move_rel(-(h / 2 - 3), 0)
                .line_rel(-5, -5)
                .move_rel(0, 10)
                .line_rel(5, -5)
                .move_rel(h / 2 - 3, 0),
            (true, _, HDir::RTL) => self
                .move_rel(-(h / 2 + 3), 0)
                .line_rel(5, -5)
                .move_rel(0, 10)
                .line_rel(-5, -5)
                .move_rel(h / 2 + 3, 0),
            (_, true, HDir::LTR) => self
                .move_rel(-(h / 2 - 3), 0)
                .line_rel(5, -5)
                .move_rel(0, 10)
                .line_rel(-5, -5)
                .move_rel(h / 2 - 3, 0),
            (_, true, HDir::RTL) => self
                .move_rel(-(h / 2 + 3), 0)
                .line_rel(-5, -5)
                .move_rel(0, 10)
                .line_rel(5, -5)
                .move_rel(h / 2 + 3, 0),
            (false, false, _) => self,
        }
    }

    /// Draw a vertical segment of height `h` from the cursor's current position.
    ///
    /// For segments taller than 50 pixels a downward arrowhead is automatically
    /// added at the midpoint.
    #[must_use]
    pub fn vertical(mut self, h: i64) -> Self {
        write!(self.text, " v {h}").unwrap();
        // Add an arrow for long stretches
        if h > 50 {
            self.move_rel(0, -(h / 2 - 3))
                .line_rel(-5, -5)
                .move_rel(10, 0)
                .line_rel(-5, 5)
                .move_rel(0, h / 2 - 3)
        } else if h < -50 {
            self.move_rel(0, -(h / 2 - 3))
                .line_rel(-5, 5)
                .move_rel(10, 0)
                .line_rel(-5, -5)
                .move_rel(0, h / 2 - 3)
        } else {
            self
        }
    }

    /// Draw a quarter-circle arc of the given `radius` in the given direction.
    ///
    /// See [`Arc`] for the available corner directions.
    #[must_use]
    pub fn arc(mut self, radius: i64, kind: Arc) -> Self {
        let (sweep, x, y) = match kind {
            Arc::EastToNorth => (1, -radius, -radius),
            Arc::EastToSouth => (0, -radius, radius),
            Arc::NorthToEast => (0, radius, radius),
            Arc::NorthToWest => (1, -radius, radius),
            Arc::SouthToEast => (1, radius, -radius),
            Arc::SouthToWest => (0, -radius, -radius),
            Arc::WestToNorth => (0, radius, -radius),
            Arc::WestToSouth => (1, radius, radius),
        };
        write!(self.text, " a {radius} {radius} 0 0 {sweep} {x} {y}").unwrap();
        self
    }
}

impl fmt::Display for PathData {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
        write!(f, "{}", self.text)
    }
}

/// A pseudo-SVG Element
///
/// ```
/// use railroad::notactuallysvg as svg;
///
/// let e = svg::Element::new("g")
///         .add(svg::Element::new("rect")
///                 .set("class", "important")
///                 .set("x", &15))
///         .add(svg::PathData::new(svg::HDir::LTR)
///                  .move_to(5, 5)
///                  .line_rel(10, 20)
///                  .into_path());
/// let serialized = e.to_string();
/// assert_eq!(serialized, "<g>\n<rect class=\"important\" x=\"15\"/>\n<path d=\" M 5 5 l 10 20\"/>\n</g>\n");
/// ```
#[derive(Debug, Clone)]
pub struct Element {
    name: String,
    attributes: HashMap<String, String>,
    text: Option<String>,
    children: Vec<Element>,
    siblings: Vec<Element>,
}

impl Element {
    /// Construct a new `Element` of type `name`.
    pub fn new<T>(name: &T) -> Self
    where
        T: ToString + ?Sized,
    {
        Self {
            name: name.to_string(),
            attributes: HashMap::default(),
            text: None,
            children: Vec::default(),
            siblings: Vec::default(),
        }
    }

    /// Set this Element's attribute `key` to `value`
    #[must_use]
    pub fn set<K, V>(mut self, key: &K, value: &V) -> Self
    where
        K: ToString + ?Sized,
        V: ToString + ?Sized,
    {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// Set all attributes via these `key`:`value`-pairs
    #[must_use]
    pub fn set_all(
        mut self,
        iter: impl IntoIterator<Item = (impl ToString, impl ToString)>,
    ) -> Self {
        self.attributes.extend(
            iter.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        self
    }

    /// Set the text within the opening and closing tag of this Element.
    ///
    /// The text is automatically HTML-escaped. It is written before any children.
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(encode_minimal(text).into_owned());
        self
    }

    /// Set the text within the opening and closing tag of this Element.
    ///
    /// The text is NOT automatically HTML-escaped.
    #[must_use]
    pub fn raw_text<T>(mut self, text: &T) -> Self
    where
        T: ToString + ?Sized,
    {
        self.text = Some(text.to_string());
        self
    }

    /// Add a child to this Element
    ///
    /// Children is written within the opening and closing tag of this Element.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn add(mut self, e: Self) -> Self {
        self.children.push(e);
        self
    }

    /// Add a child to this Element
    ///
    /// Children is written within the opening and closing tag of this Element.
    pub fn push(&mut self, e: Self) -> &mut Self {
        self.children.push(e);
        self
    }

    /// Add a sibling to this Element
    ///
    /// Siblings is written after the closing tag of this Element.
    #[must_use]
    pub fn append(mut self, e: Self) -> Self {
        self.siblings.push(e);
        self
    }

    #[cfg(not(feature = "visual-debug"))]
    #[allow(unused_variables)]
    #[doc(hidden)]
    #[must_use]
    pub fn debug(self, name: &str, x: i64, y: i64, n: &dyn super::Node) -> Self {
        self
    }

    #[cfg(not(feature = "visual-debug"))]
    #[allow(unused_variables)]
    #[doc(hidden)]
    #[must_use]
    pub fn debug_with_geometry(
        self,
        name: &str,
        x: i64,
        y: i64,
        geo: &super::NodeGeometry,
    ) -> Self {
        self
    }

    /// Adds some basic textual and visual debugging information to this Element
    #[cfg(feature = "visual-debug")]
    pub fn debug(self, name: &str, x: i64, y: i64, n: &dyn super::Node) -> Self {
        self.set("railroad:type", &name)
            .set("railroad:x", &x)
            .set("railroad:y", &y)
            .set("railroad:entry_height", &n.entry_height())
            .set("railroad:height", &n.height())
            .set("railroad:width", &n.width())
            .add(Element::new("title").text(name))
            .append(
                Element::new("path")
                    .set(
                        "d",
                        &PathData::new(HDir::LTR)
                            .move_to(x, y)
                            .horizontal(n.width())
                            .vertical(5)
                            .move_rel(-n.width(), -5)
                            .vertical(n.height())
                            .horizontal(5)
                            .move_rel(-5, -n.height())
                            .move_rel(0, n.entry_height())
                            .horizontal(10),
                    )
                    .set("class", "debug"),
            )
    }

    /// Adds debug information using cached geometry instead of querying the node again.
    #[cfg(feature = "visual-debug")]
    pub fn debug_with_geometry(
        self,
        name: &str,
        x: i64,
        y: i64,
        geo: &super::NodeGeometry,
    ) -> Self {
        self.set("railroad:type", &name)
            .set("railroad:x", &x)
            .set("railroad:y", &y)
            .set("railroad:entry_height", &geo.entry_height)
            .set("railroad:height", &geo.height)
            .set("railroad:width", &geo.width)
            .add(Element::new("title").text(name))
            .append(
                Element::new("path")
                    .set(
                        "d",
                        &PathData::new(HDir::LTR)
                            .move_to(x, y)
                            .horizontal(geo.width)
                            .vertical(5)
                            .move_rel(-geo.width, -5)
                            .vertical(geo.height)
                            .horizontal(5)
                            .move_rel(-5, -geo.height)
                            .move_rel(0, geo.entry_height)
                            .horizontal(10),
                    )
                    .set("class", "debug"),
            )
    }
}

impl ::std::fmt::Display for Element {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
        write!(f, "<{}", self.name)?;
        let mut attrs = self.attributes.iter().collect::<Vec<_>>();
        attrs.sort_by_key(|(k, _)| *k);
        for (k, v) in attrs {
            write!(f, " {}=\"{}\"", encode_minimal(k), encode_minimal(v))?;
        }
        if self.text.is_none() && self.children.is_empty() {
            f.write_str("/>\n")?;
        } else {
            f.write_str(">\n")?;
        }
        if let Some(t) = &self.text {
            f.write_str(t)?;
        }
        for child in &self.children {
            write!(f, "{child}")?;
        }

        if self.text.is_some() || !self.children.is_empty() {
            writeln!(f, "</{}>", self.name)?;
        }
        for sibling in &self.siblings {
            write!(f, "{sibling}")?;
        }
        Ok(())
    }
}

fn minimal_entity(c: char) -> Option<&'static str> {
    match c {
        '"' => Some("&quot;"),
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        '\'' => Some("&#x27;"),
        _ => None,
    }
}

fn write_escaped_minimal(f: &mut (impl fmt::Write + ?Sized), inp: &str) -> fmt::Result {
    let mut last_idx = 0;
    for (idx, c) in inp.char_indices() {
        if let Some(entity) = minimal_entity(c) {
            f.write_str(&inp[last_idx..idx])?;
            f.write_str(entity)?;
            last_idx = idx + 1;
        }
    }
    f.write_str(&inp[last_idx..])
}

/// Escape the bare minimum of characters (`"`, `&`, `<`, `>`, `'`) needed to
/// safely embed `inp` as text content or a double-quoted attribute value in SVG/HTML.
///
/// Returns a [`Cow::Borrowed`] slice when no escaping is required (i.e. when
/// `inp` contains none of the five special characters), avoiding an allocation.
///
/// # Example
/// ```
/// use railroad::notactuallysvg::encode_minimal;
///
/// assert_eq!(encode_minimal("hello"), "hello");
/// assert_eq!(encode_minimal("a & b"), "a &amp; b");
/// assert_eq!(encode_minimal("<b>"), "&lt;b&gt;");
/// ```
#[must_use]
pub fn encode_minimal(inp: &str) -> Cow<'_, str> {
    let mut buf = String::new();
    let mut last_idx = 0;
    for (idx, c) in inp.char_indices() {
        if let Some(entity) = minimal_entity(c) {
            buf.push_str(&inp[last_idx..idx]);
            buf.push_str(entity);
            last_idx = idx + 1;
        }
    }
    if buf.is_empty() {
        Cow::Borrowed(inp)
    } else {
        buf.push_str(&inp[last_idx..]);
        Cow::Owned(buf)
    }
}

const ENTITIES: [Option<&'static str>; 256] = [
    Some("&#x00;"),
    Some("&#x01;"),
    Some("&#x02;"),
    Some("&#x03;"),
    Some("&#x04;"),
    Some("&#x05;"),
    Some("&#x06;"),
    Some("&#x07;"),
    Some("&#x08;"),
    Some("&#x09;"),
    Some("&#x0A;"),
    Some("&#x0B;"),
    Some("&#x0C;"),
    Some("&#x0D;"),
    Some("&#x0E;"),
    Some("&#x0F;"),
    Some("&#x10;"),
    Some("&#x11;"),
    Some("&#x12;"),
    Some("&#x13;"),
    Some("&#x14;"),
    Some("&#x15;"),
    Some("&#x16;"),
    Some("&#x17;"),
    Some("&#x18;"),
    Some("&#x19;"),
    Some("&#x1A;"),
    Some("&#x1B;"),
    Some("&#x1C;"),
    Some("&#x1D;"),
    Some("&#x1E;"),
    Some("&#x1F;"),
    Some("&#x20;"),
    Some("&#x21;"),
    Some("&quot;"),
    Some("&#x23;"),
    Some("&#x24;"),
    Some("&#x25;"),
    Some("&amp;"),
    Some("&#x27;"),
    Some("&#x28;"),
    Some("&#x29;"),
    Some("&#x2A;"),
    Some("&#x2B;"),
    Some("&#x2C;"),
    Some("&#x2D;"),
    Some("&#x2E;"),
    Some("&#x2F;"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("&#x3A;"),
    Some("&#x3B;"),
    Some("&lt;"),
    Some("&#x3D;"),
    Some("&gt;"),
    Some("&#x3F;"),
    Some("&#x40;"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("&#x5B;"),
    Some("&#x5C;"),
    Some("&#x5D;"),
    Some("&#x5E;"),
    Some("&#x5F;"),
    Some("&#x60;"),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some("&#x7B;"),
    Some("&#x7C;"),
    Some("&#x7D;"),
    Some("&#x7E;"),
    Some("&#x7F;"),
    Some("&#x80;"),
    Some("&#x81;"),
    Some("&#x82;"),
    Some("&#x83;"),
    Some("&#x84;"),
    Some("&#x85;"),
    Some("&#x86;"),
    Some("&#x87;"),
    Some("&#x88;"),
    Some("&#x89;"),
    Some("&#x8A;"),
    Some("&#x8B;"),
    Some("&#x8C;"),
    Some("&#x8D;"),
    Some("&#x8E;"),
    Some("&#x8F;"),
    Some("&#x90;"),
    Some("&#x91;"),
    Some("&#x92;"),
    Some("&#x93;"),
    Some("&#x94;"),
    Some("&#x95;"),
    Some("&#x96;"),
    Some("&#x97;"),
    Some("&#x98;"),
    Some("&#x99;"),
    Some("&#x9A;"),
    Some("&#x9B;"),
    Some("&#x9C;"),
    Some("&#x9D;"),
    Some("&#x9E;"),
    Some("&#x9F;"),
    Some("&#xA0;"),
    Some("&#xA1;"),
    Some("&#xA2;"),
    Some("&#xA3;"),
    Some("&#xA4;"),
    Some("&#xA5;"),
    Some("&#xA6;"),
    Some("&#xA7;"),
    Some("&#xA8;"),
    Some("&#xA9;"),
    Some("&#xAA;"),
    Some("&#xAB;"),
    Some("&#xAC;"),
    Some("&#xAD;"),
    Some("&#xAE;"),
    Some("&#xAF;"),
    Some("&#xB0;"),
    Some("&#xB1;"),
    Some("&#xB2;"),
    Some("&#xB3;"),
    Some("&#xB4;"),
    Some("&#xB5;"),
    Some("&#xB6;"),
    Some("&#xB7;"),
    Some("&#xB8;"),
    Some("&#xB9;"),
    Some("&#xBA;"),
    Some("&#xBB;"),
    Some("&#xBC;"),
    Some("&#xBD;"),
    Some("&#xBE;"),
    Some("&#xBF;"),
    Some("&#xC0;"),
    Some("&#xC1;"),
    Some("&#xC2;"),
    Some("&#xC3;"),
    Some("&#xC4;"),
    Some("&#xC5;"),
    Some("&#xC6;"),
    Some("&#xC7;"),
    Some("&#xC8;"),
    Some("&#xC9;"),
    Some("&#xCA;"),
    Some("&#xCB;"),
    Some("&#xCC;"),
    Some("&#xCD;"),
    Some("&#xCE;"),
    Some("&#xCF;"),
    Some("&#xD0;"),
    Some("&#xD1;"),
    Some("&#xD2;"),
    Some("&#xD3;"),
    Some("&#xD4;"),
    Some("&#xD5;"),
    Some("&#xD6;"),
    Some("&#xD7;"),
    Some("&#xD8;"),
    Some("&#xD9;"),
    Some("&#xDA;"),
    Some("&#xDB;"),
    Some("&#xDC;"),
    Some("&#xDD;"),
    Some("&#xDE;"),
    Some("&#xDF;"),
    Some("&#xE0;"),
    Some("&#xE1;"),
    Some("&#xE2;"),
    Some("&#xE3;"),
    Some("&#xE4;"),
    Some("&#xE5;"),
    Some("&#xE6;"),
    Some("&#xE7;"),
    Some("&#xE8;"),
    Some("&#xE9;"),
    Some("&#xEA;"),
    Some("&#xEB;"),
    Some("&#xEC;"),
    Some("&#xED;"),
    Some("&#xEE;"),
    Some("&#xEF;"),
    Some("&#xF0;"),
    Some("&#xF1;"),
    Some("&#xF2;"),
    Some("&#xF3;"),
    Some("&#xF4;"),
    Some("&#xF5;"),
    Some("&#xF6;"),
    Some("&#xF7;"),
    Some("&#xF8;"),
    Some("&#xF9;"),
    Some("&#xFA;"),
    Some("&#xFB;"),
    Some("&#xFC;"),
    Some("&#xFD;"),
    Some("&#xFE;"),
    Some("&#xFF;"),
];

/// Encode all single-byte characters in `inp` as HTML numeric entities.
///
/// This is a stricter alternative to [`encode_minimal`] that encodes every
/// ASCII byte (including spaces, semicolons, dashes, etc.) as an HTML entity.
/// Multi-byte Unicode codepoints are passed through unchanged.
///
/// Prefer [`encode_minimal`] for double-quoted XML attribute values in SVG;
/// use this function only when full byte-level escaping is required (e.g.,
/// for unquoted attribute values or legacy HTML contexts).
#[must_use]
pub fn encode_attribute(inp: &str) -> Cow<'_, str> {
    let mut buf = String::new();
    let mut last_idx = 0;
    for (idx, c) in inp.char_indices() {
        if let Ok(b) = <char as TryInto<u8>>::try_into(c)
            && let Some(entity) = ENTITIES[b as usize]
        {
            let fragment = &inp[last_idx..idx];
            buf.reserve(fragment.len() + entity.len());
            buf.push_str(fragment);
            buf.push_str(entity);
            last_idx = idx + c.len_utf8();
        }
    }
    if buf.is_empty() {
        Cow::Borrowed(inp)
    } else {
        buf.push_str(&inp[last_idx..]);
        Cow::Owned(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    #[test]
    fn encode_minimal() {
        for (inp, expected) in [
            ("'a", Some("&#x27;a")),
            ("", None),
            ("'", Some("&#x27;")),
            ("a'", Some("a&#x27;")),
            ("hello world!", None),
            ("&", Some("&amp;")),
            ("<br>", Some("&lt;br&gt;")),
            (
                "\"a\" is not \"b\"",
                Some("&quot;a&quot; is not &quot;b&quot;"),
            ),
        ] {
            let result = super::encode_minimal(inp);
            assert_eq!(result, expected.unwrap_or(inp));
            assert!(matches!(
                (expected, result),
                (None, Cow::Borrowed(_)) | (Some(_), Cow::Owned(_))
            ));
        }
    }

    #[test]
    fn test_encode_attribute() {
        let data = [
            ("", None),
            ("foobar", None),
            ("0 3px", Some("0&#x20;3px")),
            ("<img \"\"\">", Some("&lt;img&#x20;&quot;&quot;&quot;&gt;")),
            ("hej; hå", Some("hej&#x3B;&#x20;h&#xE5;")),
            ("d-none m-0", Some("d&#x2D;none&#x20;m&#x2D;0")),
            (
                "\"bread\" & 奶油",
                Some("&quot;bread&quot;&#x20;&amp;&#x20;奶油"),
            ),
        ];
        for &(input, expected) in data.iter() {
            let actual = super::encode_attribute(input);
            assert_eq!(&actual, expected.unwrap_or(input));
            assert!(matches!(
                (expected, actual),
                (Some(_), Cow::Owned(_)) | (None, Cow::Borrowed(_))
            ));
        }
    }

    const PAYLOADS: &[&str] = &[
        r#""><script>alert(1)</script>"#,
        r#"' onload='alert(1)"#,
        r#"&lt;not-an-entity"#,
        r#"</style><script>bad</script>"#,
        r#"foo & bar"#,
        r#"foo"bar"#,
    ];

    /// Attribute values containing special characters must not appear verbatim in SVG output.
    #[test]
    fn element_attribute_value_no_injection() {
        for payload in PAYLOADS {
            let svg = format!("{}", super::Element::new("g").set("data-x", *payload));
            assert!(
                !svg.contains(payload),
                "raw payload appeared in SVG for value {payload:?}"
            );
        }
    }

    /// Attribute keys containing special characters must not appear verbatim in SVG output.
    #[test]
    fn element_attribute_key_no_injection() {
        for payload in PAYLOADS {
            let svg = format!("{}", super::Element::new("g").set(*payload, "value"));
            assert!(
                !svg.contains(payload),
                "raw payload appeared in SVG for key {payload:?}"
            );
        }
    }

    /// Text content set via `.text()` must be escaped.
    #[test]
    fn element_text_no_injection() {
        for payload in PAYLOADS {
            let svg = format!("{}", super::Element::new("text").text(payload));
            assert!(
                !svg.contains(payload),
                "raw payload appeared in SVG text for {payload:?}"
            );
        }
    }

    #[test]
    fn renderer_rejects_invalid_tag_names() {
        for payload in [
            "",
            "path onclick=\"alert(1)\"",
            "path><script>",
            "9path",
            "svg tag",
        ] {
            let mut output = String::new();
            let mut renderer = super::Renderer::new(&mut output);
            assert!(renderer.start_element(payload).is_err());
            assert!(renderer.end_element(payload).is_err());
        }
    }
}
