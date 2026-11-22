#![cfg(feature = "resvg")]

use railroad::{
    Continuation, ContinuationEnd, ContinuationStart, Diagram, Node, Sequence, Stylesheet,
    Terminal,
    render::{self, FitTo},
    resvg::tiny_skia::Pixmap,
};

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn render_to_png_produces_png_output() {
    let svg = Diagram::new_with_stylesheet(
        Sequence::<Box<dyn Node>>::new(vec![
            Box::new(ContinuationStart),
            Box::new(Terminal::new("BEGIN".to_owned())),
            Box::new(Continuation),
            Box::new(Terminal::new("END".to_owned())),
            Box::new(ContinuationEnd),
        ]),
        &Stylesheet::LightRendersafe,
    )
    .to_string();

    let png = render::to_png(
        &svg,
        &FitTo::MaxSize {
            width: 128,
            height: 128,
        },
    )
    .unwrap();

    assert!(png.starts_with(PNG_MAGIC));
}

#[test]
fn png_fit_uses_fractional_svg_dimensions() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0.5" height="0.75">
        <rect width="0.5" height="0.75"/>
    </svg>"#;
    for fit in [
        FitTo::MaxWidth(60),
        FitTo::MaxHeight(90),
        FitTo::MaxSize {
            width: 100,
            height: 90,
        },
    ] {
        let png = render::to_png(svg, &fit).unwrap();
        let pixmap = Pixmap::decode_png(&png).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (60, 90), "{fit:?}");
        assert_eq!(pixmap.pixel(59, 89).unwrap().alpha(), 255, "{fit:?}");
    }
}

#[test]
fn render_api_reports_invalid_xml() {
    assert_eq!(FitTo::from_size(Some(320), None), FitTo::MaxWidth(320));
    assert!(matches!(
        render::to_png("not xml at all <<<", &FitTo::default()),
        Err(render::Error::XMLParse(_))
    ));
}
