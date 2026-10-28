#[path = "../src/office.rs"]
#[allow(dead_code)]
mod office;

#[test]
fn reads_generated_docx_with_text_table_and_image() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example.docx");
    let blocks = serde_json::to_value(office::docx(&path).unwrap()).unwrap();
    let json = blocks.to_string();
    assert!(json.contains("Example heading"));
    assert!(json.contains("Bold text"));
    assert!(json.contains("Table cell"));
    assert!(json.contains("data:image/png;base64,"));
}

#[test]
fn reads_generated_pptx_with_slides_shapes_and_image() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example.pptx");
    let mut presentation = office::Presentation::open(&path).unwrap();
    let (count, first) = presentation.slide(0).unwrap();
    let (_, second) = presentation.slide(1).unwrap();
    let first = serde_json::to_value(first).unwrap().to_string();
    let second = serde_json::to_value(second).unwrap().to_string();
    assert_eq!(count, 2);
    assert!(first.contains("First slide"));
    assert!(first.contains("#CC3344"));
    assert!(first.contains("data:image/png;base64,"));
    assert!(second.contains("Second slide"));
}
