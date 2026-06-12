use rr_scip::{TextEncoding, decode_scip_from_bytes, decode_scip_from_path};

use std::{fs, path::PathBuf};

const CSHARP_SCIP_FILE_PATH: &str = "platform_dashboard.scip";
const RUST_SCIP_FILE_PATH: &str = "rr-core.scip";

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("assets")
        .join(file_name)
}

#[test]
fn decodes_rust_analyzer_fixture_from_pathbuf() {
    let index =
        decode_scip_from_path(fixture_path(RUST_SCIP_FILE_PATH)).expect("fixture should decode");
    let metadata = index.metadata.as_ref().expect("metadata should be present");
    let tool_info = metadata
        .tool_info
        .as_ref()
        .expect("tool info should be present");

    assert_eq!(tool_info.name, "rust-analyzer");
    assert_eq!(
        metadata.project_root,
        "file:///home/tony/git/refactor-radar/v1/crates/rr-core"
    );
    assert_eq!(metadata.text_document_encoding, TextEncoding::Utf8 as i32);
    assert!(!index.documents.is_empty());
    assert!(
        index
            .documents
            .iter()
            .any(|document| document.relative_path == "tests/model_tests.rs")
    );
    assert!(
        index
            .documents
            .iter()
            .any(|document| document.relative_path == "src/element_kind.rs")
    );
}

#[test]
fn decodes_scip_dotnet_fixture_from_pathbuf() {
    let index =
        decode_scip_from_path(fixture_path(CSHARP_SCIP_FILE_PATH)).expect("fixture should decode");

    assert!(index.metadata.is_some());
    assert!(!index.documents.is_empty());
    assert!(
        index
            .documents
            .iter()
            .any(|document| !document.relative_path.is_empty())
    );
    assert!(
        index
            .documents
            .iter()
            .any(|document| !document.occurrences.is_empty() || !document.symbols.is_empty())
    );
}

#[test]
fn decodes_rust_analyzer_fixture_from_bytes() {
    let bytes = fs::read(fixture_path(RUST_SCIP_FILE_PATH)).expect("fixture should be readable");
    let index = decode_scip_from_bytes(bytes).expect("fixture should decode");
    let metadata = index.metadata.as_ref().expect("metadata should be present");
    let tool_info = metadata
        .tool_info
        .as_ref()
        .expect("tool info should be present");

    assert_eq!(tool_info.name, "rust-analyzer");
    assert!(!index.documents.is_empty());
    assert!(
        index
            .documents
            .iter()
            .any(|document| document.relative_path == "tests/model_tests.rs")
    );
}

#[test]
fn missing_file_returns_read_error() {
    let error = decode_scip_from_path(fixture_path("missing.scip")).unwrap_err();

    assert_eq!(error.message(), "failed to read SCIP file");
}

#[test]
fn invalid_bytes_return_decode_error() {
    let error = decode_scip_from_bytes([0xff]).unwrap_err();

    assert_eq!(error.message(), "failed to decode SCIP file");
}
