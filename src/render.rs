//! Render SVG diagrams to PNG using `resvg` with system fonts.
//!
//! Available when the `resvg` feature is enabled.

use resvg::tiny_skia;

/// Errors encountered while rendering SVG to PNG.
#[derive(Debug)]
pub enum Error {
    XMLParse(resvg::usvg::roxmltree::Error),
    SVGParse(resvg::usvg::Error),
    InvalidSize,
    Encoding(String),
}

/// Image dimensions used for scaling while preserving the aspect ratio.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum FitTo {
    /// Width in pixels, scaling the height proportionally.
    MaxWidth(u32),
    /// Height in pixels, scaling the width proportionally.
    MaxHeight(u32),
    /// Fit within the given width and height in pixels.
    MaxSize { width: u32, height: u32 },
}

impl FitTo {
    /// Choose a width, height, or bounding box from the supplied dimensions.
    ///
    /// If both are `None`, use the default 1024 by 1024 bounding box.
    #[must_use]
    pub fn from_size(width: Option<u32>, height: Option<u32>) -> Self {
        match (width, height) {
            (Some(width), None) => Self::MaxWidth(width),
            (Some(width), Some(height)) => Self::MaxSize { width, height },
            (None, Some(height)) => Self::MaxHeight(height),
            (None, None) => Self::default(),
        }
    }

    fn fit(&self, size: tiny_skia::Size) -> Option<(tiny_skia::IntSize, tiny_skia::Transform)> {
        let (max_width, max_height) = match *self {
            Self::MaxWidth(width) => (f64::from(width), f64::INFINITY),
            Self::MaxHeight(height) => (f64::INFINITY, f64::from(height)),
            Self::MaxSize { width, height } => (f64::from(width), f64::from(height)),
        };
        let (svg_width, svg_height) = (f64::from(size.width()), f64::from(size.height()));

        // Keep the limiting dimension exact and round the other up to a whole pixel.
        let (width, height, scale) = if max_width * svg_height <= max_height * svg_width {
            let height = (max_width * svg_height / svg_width).ceil().min(max_height);
            (max_width, height, max_width / svg_width)
        } else {
            let width = (max_height * svg_width / svg_height).ceil().min(max_width);
            (width, max_height, max_height / svg_height)
        };
        let scale = scale as f32;
        if !scale.is_finite() || width > f64::from(u32::MAX) || height > f64::from(u32::MAX) {
            return None;
        }
        let size = tiny_skia::IntSize::from_wh(width as u32, height as u32)?;
        Some((size, tiny_skia::Transform::from_scale(scale, scale)))
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

/// Render SVG source to PNG bytes, scaled according to `fit_to`.
///
/// The output canvas rounds the scaled dimensions up to whole pixels.
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

    let (size, transform) = fit_to.fit(svg_tree.size()).ok_or(Error::InvalidSize)?;
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(Error::InvalidSize)?;

    resvg::render(&svg_tree, transform, &mut pixmap.as_mut());

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
        let result = match to_png(&svg, &FitTo::MaxWidth(200)) {
            Ok(buf) => buf,
            Err(e) => {
                eprintln!("{}", svg);
                panic!("to_png failed with error: {:?}", e);
            }
        };
        assert!(result.starts_with(&PNG_MAGIC), "output is not a PNG");
    }

    #[test]
    fn to_png_max_height_produces_valid_png() {
        let svg = make_svg();
        let result = match to_png(&svg, &FitTo::MaxHeight(200)) {
            Ok(buf) => buf,
            Err(e) => {
                eprintln!("{}", svg);
                panic!("to_png failed with error: {:?}", e);
            }
        };
        assert!(result.starts_with(&PNG_MAGIC), "output is not a PNG");
    }

    #[test]
    fn to_png_max_size_produces_valid_png() {
        let svg = make_svg();
        let result = match to_png(
            &svg,
            &FitTo::MaxSize {
                width: 200,
                height: 200,
            },
        ) {
            Ok(buf) => buf,
            Err(e) => {
                eprintln!("{}", svg);
                panic!("to_png failed with error: {:?}", e);
            }
        };
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
