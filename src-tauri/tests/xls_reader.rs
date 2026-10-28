#![cfg(target_os = "linux")]
#[path = "../src/workbook.rs"]
#[allow(dead_code)]
mod workbook;

#[test]
fn isolated_xls_reader_uses_the_open_file_and_selects_sheets() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workbook.xls");
    let temp =
        std::env::temp_dir().join(format!("shoruipoppu-xls-test-{}.xls", std::process::id()));
    std::fs::copy(fixture, &temp).unwrap();
    let source = std::fs::File::open(&temp).unwrap();
    std::fs::remove_file(temp).unwrap();
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_shoruipoppu"));
    let first = workbook::read_xls_with_executable(&source, None, executable).unwrap();
    assert_eq!(first.sheets, ["First", "Second"]);
    assert_eq!(first.data.viewport(1, 1, 1, 1).unwrap()[0].value, "42");
    let second = workbook::read_xls_with_executable(&source, Some("Second"), executable).unwrap();
    assert_eq!(
        second.data.viewport(0, 1, 0, 1).unwrap()[0].value,
        "second sheet"
    );
    assert!(workbook::read_xls_with_executable(&source, Some("missing"), executable).is_err());
}

#[test]
fn oversized_sparse_xls_is_rejected_without_killing_the_viewer() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sparse.xls");
    let source = std::fs::File::open(fixture).unwrap();
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_shoruipoppu"));
    let error = match workbook::read_xls_with_executable(&source, None, executable) {
        Ok(_) => panic!("the oversized workbook should be rejected"),
        Err(error) => error,
    };
    assert!(error.contains("memory limit"));
}
