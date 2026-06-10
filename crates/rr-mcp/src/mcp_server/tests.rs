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
fn given_tool_params_when_generating_schema_then_runtime_paths_are_not_exposed() {
    let scan_schema = schemars::schema_for!(ProjectScanParams);
    let generate_schema = schemars::schema_for!(RustScipGenerateParams);
    let brief_schema = schemars::schema_for!(LlmBriefParams);

    let scan_schema_json = serde_json::to_string(&scan_schema).expect("scan schema serializes");
    let generate_schema_json =
        serde_json::to_string(&generate_schema).expect("generate schema serializes");
    let brief_schema_json = serde_json::to_string(&brief_schema).expect("brief schema serializes");

    assert!(!scan_schema_json.contains("rust_analyzer_path"));
    assert!(!generate_schema_json.contains("rust_analyzer_path"));
    assert!(!brief_schema_json.contains("rust_analyzer_path"));
    assert!(brief_schema_json.contains("brief_model"));
    assert!(brief_schema_json.contains("max_concurrent_requests"));
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
    let brief_result = serde_json::from_value::<LlmBriefParams>(serde_json::json!({
        "project_id": "fixture",
        "rust_analyzer_path": "/tmp/rust-analyzer"
    }));

    assert!(scan_result.is_err());
    assert!(generate_result.is_err());
    assert!(brief_result.is_err());
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
async fn given_cached_project_when_generating_llm_brief_then_per_element_inputs_are_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let (_directory, path) = write_fixture_scip()?;

    server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: None,
        }))
        .await
        .map_err(to_io_error)?;

    let Json(result) = server
        .generate_llm_brief(Parameters(LlmBriefParams {
            project_id: "fixture".to_owned(),
            symbol_ids: Some(vec![SYMBOL.to_owned()]),
            limit: None,
            reference_limit: Some(3),
            include_inferred: Some(true),
            budget_tokens: Some(500),
            brief_model: Some("test-brief-model".to_owned()),
            max_concurrent_requests: Some(2),
        }))
        .await
        .map_err(to_io_error)?;

    assert_eq!("fixture", result["project_id"]);
    assert_eq!("test-brief-model", result["brief_model"]);
    assert_eq!("5w-summary-v1", result["prompt_version"]);
    assert!(
        result["instructions"]
            .as_str()
            .map(|instructions| {
                instructions.contains("using only evidence_packet facts")
                    && instructions
                        .contains("Use unknown when the evidence does not support a claim")
            })
            .unwrap_or(false)
    );
    assert_eq!("string", result["summary_schema"]["summary"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["who"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["what"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["when"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["where"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["why"]);
    assert_eq!("string", result["summary_schema"]["five_w"]["how"]);
    assert_eq!(
        "copy item.evidence_hash",
        result["summary_schema"]["evidence_hash"]
    );
    assert_eq!("copy item.cache_key", result["summary_schema"]["cache_key"]);
    assert_eq!(500, result["budget_tokens"]);
    assert_eq!(2, result["max_concurrent_requests"]);
    assert_eq!(
        1,
        result["items"].as_array().map(Vec::len).unwrap_or_default()
    );
    assert_eq!(
        64,
        result["items"][0]["evidence_hash"]
            .as_str()
            .map(str::len)
            .unwrap_or_default()
    );
    assert_eq!("fixture", result["items"][0]["cache_key"]["project_id"]);
    assert_eq!(SYMBOL, result["items"][0]["cache_key"]["symbol_id"]);
    assert_eq!(
        result["items"][0]["evidence_hash"],
        result["items"][0]["cache_key"]["evidence_hash"]
    );
    assert_eq!(
        "5w-summary-v1",
        result["items"][0]["cache_key"]["prompt_version"]
    );
    assert_eq!(
        "test-brief-model",
        result["items"][0]["cache_key"]["brief_model"]
    );
    assert_eq!(SYMBOL, result["items"][0]["evidence_packet"]["symbol_id"]);
    assert_eq!(
        "public_sum",
        result["items"][0]["evidence_packet"]["display_name"]
    );
    assert!(result["items"][0]["evidence_packet"]["five_w"].is_object());
    assert!(result["items"][0]["evidence_packet"]["evidence"].is_array());
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

#[tokio::test]
async fn given_zero_concurrency_when_generating_llm_brief_then_minimum_one_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let (_directory, path) = write_fixture_scip()?;

    server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: None,
        }))
        .await
        .map_err(to_io_error)?;

    let Json(result) = server
        .generate_llm_brief(Parameters(LlmBriefParams {
            project_id: "fixture".to_owned(),
            symbol_ids: Some(vec![SYMBOL.to_owned()]),
            limit: None,
            reference_limit: None,
            include_inferred: None,
            budget_tokens: None,
            brief_model: None,
            max_concurrent_requests: Some(0),
        }))
        .await
        .map_err(to_io_error)?;

    assert_eq!(1, result["max_concurrent_requests"]);
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
