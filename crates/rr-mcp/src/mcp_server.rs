use crate::{
    BriefCacheKey, BriefGenerationCapabilities, BriefLlmClient, BriefLlmRequest,
    BriefTaskOutputSchema, BriefTaskToolCall, BriefWorkPlan, BriefWorkPlanParams, BriefWorkTask,
    ElementBriefParams, ElementParams, ElementQueryParams, FiveWSummaryParams,
    FunctionSignatureParams, GeneratedBrief, GeneratedBriefCacheEntry, GeneratedBriefContent,
    LlmBriefParams, McpError, McpResult, ProjectCache, ProjectScanParams, ProjectScanResult,
    ProjectSummaryParams, ReferencesFindParams, RelatedSymbolsParams, RustScipGenerateParams,
    RustScipGenerateResult, SamplingBriefLlmClient, ScipProjectParams, ScipProjectResult,
    SourceSpanParams, scip_to_protocol_error, to_protocol_error,
};

use rr_core::{ElementSummary, SemanticModel};
use rr_report::{build_element_brief, build_project_summary};
use rr_scip::{Format, generate_rust_scip as run_rust_scip, project_scip_index};

use futures::{StreamExt, stream};
use rmcp::{
    ErrorData as ProtocolError, RoleServer, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

const LLM_BRIEF_PROMPT_VERSION: &str = "5w-summary-v1";
const LLM_BRIEF_INSTRUCTIONS: &str = "Generate one concise 5W summary per item using only evidence_packet facts. Preserve confirmed, inferred, and unknown distinctions. Use unknown when the evidence does not support a claim.";

#[derive(Clone)]
pub struct McpServer {
    cache: Arc<RwLock<ProjectCache>>,
    rust_analyzer_path: Option<PathBuf>,
    brief_model: Option<String>,
    max_concurrent_requests: Option<usize>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new() -> Self {
        Self::with_rust_analyzer_path(None)
    }

    pub fn with_rust_analyzer_path(rust_analyzer_path: Option<PathBuf>) -> Self {
        Self::with_runtime_config(rust_analyzer_path, None, None)
    }

    pub fn with_runtime_config(
        rust_analyzer_path: Option<PathBuf>,
        brief_model: Option<String>,
        max_concurrent_requests: Option<usize>,
    ) -> Self {
        Self {
            cache: Arc::new(RwLock::new(ProjectCache::default())),
            rust_analyzer_path,
            brief_model,
            max_concurrent_requests,
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

// RMCP tool handlers are the protocol boundary: the SDK requires
// rmcp::ErrorData here so errors can be serialized as MCP responses.
// All local validation/report/serialization failures stay as McpError
// or McpResult until they are converted through error_conversion.rs.
#[tool_router]
impl McpServer {
    #[tool(
        name = "load_scip_project",
        description = "Load a SCIP index into the RefactorRadar cache"
    )]
    async fn load_scip_project(
        &self,
        Parameters(params): Parameters<ScipProjectParams>,
    ) -> Result<Json<ScipProjectResult>, ProtocolError> {
        let path = PathBuf::from(&params.path);
        let format = match params.format.as_deref() {
            Some("scip") => Some(Format::Binary),
            Some("json") => Some(Format::Json),
            Some(other) => return Err(to_protocol_error(McpError::unsupported_format(other))),
            None => None,
        };

        let model = project_scip_index(&path, format).map_err(scip_to_protocol_error)?;
        let result = ScipProjectResult {
            project_id: model.project.project_id.clone(),
            document_count: model.files.len(),
            symbol_count: model.elements.len(),
            occurrence_count: occurrence_count(&model),
        };

        self.cache.write().await.insert(model, path, None, None);
        Ok(Json(result))
    }

    #[tool(
        name = "generate_rust_scip",
        description = "Generate a rust-analyzer SCIP file"
    )]
    async fn generate_rust_scip(
        &self,
        Parameters(params): Parameters<RustScipGenerateParams>,
    ) -> Result<Json<RustScipGenerateResult>, ProtocolError> {
        let project_path = PathBuf::from(&params.path);
        let output_path = PathBuf::from(&params.output_path);
        let rust_analyzer_path = self.rust_analyzer_path.clone();
        let config_path = params.config_path.as_deref().map(PathBuf::from);

        let stderr = run_rust_scip(
            &project_path,
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            params.exclude_vendored_libraries.unwrap_or(false),
            params.num_threads,
        )
        .await
        .map_err(scip_to_protocol_error)?;

        let file_size = std::fs::metadata(&output_path)
            .map(|metadata| metadata.len())
            .map_err(|error| {
                McpError::generated_scip_metadata_read_failed(
                    output_path.display().to_string(),
                    error.to_string(),
                )
            })
            .map_err(to_protocol_error)?;

        let (producer_name, producer_version) =
            project_scip_index(&output_path, Some(Format::Binary))
                .ok()
                .map(|model| (model.project.producer_name, model.project.producer_version))
                .unwrap_or((None, None));

        Ok(Json(RustScipGenerateResult {
            path: output_path.display().to_string(),
            stderr_digest: stderr.chars().take(500).collect(),
            producer_name,
            producer_version,
            file_size,
        }))
    }

    #[tool(
        name = "scan_project",
        description = "Scan one source project (Rust crate path today), build and cache its SCIP model, and return the project_id for query tools."
    )]
    async fn scan_project(
        &self,
        Parameters(params): Parameters<ProjectScanParams>,
    ) -> Result<Json<ProjectScanResult>, ProtocolError> {
        let (output_path, temporary_scip_path) = match &params.output_path {
            Some(output_path) => (PathBuf::from(output_path), None),
            None => {
                let temporary_directory = std::env::temp_dir();
                let temp_path = tempfile::Builder::new()
                    .suffix(".scip")
                    .tempfile_in(&temporary_directory)
                    .map_err(|error| {
                        McpError::temporary_scip_output_creation_failed(
                            temporary_directory.display().to_string(),
                            error.to_string(),
                        )
                    })
                    .map_err(to_protocol_error)?
                    .into_temp_path();
                let output_path = temp_path.to_path_buf();
                (output_path, Some(temp_path))
            }
        };

        let project_path = PathBuf::from(&params.path);
        let rust_analyzer_path = self.rust_analyzer_path.clone();
        let config_path = params.config_path.as_deref().map(PathBuf::from);

        let stderr = run_rust_scip(
            &project_path,
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            false,
            None,
        )
        .await
        .map_err(scip_to_protocol_error)?;

        let model = project_scip_index(&output_path, Some(Format::Binary))
            .map_err(scip_to_protocol_error)?;

        let result = ProjectScanResult {
            project_id: model.project.project_id.clone(),
            output_path: output_path.display().to_string(),
            document_count: model.files.len(),
            symbol_count: model.elements.len(),
            occurrence_count: occurrence_count(&model),
        };

        self.cache
            .write()
            .await
            .insert(model, output_path, Some(stderr), temporary_scip_path);

        Ok(Json(result))
    }

    #[tool(
        name = "get_project_summary",
        description = "Return one cached project summary",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_project_summary(
        &self,
        Parameters(params): Parameters<ProjectSummaryParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
            .map_err(to_protocol_error)?;

        structured(build_project_summary(model)).map_err(to_protocol_error)
    }

    #[tool(
        name = "list_elements",
        description = "List cached semantic elements",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn list_elements(
        &self,
        Parameters(params): Parameters<ElementQueryParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
            .map_err(to_protocol_error)?;
        let limit = params.limit.unwrap_or(100);
        let elements: Vec<_> = model
            .elements
            .iter()
            .filter(|element| {
                params
                    .kind
                    .as_ref()
                    .map(|kind| {
                        normalize_kind(&format!("{:?}", element.kind)) == normalize_kind(kind)
                    })
                    .unwrap_or(true)
            })
            .filter(|element| {
                params
                    .path
                    .as_ref()
                    .map(|path| {
                        element
                            .definition_span
                            .as_ref()
                            .map(|span| span.document_path == *path)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .take(limit)
            .collect();

        structured(elements).map_err(to_protocol_error)
    }

    #[tool(
        name = "get_element",
        description = "Return one element by stable ID or SCIP symbol",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_element(
        &self,
        Parameters(params): Parameters<ElementParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let (_model, element) =
            resolve_element(&cache, &params.symbol_id).map_err(to_protocol_error)?;

        structured(element).map_err(to_protocol_error)
    }

    #[tool(
        name = "get_function_signature",
        description = "Return function signature facts",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_function_signature(
        &self,
        Parameters(params): Parameters<FunctionSignatureParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let (_model, element) =
            resolve_element(&cache, &params.symbol_id).map_err(to_protocol_error)?;

        structured(serde_json::json!({
            "stable_id": element.stable_id.as_str(),
            "symbol_id": element.symbol_id.as_str(),
            "definition_span_id": element.definition_span.as_ref().map(|span| span.id.as_str()),
            "signature": &element.signature,
        }))
        .map_err(to_protocol_error)
    }

    #[tool(
        name = "get_element_brief",
        description = "Return a 5W element brief",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_element_brief(
        &self,
        Parameters(params): Parameters<ElementBriefParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let (model, element) = match params.project_id.as_deref() {
            Some(project_id) => {
                let model = cache
                    .models
                    .get(project_id)
                    .ok_or_else(|| McpError::missing_project(project_id.to_owned()))
                    .map_err(to_protocol_error)?;
                let scip_id = params
                    .symbol_id
                    .strip_prefix("scip:")
                    .unwrap_or(params.symbol_id.as_str());
                let element = model
                    .element_by_id(&params.symbol_id)
                    .or_else(|| {
                        if scip_id == params.symbol_id.as_str() {
                            None
                        } else {
                            model.element_by_id(scip_id)
                        }
                    })
                    .ok_or_else(|| McpError::missing_element(params.symbol_id.clone()))
                    .map_err(to_protocol_error)?;
                (model, element)
            }
            None => resolve_element(&cache, &params.symbol_id).map_err(to_protocol_error)?,
        };
        let brief = build_element_brief(
            model,
            element.symbol_id.as_str(),
            params.include_inferred.unwrap_or(true),
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpError::report_generation_failed(error.message()))
        .map_err(to_protocol_error)?;

        structured(brief).map_err(to_protocol_error)
    }

    #[tool(
        name = "get_5w_summary",
        description = "Return the 5W summary and follow-up IDs for an element",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_5w_summary(
        &self,
        Parameters(params): Parameters<FiveWSummaryParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let (model, element) =
            resolve_element(&cache, &params.symbol_id).map_err(to_protocol_error)?;
        let brief = build_element_brief(
            model,
            element.symbol_id.as_str(),
            true,
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpError::report_generation_failed(error.message()))
        .map_err(to_protocol_error)?;

        let stable_id = brief.element.stable_id.as_str().to_owned();
        let symbol_id = brief.element.symbol_id.as_str().to_owned();
        let definition_span_id = brief
            .element
            .definition_span
            .as_ref()
            .map(|span| span.id.as_str().to_owned());

        structured(serde_json::json!({
            "stable_id": stable_id,
            "symbol_id": symbol_id,
            "definition_span_id": definition_span_id,
            "five_w": brief.five_w,
            "evidence": brief.evidence,
            "confirmed": brief.confirmed,
            "inferred": brief.inferred,
            "unknown": brief.unknown,
        }))
        .map_err(to_protocol_error)
    }

    #[tool(
        name = "find_references",
        description = "Return span metadata for symbol references",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn find_references(
        &self,
        Parameters(params): Parameters<ReferencesFindParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let (model, symbol_id) =
            resolve_symbol_id(&cache, &params.symbol_id).map_err(to_protocol_error)?;
        let references: Vec<_> = model
            .references
            .iter()
            .filter(|reference| reference.referenced_symbol_id.as_str() == symbol_id.as_str())
            .take(limit)
            .collect();

        structured(references).map_err(to_protocol_error)
    }

    #[tool(
        name = "find_related_symbols",
        description = "Return child symbols and SCIP-derived function-like relations",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn find_related_symbols(
        &self,
        Parameters(params): Parameters<RelatedSymbolsParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let (model, symbol_id) =
            resolve_symbol_id(&cache, &params.symbol_id).map_err(to_protocol_error)?;

        let child_symbols: Vec<_> = model
            .elements
            .iter()
            .filter(|element| element.enclosing_symbol.as_deref() == Some(symbol_id.as_str()))
            .take(limit)
            .collect();
        let outgoing_function_like_references: Vec<_> = model
            .outgoing_call_edges(&symbol_id)
            .into_iter()
            .take(limit)
            .collect();
        let incoming_function_like_references: Vec<_> = model
            .incoming_call_edges(&symbol_id)
            .into_iter()
            .take(limit)
            .collect();

        structured(serde_json::json!({
            "child_symbols": child_symbols,
            "outgoing_function_like_references": outgoing_function_like_references,
            "incoming_function_like_references": incoming_function_like_references,
        }))
        .map_err(to_protocol_error)
    }

    #[tool(
        name = "get_source_span",
        description = "Return source span metadata without raw source",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_source_span(
        &self,
        Parameters(params): Parameters<SourceSpanParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let span = cache
            .models
            .values()
            .flat_map(|model| model.spans.iter())
            .find(|span| span.id.as_str() == params.span_id.as_str())
            .ok_or_else(|| McpError::missing_span(params.span_id.clone()))
            .map_err(to_protocol_error)?;

        structured(span).map_err(to_protocol_error)
    }

    #[tool(
        name = "get_brief_generation_capabilities",
        description = "Report whether the connected MCP host supports generated brief sampling",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn get_brief_generation_capabilities(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        structured(brief_generation_capabilities_from_peer(&context.peer))
            .map_err(to_protocol_error)
    }

    #[tool(
        name = "generate_brief_work_plan",
        description = "Return a Codex-ready sub-agent work manifest for decentralized brief generation",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn generate_brief_work_plan(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(params): Parameters<BriefWorkPlanParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let generation_capabilities = brief_generation_capabilities_from_peer(&context.peer);
        self.generate_brief_work_plan_with_capabilities(params, generation_capabilities)
            .await
    }

    async fn generate_brief_work_plan_with_capabilities(
        &self,
        params: BriefWorkPlanParams,
        generation_capabilities: BriefGenerationCapabilities,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
            .map_err(to_protocol_error)?;
        let elements =
            select_brief_work_plan_elements(model, &params).map_err(to_protocol_error)?;
        let plan = build_brief_work_plan(model, &params, elements, generation_capabilities);

        structured(plan).map_err(to_protocol_error)
    }

    #[tool(
        name = "generate_llm_brief",
        description = "Generate host-sampled per-element 5W summaries from cached RefactorRadar evidence",
        output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn generate_llm_brief(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(params): Parameters<LlmBriefParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let brief_client = Arc::new(SamplingBriefLlmClient::new(context.peer.clone()));
        self.generate_llm_brief_with_client(params, brief_client)
            .await
    }

    async fn generate_llm_brief_with_client(
        &self,
        params: LlmBriefParams,
        brief_client: Arc<dyn BriefLlmClient>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let model_hint = params
            .brief_model
            .clone()
            .or_else(|| self.brief_model.clone());
        let cache_model_key = model_hint
            .clone()
            .unwrap_or_else(|| "host-default".to_owned());
        let max_concurrent_requests = params
            .max_concurrent_requests
            .or(self.max_concurrent_requests)
            .unwrap_or(1)
            .max(1);
        let max_tokens = params.budget_tokens.unwrap_or(800).min(u32::MAX as usize) as u32;
        let (project_id, work_items) = {
            let cache = self.cache.read().await;
            let model = cache
                .models
                .get(&params.project_id)
                .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
                .map_err(to_protocol_error)?;
            let elements = select_llm_brief_elements(model, &params).map_err(to_protocol_error)?;
            let mut work_items = Vec::with_capacity(elements.len());

            for element in elements {
                let brief = build_element_brief(
                    model,
                    element.symbol_id.as_str(),
                    params.include_inferred.unwrap_or(true),
                    params.reference_limit.unwrap_or(5),
                )
                .map_err(|error| McpError::report_generation_failed(error.message()))
                .map_err(to_protocol_error)?;

                let evidence_packet = serde_json::json!({
                    "symbol_id": brief.element.symbol_id.as_str(),
                    "stable_id": brief.element.stable_id.as_str(),
                    "display_name": brief.element.display_name,
                    "kind": format!("{:?}", brief.element.kind),
                    "five_w": brief.five_w,
                    "evidence": brief.evidence,
                    "confirmed": brief.confirmed,
                    "inferred": brief.inferred,
                    "unknown": brief.unknown,
                });
                let evidence_hash = evidence_hash(&evidence_packet).map_err(to_protocol_error)?;
                let cache_key = BriefCacheKey {
                    project_id: model.project.project_id.clone(),
                    symbol_id: brief.element.symbol_id.as_str().to_owned(),
                    evidence_hash: evidence_hash.clone(),
                    prompt_version: LLM_BRIEF_PROMPT_VERSION.to_owned(),
                    brief_model: cache_model_key.clone(),
                };
                let cached = cache
                    .generated_briefs
                    .get(&cache_key)
                    .map(|entry| entry.brief.clone());

                work_items.push((cache_key, evidence_hash, evidence_packet, cached));
            }

            (model.project.project_id.clone(), work_items)
        };

        let generated_results = stream::iter(work_items)
            .map(|(cache_key, evidence_hash, evidence_packet, cached)| {
                let client = Arc::clone(&brief_client);
                let model_hint = model_hint.clone();
                let instructions = LLM_BRIEF_INSTRUCTIONS.to_owned();

                async move {
                    if let Some(brief) = cached {
                        return Ok(brief);
                    }

                    let (content, generated_by_model) = client
                        .generate(BriefLlmRequest {
                            model_hint,
                            instructions,
                            evidence_packet,
                            max_tokens,
                        })
                        .await?;

                    Ok(generated_brief_from_content(
                        content,
                        evidence_hash,
                        cache_key,
                        generated_by_model,
                        LLM_BRIEF_PROMPT_VERSION,
                    ))
                }
            })
            .buffered(max_concurrent_requests)
            .collect::<Vec<McpResult<GeneratedBrief>>>()
            .await
            .into_iter()
            .collect::<McpResult<Vec<_>>>()
            .map_err(to_protocol_error)?;

        {
            let mut cache = self.cache.write().await;
            for brief in &generated_results {
                cache.generated_briefs.insert(
                    brief.cache_key.clone(),
                    GeneratedBriefCacheEntry {
                        brief: brief.clone(),
                        evidence_hash: brief.evidence_hash.clone(),
                        prompt_version: brief.prompt_version.clone(),
                        model_hint: brief.cache_key.brief_model.clone(),
                        generated_by_model: brief.generated_by_model.clone(),
                    },
                );
            }
        }

        structured(serde_json::json!({
            "project_id": project_id,
            "brief_model": cache_model_key,
            "model_hint": model_hint,
            "prompt_version": LLM_BRIEF_PROMPT_VERSION,
            "max_concurrent_requests": max_concurrent_requests,
            "items": generated_results,
        }))
        .map_err(to_protocol_error)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("SCIP-backed RefactorRadar semantic query server")
    }
}

fn structured(value: impl Serialize) -> McpResult<Json<serde_json::Value>> {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|error| McpError::serialization_failed(error.to_string()))
}

fn evidence_hash(value: impl Serialize) -> McpResult<String> {
    let bytes = serde_json::to_vec(&value)
        .map_err(|error| McpError::serialization_failed(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn select_llm_brief_elements<'a>(
    model: &'a SemanticModel,
    params: &LlmBriefParams,
) -> McpResult<Vec<&'a ElementSummary>> {
    match &params.symbol_ids {
        Some(symbol_ids) => symbol_ids
            .iter()
            .map(|symbol_id| {
                model
                    .element_by_id(symbol_id)
                    .ok_or_else(|| McpError::missing_element(symbol_id.clone()))
            })
            .collect::<McpResult<Vec<_>>>(),
        None => {
            let mut ranked_elements: Vec<_> = model.elements.iter().collect();
            ranked_elements.sort_by(|left, right| {
                right
                    .reference_count
                    .cmp(&left.reference_count)
                    .then_with(|| left.display_name.cmp(&right.display_name))
                    .then_with(|| left.symbol_id.as_str().cmp(right.symbol_id.as_str()))
            });
            Ok(ranked_elements
                .into_iter()
                .take(params.limit.unwrap_or(25))
                .collect())
        }
    }
}

fn brief_generation_capabilities_from_peer(
    peer: &rmcp::Peer<RoleServer>,
) -> BriefGenerationCapabilities {
    let (legacy_sampling_supported, task_sampling_create_message_supported) = peer
        .peer_info()
        .map(|info| {
            let legacy = info.capabilities.sampling.is_some();
            let task = info
                .capabilities
                .tasks
                .as_ref()
                .map(|tasks| tasks.supports_sampling_create_message())
                .unwrap_or(false);
            (legacy, task)
        })
        .unwrap_or((false, false));
    let sampling_supported = legacy_sampling_supported || task_sampling_create_message_supported;
    let notes = if sampling_supported {
        vec!["generate_llm_brief may request sampling/createMessage from this host.".to_owned()]
    } else {
        vec!["Use generate_brief_work_plan and Codex-managed sub-agents instead of generate_llm_brief.".to_owned()]
    };

    BriefGenerationCapabilities {
        sampling_supported,
        legacy_sampling_supported,
        task_sampling_create_message_supported,
        generate_llm_brief_available: sampling_supported,
        deterministic_work_plan_available: true,
        fallback_tool: "generate_brief_work_plan".to_owned(),
        unsupported_error_message: "MCP client does not advertise sampling support".to_owned(),
        notes,
    }
}

fn generated_brief_from_content(
    content: GeneratedBriefContent,
    evidence_hash: String,
    cache_key: BriefCacheKey,
    generated_by_model: String,
    prompt_version: &str,
) -> GeneratedBrief {
    GeneratedBrief {
        summary: content.summary,
        five_w: content.five_w,
        unknowns: content.unknowns,
        evidence_hash,
        cache_key,
        generated_by_model,
        prompt_version: prompt_version.to_owned(),
    }
}

fn select_brief_work_plan_elements<'a>(
    model: &'a SemanticModel,
    params: &BriefWorkPlanParams,
) -> McpResult<Vec<&'a ElementSummary>> {
    match &params.symbol_ids {
        Some(symbol_ids) => symbol_ids
            .iter()
            .map(|symbol_id| {
                model
                    .element_by_id(symbol_id)
                    .ok_or_else(|| McpError::missing_element(symbol_id.clone()))
            })
            .collect::<McpResult<Vec<_>>>(),
        None => {
            let mut ranked_elements: Vec<_> = model.elements.iter().collect();
            ranked_elements.sort_by(|left, right| {
                right
                    .reference_count
                    .cmp(&left.reference_count)
                    .then_with(|| left.display_name.cmp(&right.display_name))
                    .then_with(|| left.symbol_id.as_str().cmp(right.symbol_id.as_str()))
            });
            Ok(ranked_elements
                .into_iter()
                .take(params.limit.unwrap_or(25))
                .collect())
        }
    }
}

fn build_brief_work_plan(
    model: &SemanticModel,
    params: &BriefWorkPlanParams,
    elements: Vec<&ElementSummary>,
    generation_capabilities: BriefGenerationCapabilities,
) -> BriefWorkPlan {
    let max_tasks = params.max_subagent_tasks.unwrap_or(6).max(1);
    let chunk_size = elements.len().div_ceil(max_tasks).max(1);
    let preferred_agent = params
        .preferred_agent
        .clone()
        .unwrap_or_else(|| "explorer".to_owned());
    let include_generated_brief_call = generation_capabilities.generate_llm_brief_available;
    let tasks = elements
        .chunks(chunk_size)
        .enumerate()
        .map(|(index, chunk)| {
            build_brief_work_task(
                model,
                params,
                chunk,
                index,
                preferred_agent.as_str(),
                include_generated_brief_call,
            )
        })
        .collect();

    BriefWorkPlan {
        project_id: model.project.project_id.clone(),
        prompt_version: LLM_BRIEF_PROMPT_VERSION.to_owned(),
        execution_model: "codex-parent-spawns-subagents".to_owned(),
        generation_capabilities,
        parent_instructions: vec![
            "Spawn one Codex sub-agent per task_id.".to_owned(),
            "Pass each sub-agent only its task object and the repository constraints.".to_owned(),
            "Each sub-agent must execute only the MCP tool calls listed in its task.".to_owned(),
            "The MCP server does not spawn or manage Codex sub-agents.".to_owned(),
        ],
        tasks,
        merge_instructions: vec![
            "Merge task outputs by merge_key.".to_owned(),
            "Preserve confirmed, inferred, and unknown distinctions.".to_owned(),
            "Do not collapse different symbol scopes into one claim unless both task outputs support it.".to_owned(),
        ],
        verification_commands: vec![
            "cargo test -p rr-mcp".to_owned(),
            "cargo check -p rr-mcp --all-targets".to_owned(),
        ],
        unknowns: vec![
            "Codex host sub-agent execution is outside rr-mcp and must be explicitly requested by the parent Codex session.".to_owned(),
        ],
    }
}

fn build_brief_work_task(
    model: &SemanticModel,
    params: &BriefWorkPlanParams,
    elements: &[&ElementSummary],
    index: usize,
    preferred_agent: &str,
    include_generated_brief_call: bool,
) -> BriefWorkTask {
    let task_number = index + 1;
    let task_id = format!("brief-task-{task_number:03}");
    let budget_tokens = params.budget_tokens.unwrap_or(800);
    let reference_limit = params.reference_limit.unwrap_or(5);
    let include_inferred = params.include_inferred.unwrap_or(true);
    let symbol_ids = elements
        .iter()
        .map(|element| element.symbol_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let title = brief_work_task_title(elements, task_number);
    let tool_calls = brief_work_tool_calls(
        model.project.project_id.as_str(),
        symbol_ids.as_slice(),
        params.brief_model.clone(),
        budget_tokens,
        reference_limit,
        include_inferred,
        include_generated_brief_call,
    );

    BriefWorkTask {
        task_id: task_id.clone(),
        title,
        preferred_agent: preferred_agent.to_owned(),
        scope: format!(
            "Independent symbol group with {} selected RefactorRadar element(s).",
            symbol_ids.len()
        ),
        symbol_ids,
        tool_calls,
        expected_output_schema: brief_task_output_schema(),
        merge_key: format!("{}::{task_id}", model.project.project_id),
        budget_tokens,
        dependencies: Vec::new(),
        acceptance_criteria: vec![
            "Use only evidence returned by listed MCP tool calls.".to_owned(),
            "Separate confirmed facts from inference.".to_owned(),
            "Put unsupported facts in unknowns.".to_owned(),
            "Return JSON matching expected_output_schema.".to_owned(),
        ],
    }
}

fn brief_work_task_title(elements: &[&ElementSummary], task_number: usize) -> String {
    match elements {
        [element] => format!("Build focused brief for {}", element.display_name),
        _ => format!("Build focused brief for symbol group {task_number:03}"),
    }
}

fn brief_work_tool_calls(
    project_id: &str,
    symbol_ids: &[String],
    brief_model: Option<String>,
    budget_tokens: usize,
    reference_limit: usize,
    include_inferred: bool,
    include_generated_brief_call: bool,
) -> Vec<BriefTaskToolCall> {
    let mut calls = symbol_ids
        .iter()
        .map(|symbol_id| BriefTaskToolCall {
            tool_name: "get_element_brief".to_owned(),
            arguments: serde_json::json!({
                "symbol_id": symbol_id,
                "project_id": project_id,
                "include_inferred": include_inferred,
                "reference_limit": reference_limit
            }),
        })
        .collect::<Vec<_>>();

    if include_generated_brief_call {
        calls.push(BriefTaskToolCall {
            tool_name: "generate_llm_brief".to_owned(),
            arguments: serde_json::json!({
                "project_id": project_id,
                "symbol_ids": symbol_ids,
                "reference_limit": reference_limit,
                "include_inferred": include_inferred,
                "budget_tokens": budget_tokens,
                "brief_model": brief_model,
                "max_concurrent_requests": 1
            }),
        });
    }

    calls
}

fn brief_task_output_schema() -> BriefTaskOutputSchema {
    BriefTaskOutputSchema {
        format: "json".to_owned(),
        required_fields: vec![
            "task_id".to_owned(),
            "merge_key".to_owned(),
            "summary".to_owned(),
            "five_w".to_owned(),
            "confirmed".to_owned(),
            "inferred".to_owned(),
            "unknowns".to_owned(),
            "evidence_hashes".to_owned(),
            "source_span_ids".to_owned(),
        ],
        json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "task_id",
                "merge_key",
                "summary",
                "five_w",
                "confirmed",
                "inferred",
                "unknowns",
                "evidence_hashes",
                "source_span_ids"
            ],
            "properties": {
                "task_id": { "type": "string" },
                "merge_key": { "type": "string" },
                "summary": { "type": "string" },
                "five_w": {
                    "type": "object",
                    "required": ["who", "what", "when", "where", "why", "how"],
                    "properties": {
                        "who": { "type": "string" },
                        "what": { "type": "string" },
                        "when": { "type": "string" },
                        "where": { "type": "string" },
                        "why": { "type": "string" },
                        "how": { "type": "string" }
                    },
                    "additionalProperties": false
                },
                "confirmed": { "type": "array", "items": { "type": "string" } },
                "inferred": { "type": "array", "items": { "type": "string" } },
                "unknowns": { "type": "array", "items": { "type": "string" } },
                "evidence_hashes": { "type": "array", "items": { "type": "string" } },
                "source_span_ids": { "type": "array", "items": { "type": "string" } }
            },
            "additionalProperties": false
        }),
    }
}

fn occurrence_count(model: &SemanticModel) -> usize {
    model.files.iter().map(|file| file.occurrence_count).sum()
}

fn resolve_element<'a>(
    cache: &'a ProjectCache,
    requested_id: &str,
) -> McpResult<(&'a SemanticModel, &'a ElementSummary)> {
    let scip_id = requested_id.strip_prefix("scip:").unwrap_or(requested_id);
    let matches: Vec<_> = cache
        .models
        .values()
        .filter_map(|model| {
            model
                .element_by_id(requested_id)
                .or_else(|| {
                    if scip_id == requested_id {
                        None
                    } else {
                        model.element_by_id(scip_id)
                    }
                })
                .map(|element| (model, element))
        })
        .collect();

    match matches.as_slice() {
        [] => Err(McpError::missing_element(requested_id.to_owned())),
        [(model, element)] => Ok((*model, *element)),
        _ => Err(ambiguous_symbol_mcp_error(
            requested_id,
            matches
                .iter()
                .map(|(model, _element)| model.project.project_id.as_str())
                .collect(),
        )),
    }
}

fn resolve_symbol_id<'a>(
    cache: &'a ProjectCache,
    requested_id: &str,
) -> McpResult<(&'a SemanticModel, String)> {
    let scip_id = requested_id.strip_prefix("scip:").unwrap_or(requested_id);
    let mut matches = Vec::new();

    for model in cache.models.values() {
        if let Some(element) = model.element_by_id(requested_id).or_else(|| {
            if scip_id == requested_id {
                None
            } else {
                model.element_by_id(scip_id)
            }
        }) {
            matches.push((model, element.symbol_id.as_str().to_owned()));
            continue;
        }

        let references_symbol = model
            .references
            .iter()
            .any(|reference| reference.referenced_symbol_id.as_str() == scip_id)
            || model
                .elements
                .iter()
                .any(|element| element.enclosing_symbol.as_deref() == Some(scip_id))
            || model.call_edges.iter().any(|edge| {
                edge.enclosing_symbol_id.as_str() == scip_id
                    || edge.referenced_symbol_id.as_str() == scip_id
            });

        if references_symbol {
            matches.push((model, scip_id.to_owned()));
        }
    }

    match matches.as_slice() {
        [] => Err(McpError::missing_symbol(requested_id.to_owned())),
        [(model, symbol_id)] => Ok((*model, symbol_id.clone())),
        _ => {
            let mut project_ids: Vec<_> = matches
                .iter()
                .map(|(model, _symbol_id)| model.project.project_id.as_str())
                .collect();
            project_ids.sort_unstable();
            project_ids.dedup();
            Err(ambiguous_symbol_mcp_error(requested_id, project_ids))
        }
    }
}

fn ambiguous_symbol_mcp_error(requested_id: &str, project_ids: Vec<&str>) -> McpError {
    let project_ids = project_ids.into_iter().map(str::to_owned).collect();
    McpError::ambiguous_symbol(requested_id, project_ids)
}

fn normalize_kind(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests;
