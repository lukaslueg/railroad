//! A shorthand for rendering diagrams to images, using `resvg`'s default options.
//!
//! This module is only available if the `resvg`-feature is active.

/// Errors encountered while rendering
#[derive(Debug)]
pub enum Error {
    XMLParse(resvg::usvg::roxmltree::Error),
    SVGParse(resvg::usvg::Error),
    InvalidSize,
    Encoding(String),
}

/// Scales the final image, preserving aspect-ratio
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum FitTo {
    /// Maximum width in pixels, scaling height as necessary
    MaxWidth(u32),
    /// Maximum hight in pixels, scaling width as necessary
    MaxHeight(u32),
    /// Miximum height and width in pixels, scaling as necessary
    MaxSize { width: u32, height: u32 },
}

impl FitTo {
    #[must_use]
    pub fn from_size(width: Option<u32>, height: Option<u32>) -> Self {
        match (width, height) {
            (Some(width), None) => Self::MaxWidth(width),
            (Some(width), Some(height)) => Self::MaxSize { width, height },
            (None, Some(height)) => Self::MaxHeight(height),
            (None, None) => Self::default(),
        }
    }

    fn fit_to_size(&self, size: resvg::tiny_skia::IntSize) -> Option<resvg::tiny_skia::IntSize> {
        match self {
            Self::MaxWidth(w) => size.scale_to_width(*w),
            Self::MaxHeight(h) => size.scale_to_height(*h),
            Self::MaxSize { width, height } => {
                resvg::tiny_skia::IntSize::from_wh(*width, *height).map(|s| size.scale_to(s))
            }
        }
    }

    fn fit_to_transform(&self, size: resvg::tiny_skia::IntSize) -> resvg::tiny_skia::Transform {
        let size1 = size.to_size();
        let size2 = match self.fit_to_size(size) {
            Some(v) => v.to_size(),
            None => return resvg::tiny_skia::Transform::default(),
        };
        resvg::tiny_skia::Transform::from_scale(
            size2.width() / size1.width(),
            size2.height() / size1.height(),
        )
    }
}

impl Default for FitTo {
    fn default() -> Self {
        Self::MaxSize {
            width: 1024,
            height: 1024,
        }
    }
}

static USVG_OPTS: std::sync::LazyLock<resvg::usvg::Options> = std::sync::LazyLock::new(|| {
    let mut opts = resvg::usvg::Options::default();
    opts.fontdb_mut().load_system_fonts();
    opts
});

/// Render the given svg-source to an image in png-format.
///
/// ```rust
/// use railroad::*;
///
/// let mut seq = Sequence::default();
/// seq.push(Box::new(Start) as Box<dyn Node>)
///    .push(Box::new(Terminal::new("BEGIN".to_owned())))
///    .push(Box::new(NonTerminal::new("syntax".to_owned())))
///    .push(Box::new(End));
/// let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::Light);
/// let svg_src = dia.to_string();
///
/// let png_buffer: Vec<u8> = render::to_png(&svg_src, &render::FitTo::default()).unwrap();
/// ```
#[allow(clippy::missing_errors_doc)]
pub fn to_png(svg_src: &str, fit_to: &FitTo) -> Result<Vec<u8>, Error> {
    let xml_tree = resvg::usvg::roxmltree::Document::parse_with_options(
        svg_src,
        resvg::usvg::roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .map_err(Error::XMLParse)?;

    let svg_tree =
        resvg::usvg::Tree::from_xmltree(&xml_tree, &USVG_OPTS).map_err(Error::SVGParse)?;

    let size = fit_to
        .fit_to_size(svg_tree.size().to_int_size())
        .ok_or(Error::InvalidSize)?;

    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(Error::InvalidSize)?;

    let ts = fit_to.fit_to_transform(svg_tree.size().to_int_size());

    resvg::render(&svg_tree, ts, &mut pixmap.as_mut());

    let png_buf = pixmap
        .encode_png()
        .map_err(|e| Error::Encoding(e.to_string()))?;
    Ok(png_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagram, Stylesheet, Terminal};

    const PNG_MAGIC: [u8; 4] = [0x89, 0x50, 0x4e, 0x47];

    fn make_svg() -> String {
        Diagram::new_with_stylesheet(
            Terminal::new("test".to_owned()),
            &Stylesheet::LightRendersafe,
        )
        .to_string()
    }

    // --- FitTo::from_size ---

    #[test]
    fn fit_to_from_size_width_only() {
        assert_eq!(FitTo::from_size(Some(800), None), FitTo::MaxWidth(800));
    }

    #[test]
    fn fit_to_from_size_height_only() {
        assert_eq!(FitTo::from_size(None, Some(600)), FitTo::MaxHeight(600));
    }

    #[test]
    fn fit_to_from_size_both() {
        assert_eq!(
            FitTo::from_size(Some(800), Some(600)),
            FitTo::MaxSize {
                width: 800,
                height: 600
            }
        );
    }

    #[test]
    fn fit_to_from_size_neither() {
        assert_eq!(FitTo::from_size(None, None), FitTo::default());
    }

    // --- to_png happy path ---

    #[test]
    fn to_png_max_width_produces_valid_png() {
        let svg = make_svg();
        let result = to_png(&svg, &FitTo::MaxWidth(200)).unwrap();
        assert!(result.starts_with(&PNG_MAGIC), "output is not a PNG");
    }

    #[test]
    fn to_png_max_height_produces_valid_png() {
        let svg = make_svg();
        let result = to_png(&svg, &FitTo::MaxHeight(200)).unwrap();
        assert!(result.starts_with(&PNG_MAGIC), "output is not a PNG");
    }

    #[test]
    fn to_png_max_size_produces_valid_png() {
        let svg = make_svg();
        let result = to_png(
            &svg,
            &FitTo::MaxSize {
                width: 200,
                height: 200,
            },
        )
        .unwrap();
        assert!(result.starts_with(&PNG_MAGIC), "output is not a PNG");
    }

    // --- to_png error paths ---

    #[test]
    fn to_png_invalid_xml_returns_xml_parse_error() {
        let result = to_png("not xml at all <<<", &FitTo::default());
        assert!(matches!(result, Err(Error::XMLParse(_))));
    }

    #[test]
    fn to_png_valid_xml_not_svg_returns_svg_parse_error() {
        let result = to_png("<foo/>", &FitTo::default());
        assert!(matches!(result, Err(Error::SVGParse(_))));
    }
}
