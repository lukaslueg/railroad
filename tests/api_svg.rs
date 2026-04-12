use std::collections::HashMap;

use railroad::svg;

#[test]
fn renderer_escapes_content_and_rejects_invalid_tag_names() {
    let mut out = String::new();
    let mut renderer = svg::Renderer::new(&mut out);
    renderer
        .text_element("text", "a < b & c", |tag| {
            tag.attr("data-note", "\"quoted\" & tagged")
        })
        .unwrap();
    renderer
        .raw_text_element("style", "text { fill: red < blue; }", |tag| {
            tag.attr("type", "text/css")
        })
        .unwrap();

    assert!(out.contains("<text data-note=\"&quot;quoted&quot; &amp; tagged\">"));
    assert!(out.contains("a &lt; b &amp; c</text>"));
    assert!(out.contains("text { fill: red < blue; }</style>"));

    let mut sink = String::new();
    let mut renderer = svg::Renderer::new(&mut sink);
    assert!(renderer.start_element("1bad").is_err());
    assert!(renderer.end_element("bad tag").is_err());
}

#[test]
fn start_tag_attr_hashmap_writes_attributes_in_key_order() {
    let mut attrs = HashMap::new();
    attrs.insert("b".to_owned(), "2".to_owned());
    attrs.insert("a".to_owned(), "1".to_owned());

    let mut out = String::new();
    let mut renderer = svg::Renderer::new(&mut out);
    let mut tag = renderer.start_element("g").unwrap();
    tag.attr_hashmap(&attrs).unwrap();
    tag.finish_empty().unwrap();

    assert_eq!(out, "<g a=\"1\" b=\"2\"/>\n");
}

#[test]
fn path_data_tracks_direction_and_arrowhead_shape() {
    let ltr = svg::PathData::new(svg::HDir::LTR)
        .move_to(0, 0)
        .horizontal(60)
        .arc(12, svg::Arc::WestToSouth)
        .to_string();
    let rtl = svg::PathData::new(svg::HDir::RTL)
        .move_to(0, 0)
        .horizontal(60)
        .to_string();

    assert_eq!(svg::HDir::LTR.invert(), svg::HDir::RTL);
    assert!(ltr.contains(" M 0 0 h 60"));
    assert!(ltr.contains(" l -5 -5 m 0 10 l 5 -5"));
    assert!(ltr.contains(" a 12 12 0 0 1 12 12"));
    assert!(rtl.contains(" l 5 -5 m 0 10 l -5 -5"));
    assert_ne!(ltr, rtl);
}

#[test]
fn element_serializes_children_text_and_siblings() {
    let xml = svg::Element::new("g")
        .set("id", "root")
        .text("5 < 6")
        .add(svg::Element::new("path").set("data-kind", "child"))
        .append(svg::Element::new("desc").raw_text("trusted <raw>"))
        .to_string();

    assert!(xml.contains("<g id=\"root\">"));
    assert!(xml.contains("5 &lt; 6"));
    assert!(xml.contains("<path data-kind=\"child\"/>"));
    assert!(xml.contains("<desc>\ntrusted <raw></desc>"));
}
