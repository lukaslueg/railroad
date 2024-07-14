use std::{
    borrow::Cow,
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

/// Entity-encode the bare minimum of the given string (`"`, `&`, `<`, `>`, `'`) to allow
/// safely using that string as pure text in an SVG.
pub fn encode_minimal(inp: &str) -> Cow<str> {
    let mut buf = String::new();
    let mut last_idx = 0;
    for (idx, c) in inp.char_indices() {
        if let Some(entity) = match c {
            '"' => Some("&quot;"),
            '&' => Some("&amp;"),
            '<' => Some("&lt;"),
            '>' => Some("&gt;"),
            '\'' => Some("&#x27;"),
            _ => None,
        } {
            buf.push_str(&inp[last_idx..idx]);
            buf.push_str(entity);
            last_idx = idx + 1;
        }
    }
    if !buf.is_empty() {
        buf.push_str(&inp[last_idx..]);
        Cow::Owned(buf)
    } else {
        Cow::Borrowed(inp)
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

/// Encode the given string to allow safely using that string as an attribute-value.
pub fn encode_attribute(inp: &str) -> Cow<str> {
    let mut buf = String::new();
    let mut last_idx = 0;
    for (idx, c) in inp.char_indices() {
        if let Ok(b) = <char as TryInto<u8>>::try_into(c) {
            if let Some(entity) = ENTITIES[b as usize] {
                let fragment = &inp[last_idx..idx];
                buf.reserve(fragment.len() + entity.len());
                buf.push_str(fragment);
                buf.push_str(entity);
                last_idx = idx + c.len_utf8();
            }
        }
    }
    if !buf.is_empty() {
        buf.push_str(&inp[last_idx..]);
        Cow::Owned(buf)
    } else {
        Cow::Borrowed(inp)
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
            eprintln!("now hear this: {}", inp);
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
}
