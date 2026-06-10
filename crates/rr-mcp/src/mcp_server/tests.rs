use super::*;

use protobuf::{Message, MessageField};
use rmcp::handler::server::wrapper::{Json, Parameters};
use scip::types::{
    Document, Index, Metadata, Occurrence, SymbolInformation, SymbolRole, ToolInfo,
    symbol_information,
};
use tempfile::tempdir;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";

#[test]
fn given_tool_params_when_generating_schema_then_rust_analyzer_path_is_not_exposed() {
    let scan_schema = schemars::schema_for!(ProjectScanParams);
    let generate_schema = schemars::schema_for!(RustScipGenerateParams);

    let scan_schema_json = serde_json::to_string(&scan_schema).expect("scan schema serializes");
    let generate_schema_json =
        serde_json::to_string(&generate_schema).expect("generate schema serializes");

    assert!(!scan_schema_json.contains("rust_analyzer_path"));
    assert!(!generate_schema_json.contains("rust_analyzer_path"));
}

#[test]
fn given_tool_params_when_deserializing_then_rust_analyzer_path_is_rejected() {
    let scan_result = serde_json::from_value::<ProjectScanParams>(serde_json::json!({
        "path": "/tmp/project",
        "rust_analyzer_path": "/tmp/rust-analyzer"
    }));
    let generate_result = serde_json::from_value::<RustScipGenerateParams>(serde_json::json!({
        "path": "/tmp/project",
        "output_path": "/tmp/project.scip",
        "rust_analyzer_path": "/tmp/rust-analyzer"
    }));

    assert!(scan_result.is_err());
    assert!(generate_result.is_err());
}

#[tokio::test]
async fn given_scip_file_when_loading_project_then_cache_contains_project()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let (_directory, path) = write_fixture_scip()?;

    let Json(result) = server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: None,
        }))
        .await
        .map_err(to_io_error)?;

    assert_eq!("fixture", result.project_id);
    assert_eq!(1, result.document_count);
    assert!(server.cache.read().await.models.contains_key("fixture"));
    Ok(())
}

#[tokio::test]
async fn given_missing_project_when_getting_summary_then_mcp_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();

    let error = server
        .get_project_summary(Parameters(ProjectSummaryParams {
            project_id: "missing".to_owned(),
        }))
        .await
        .err();

    assert!(error.is_some());
    Ok(())
}

fn write_fixture_scip()
-> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;
    Ok((directory, path))
}

fn sample_index() -> Index {
    Index {
        metadata: MessageField::some(Metadata {
            project_root: "fixture".to_owned(),
            tool_info: MessageField::some(ToolInfo {
                name: "rust-analyzer".to_owned(),
                version: "test".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        documents: vec![Document {
            relative_path: "src/lib.rs".to_owned(),
            language: "rust".to_owned(),
            symbols: vec![SymbolInformation {
                symbol: SYMBOL.to_owned(),
                kind: symbol_information::Kind::Function.into(),
                display_name: "public_sum".to_owned(),
                ..Default::default()
            }],
            occurrences: vec![Occurrence {
                range: vec![0, 0, 10],
                symbol: SYMBOL.to_owned(),
                symbol_roles: SymbolRole::Definition as i32,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn to_io_error(error: rmcp::ErrorData) -> std::io::Error {
    std::io::Error::other(format!("{error:?}"))
}
