use std::{
    collections::HashMap,
    fmt::{self, Write},
};

/// A shorthand to draw rounded corners, see `PathData::arc`.
#[derive(Debug, Clone, Copy)]
pub enum Arc {
    EastToNorth,
    EastToSouth,
    NorthToEast,
    NorthToWest,
    SouthToEast,
    SouthToWest,
    WestToNorth,
    WestToSouth,
}

/// Selects the direction in which arrows on positive direction horizontal
/// lines point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HDir {
    LTR,
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

impl Default for HDir {
    fn default() -> Self {
        Self::LTR
    }
}

/// A builder-struct for SVG-Paths
pub struct PathData {
    text: String,
    h_dir: HDir,
}

impl PathData {
    /// Construct a empty `PathData`
    #[must_use]
    pub fn new(h_dir: HDir) -> Self {
        Self {
            text: String::new(),
            h_dir,
        }
    }

    /// Convert to a `Element` of type `path` and fill it's data-attribute
    #[must_use]
    pub fn into_path(self) -> Element {
        Element::new("path").set("d", &self.text)
    }

    /// Move the cursor to this absolute position without drawing anything
    #[must_use]
    pub fn move_to(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " M {x} {y}").unwrap();
        self
    }

    /// Move the cursor relative to the current position without drawing anything
    #[must_use]
    pub fn move_rel(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " m {x} {y}").unwrap();
        self
    }

    /// Draw a line from the cursor's current location to the given relative position
    #[must_use]
    pub fn line_rel(mut self, x: i64, y: i64) -> Self {
        write!(self.text, " l {x} {y}").unwrap();
        self
    }

    /// Draw a horizontal section from the cursor's current position
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

    /// Draw a vertical section from the cursor's current position
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

    /// Draw a rounded corner using the given radius and direction
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
        where T: ToString + ?Sized
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
        where K: ToString + ?Sized,
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
        self.text = Some(htmlescape::encode_minimal(text));
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
}

impl ::std::fmt::Display for Element {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
        write!(f, "<{}", self.name)?;
        let mut attrs = self.attributes.iter().collect::<Vec<_>>();
        attrs.sort_by_key(|(k, _)| *k);
        for (k, v) in attrs {
            write!(f, " {k}=\"{v}\"")?;
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
