#[test]
fn test_encode_minimal() {
    assert_eq!(railroad::svg::encode_minimal("foo<bar>"), "foo&lt;bar&gt;");
}
