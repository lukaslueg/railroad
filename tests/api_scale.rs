use railroad::{Comment, Diagram, Empty, Node, Repeat, Scale, Terminal, svg};

#[test]
fn adjusted_scale_bounds_rounding_and_preserves_child_geometry() {
    let child = Terminal::new("token".to_owned());
    let natural = child.compute_geometry();
    for requested in [0.63, 1.0, 1.8] {
        let node = Scale::new(&child, requested);
        let geo = node.compute_geometry();
        let factor = node.effective_scale();

        assert_eq!(node.requested_scale(), requested);
        assert!((geo.width as f64 - requested * natural.width as f64).abs() <= 0.5);
        assert_eq!(factor, geo.width as f64 / natural.width as f64);
        for padding in [
            geo.entry_height as f64 - factor * natural.entry_height as f64,
            geo.height_below_entry() as f64 - factor * natural.height_below_entry() as f64,
        ] {
            assert!((0.0..1.0).contains(&padding));
        }
        assert_eq!(geo.children[0].width, natural.width);
        assert_eq!(geo.children[0].height, natural.height);
        assert_eq!(geo.children[0].entry_height, natural.entry_height);
    }
}

#[test]
fn precise_scale_preserves_the_factor_and_rounds_geometry_outward() {
    let child = Terminal::new("token".to_owned());
    for factor in [0.63, 1.0, 1.8] {
        let node = Scale::new_precise(&child, factor);
        assert_eq!(node.requested_scale(), factor);
        assert_eq!(node.effective_scale(), factor);
        for (scaled, natural) in [
            (node.width(), child.width()),
            (node.height(), child.height()),
            (node.entry_height(), child.entry_height()),
        ] {
            assert!((0.0..1.0).contains(&(scaled as f64 - factor * natural as f64)));
        }
    }
}

#[test]
fn scaling_handles_empty_children_and_tiny_target_widths() {
    for node in [Scale::new(Empty, 0.5), Scale::new_precise(Empty, 0.5)] {
        assert_eq!(
            (node.width(), node.height(), node.entry_height()),
            (0, 0, 0)
        );
        assert_eq!(node.effective_scale(), node.requested_scale());
    }
    let child = Terminal::new("token".to_owned());
    let node = Scale::new(&child, 0.25 / child.width() as f64);
    assert_eq!(node.width(), 1);
}

#[test]
fn zero_scale_collapses_the_child() {
    let child = Terminal::new("token".to_owned());
    for node in [Scale::new(&child, 0.0), Scale::new_precise(&child, 0.0)] {
        assert_eq!(
            (node.width(), node.height(), node.entry_height()),
            (0, 0, 0)
        );
        assert_eq!(node.effective_scale(), 0.0);
        assert!(node.compute_geometry().children.is_empty());
        assert_eq!(node.draw(0, 0, svg::HDir::LTR).to_string(), "<g/>\n");
        let mut streamed = String::new();
        node.render(&mut svg::Renderer::new(&mut streamed), 0, 0, svg::HDir::LTR)
            .unwrap();
        assert!(streamed.is_empty());
    }
}

#[test]
fn scaled_nodes_compose_and_render_through_both_backends() {
    let diagram = Diagram::with_default_css(Scale::new(
        Repeat::new(
            Terminal::new("item".to_owned()),
            Scale::new_precise(Comment::new("separator".to_owned()), 0.75),
        ),
        0.63,
    ));
    for direction in [svg::HDir::LTR, svg::HDir::RTL] {
        let element = diagram.draw(0, 0, direction).to_string();
        let mut streamed = String::new();
        diagram
            .render(&mut svg::Renderer::new(&mut streamed), 0, 0, direction)
            .unwrap();
        for output in [element, streamed] {
            for expected in [
                "class=\"scale\"",
                "class=\"repeat\"",
                "transform=",
                "item",
                "separator",
            ] {
                assert!(output.contains(expected));
            }
        }
    }
}
