use crate::{
    BriefLlmClient, BriefLlmRequest, GeneratedBriefContent, GeneratedFiveW, LlmBriefParams,
    McpError, McpResult, McpServer, ProjectScanParams, ProjectSummaryParams,
    RustScipGenerateParams, ScipProjectParams,
};
use protobuf::{Message, MessageField};
use rmcp::handler::server::wrapper::{Json, Parameters};
use scip::types::{
    Document, Index, Metadata, Occurrence, SymbolInformation, SymbolRole, ToolInfo,
    symbol_information,
};
use std::sync::Arc;
use tempfile::tempdir;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
const SECOND_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/second_sum().";
const THIRD_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/third_sum().";

#[derive(Clone, Default)]
struct FakeBriefLlmClient {
    calls: Arc<tokio::sync::Mutex<Vec<BriefLlmRequest>>>,
    active_calls: Arc<std::sync::atomic::AtomicUsize>,
    max_active_calls: Arc<std::sync::atomic::AtomicUsize>,
    fail_invalid_response: bool,
}

#[async_trait::async_trait]
impl BriefLlmClient for FakeBriefLlmClient {
    async fn generate(
        &self,
        request: BriefLlmRequest,
    ) -> McpResult<(GeneratedBriefContent, String)> {
        if self.fail_invalid_response {
            return Err(McpError::brief_generation_response_invalid(
                "fake invalid response",
            ));
        }

        self.calls.lock().await.push(request.clone());
        let active = self
            .active_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        record_max_active(&self.max_active_calls, active);
        tokio::task::yield_now().await;
        self.active_calls
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        Ok((
            GeneratedBriefContent {
                summary: "generated summary from fake sampling".to_owned(),
                five_w: GeneratedFiveW {
                    who: "unknown".to_owned(),
                    what: "generated from fixture evidence".to_owned(),
                    when: "unknown".to_owned(),
                    where_: "src/lib.rs".to_owned(),
                    why: "unknown".to_owned(),
                    how: "SCIP evidence packet".to_owned(),
                },
                unknowns: vec!["runtime behavior is unknown from SCIP".to_owned()],
            },
            request
                .model_hint
                .clone()
                .unwrap_or_else(|| "fake-host-model".to_owned()),
        ))
    }
}

fn record_max_active(max_active_calls: &std::sync::atomic::AtomicUsize, active: usize) {
    let mut current = max_active_calls.load(std::sync::atomic::Ordering::SeqCst);
    while active > current {
        match max_active_calls.compare_exchange(
            current,
            active,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        ) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

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
async fn given_request_model_hint_when_generating_llm_brief_then_generated_summary_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            LlmBriefParams {
                project_id: "fixture".to_owned(),
                symbol_ids: Some(vec![SYMBOL.to_owned()]),
                limit: None,
                reference_limit: Some(3),
                include_inferred: Some(true),
                budget_tokens: Some(500),
                brief_model: Some("test-model-hint".to_owned()),
                max_concurrent_requests: Some(2),
            },
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!("fixture", result["project_id"]);
    assert_eq!("test-model-hint", result["brief_model"]);
    assert_eq!("test-model-hint", result["model_hint"]);
    assert_eq!("5w-summary-v1", result["prompt_version"]);
    assert_eq!(2, result["max_concurrent_requests"]);
    assert!(result["instructions"].is_null());
    assert!(result["summary_schema"].is_null());
    assert!(result["items"][0]["evidence_packet"].is_null());
    assert_eq!(
        "generated summary from fake sampling",
        result["items"][0]["summary"]
    );
    assert_eq!("unknown", result["items"][0]["five_w"]["who"]);
    assert_eq!(
        "generated from fixture evidence",
        result["items"][0]["five_w"]["what"]
    );
    assert_eq!("unknown", result["items"][0]["five_w"]["when"]);
    assert_eq!("src/lib.rs", result["items"][0]["five_w"]["where"]);
    assert_eq!("unknown", result["items"][0]["five_w"]["why"]);
    assert_eq!("SCIP evidence packet", result["items"][0]["five_w"]["how"]);
    assert_eq!(
        "runtime behavior is unknown from SCIP",
        result["items"][0]["unknowns"][0]
    );
    assert_eq!(
        64,
        result["items"][0]["evidence_hash"]
            .as_str()
            .map(str::len)
            .unwrap_or_default()
    );
    assert_eq!(
        result["items"][0]["evidence_hash"],
        result["items"][0]["cache_key"]["evidence_hash"]
    );
    assert_eq!(
        "test-model-hint",
        result["items"][0]["cache_key"]["brief_model"]
    );
    assert_eq!("test-model-hint", result["items"][0]["generated_by_model"]);

    let calls = fake.calls.lock().await;
    assert_eq!(1, calls.len());
    assert_eq!(Some("test-model-hint"), calls[0].model_hint.as_deref());
    assert_eq!(500, calls[0].max_tokens);
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
async fn given_server_model_hint_when_request_hint_is_omitted_then_hint_is_used()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(Some("server-model".to_owned()), None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(None, None, Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!("server-model", result["brief_model"]);
    assert_eq!("server-model", result["model_hint"]);
    assert_eq!(
        "server-model",
        result["items"][0]["cache_key"]["brief_model"]
    );
    assert_eq!("server-model", result["items"][0]["generated_by_model"]);
    assert_eq!(
        Some("server-model"),
        fake.calls.lock().await[0].model_hint.as_deref()
    );
    Ok(())
}

#[tokio::test]
async fn given_no_model_hint_when_generating_llm_brief_then_host_default_cache_key_is_used()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(None, None, Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!("host-default", result["brief_model"]);
    assert!(result["model_hint"].is_null());
    assert_eq!(
        "host-default",
        result["items"][0]["cache_key"]["brief_model"]
    );
    assert_eq!("fake-host-model", result["items"][0]["generated_by_model"]);
    assert_eq!(None, fake.calls.lock().await[0].model_hint.as_deref());
    Ok(())
}

#[tokio::test]
async fn given_request_concurrency_when_server_config_differs_then_request_value_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, Some(1));
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(
                Some("test-model"),
                Some(2),
                Some(3),
                vec![SYMBOL, SECOND_SYMBOL, THIRD_SYMBOL],
            ),
            Arc::new(fake),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(2, result["max_concurrent_requests"]);
    Ok(())
}

#[tokio::test]
async fn given_server_concurrency_when_request_omits_it_then_server_value_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, Some(3));
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(Some("test-model"), None, Some(3), vec![SYMBOL]),
            Arc::new(fake),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(3, result["max_concurrent_requests"]);
    Ok(())
}

#[tokio::test]
async fn given_zero_concurrency_when_generating_llm_brief_then_minimum_one_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(None, Some(0), None, vec![SYMBOL]),
            Arc::new(fake),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(1, result["max_concurrent_requests"]);
    Ok(())
}

#[tokio::test]
async fn given_same_cache_key_when_generating_twice_then_second_call_uses_cache()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    server
        .generate_llm_brief_with_client(
            brief_params(Some("test-model"), Some(1), Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;
    server
        .generate_llm_brief_with_client(
            brief_params(Some("test-model"), Some(1), Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(1, fake.calls.lock().await.len());
    Ok(())
}

#[tokio::test]
async fn given_different_model_hint_when_generating_twice_then_cache_miss_occurs()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    server
        .generate_llm_brief_with_client(
            brief_params(Some("first-model"), Some(1), Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;
    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(Some("second-model"), Some(1), Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(2, fake.calls.lock().await.len());
    assert_eq!(
        "second-model",
        result["items"][0]["cache_key"]["brief_model"]
    );
    Ok(())
}

#[tokio::test]
async fn given_changed_evidence_hash_when_generating_twice_then_cache_miss_occurs()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(first) = server
        .generate_llm_brief_with_client(
            brief_params(Some("test-model"), Some(1), Some(0), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;
    let Json(second) = server
        .generate_llm_brief_with_client(
            brief_params(Some("test-model"), Some(1), Some(3), vec![SYMBOL]),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(2, fake.calls.lock().await.len());
    assert_ne!(
        first["items"][0]["evidence_hash"],
        second["items"][0]["evidence_hash"]
    );
    Ok(())
}

#[tokio::test]
async fn given_multiple_items_when_generating_llm_brief_then_concurrency_cap_is_respected()
-> Result<(), Box<dyn std::error::Error>> {
    let server = server_with_fake_client(None, None);
    load_fixture_project(&server).await?;
    let fake = FakeBriefLlmClient::default();

    let Json(result) = server
        .generate_llm_brief_with_client(
            brief_params(
                Some("test-model"),
                Some(2),
                Some(3),
                vec![SYMBOL, SECOND_SYMBOL, THIRD_SYMBOL],
            ),
            Arc::new(fake.clone()),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!(
        3,
        result["items"].as_array().map(Vec::len).unwrap_or_default()
    );
    assert!(
        fake.max_active_calls
            .load(std::sync::atomic::Ordering::SeqCst)
            <= 2
    );
    Ok(())
}

fn write_fixture_scip()
-> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;
    Ok((directory, path))
}

async fn load_fixture_project(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, path) = write_fixture_scip()?;
    server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: None,
        }))
        .await
        .map_err(to_io_error)?;
    Ok(())
}

fn server_with_fake_client(
    brief_model: Option<String>,
    max_concurrent_requests: Option<usize>,
) -> McpServer {
    McpServer::with_runtime_config(None, brief_model, max_concurrent_requests)
}

fn brief_params(
    brief_model: Option<&str>,
    max_concurrent_requests: Option<usize>,
    reference_limit: Option<usize>,
    symbol_ids: Vec<&str>,
) -> LlmBriefParams {
    LlmBriefParams {
        project_id: "fixture".to_owned(),
        symbol_ids: Some(symbol_ids.into_iter().map(str::to_owned).collect()),
        limit: None,
        reference_limit,
        include_inferred: Some(true),
        budget_tokens: Some(500),
        brief_model: brief_model.map(str::to_owned),
        max_concurrent_requests,
    }
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
            symbols: vec![
                SymbolInformation {
                    symbol: SYMBOL.to_owned(),
                    kind: symbol_information::Kind::Function.into(),
                    display_name: "public_sum".to_owned(),
                    ..Default::default()
                },
                SymbolInformation {
                    symbol: SECOND_SYMBOL.to_owned(),
                    kind: symbol_information::Kind::Function.into(),
                    display_name: "second_sum".to_owned(),
                    ..Default::default()
                },
                SymbolInformation {
                    symbol: THIRD_SYMBOL.to_owned(),
                    kind: symbol_information::Kind::Function.into(),
                    display_name: "third_sum".to_owned(),
                    ..Default::default()
                },
            ],
            occurrences: vec![
                Occurrence {
                    range: vec![0, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: SymbolRole::Definition as i32,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![1, 0, 10],
                    symbol: SECOND_SYMBOL.to_owned(),
                    symbol_roles: SymbolRole::Definition as i32,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![2, 0, 10],
                    symbol: THIRD_SYMBOL.to_owned(),
                    symbol_roles: SymbolRole::Definition as i32,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![3, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: 0,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![4, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: 0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn to_io_error(error: rmcp::ErrorData) -> std::io::Error {
    std::io::Error::other(format!("{error:?}"))
}
