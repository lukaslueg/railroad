#[test]
fn test_encode_minimal() {
    assert_eq!(railroad::svg::encode_minimal("foo<bar>"), "foo&lt;bar&gt;");
}

#[test]
fn test_encode_attribute() {
    assert_eq!(
        railroad::svg::encode_attribute("fö bör"),
        "f&#xF6;&#x20;bo\u{308}r"
    );
}
