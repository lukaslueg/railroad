#![cfg(feature = "resvg")]

use railroad::{
    Diagram, Stylesheet, Terminal,
    render::{self, FitTo},
};

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn render_to_png_produces_png_output() {
    let svg = Diagram::new_with_stylesheet(
        Terminal::new("render".to_owned()),
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
fn render_api_reports_invalid_xml() {
    assert_eq!(FitTo::from_size(Some(320), None), FitTo::MaxWidth(320));
    assert!(matches!(
        render::to_png("not xml at all <<<", &FitTo::default()),
        Err(render::Error::XMLParse(_))
    ));
}
