use std::fs;

use railroad::*;

fn main() {
    use std::io::Write;

    let mut seq = Sequence::default();
    seq.push(Box::new(Start) as Box<dyn Node>)
        .push(Box::new(Terminal::new("BEGIN".to_owned())))
        .push(Box::new(NonTerminal::new("syntax".to_owned())))
        .push(Box::new(End));

    let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::LightRendersafe);

    let svg_src = dia.to_string();

    let png_buffer = render::to_png(&svg_src, &render::FitTo::default()).unwrap();

    let mut f = fs::File::create("examples/render.png").unwrap();
    f.write_all(&png_buffer).unwrap();
}
