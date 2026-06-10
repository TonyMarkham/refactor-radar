use crate::{
    ElementBriefParams, ElementParams, ElementQueryParams, FiveWSummaryParams,
    FunctionSignatureParams, LlmBriefParams, McpError, McpResult, ProjectCache, ProjectScanParams,
    ProjectScanResult, ProjectSummaryParams, ReferencesFindParams, RelatedSymbolsParams,
    RustScipGenerateParams, RustScipGenerateResult, ScipProjectParams, ScipProjectResult,
    SourceSpanParams, scip_to_protocol_error, to_protocol_error,
};

use rmcp::{
    ErrorData as ProtocolError, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use rr_core::{ElementSummary, SemanticModel};
use rr_report::{
    build_element_brief, build_project_summary, generate_llm_brief as render_llm_brief,
};
use rr_scip::{Format, generate_rust_scip as run_rust_scip, project_scip_index};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct McpServer {
    cache: Arc<RwLock<ProjectCache>>,
    rust_analyzer_path: Option<PathBuf>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new() -> Self {
        Self::with_rust_analyzer_path(None)
    }

    pub fn with_rust_analyzer_path(rust_analyzer_path: Option<PathBuf>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(ProjectCache::default())),
            rust_analyzer_path,
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
        description = "Generate SCIP and load it into the cache"
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
        let (model, element) =
            resolve_element(&cache, &params.symbol_id).map_err(to_protocol_error)?;
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
          name = "generate_llm_brief",
          description = "Return a compact Markdown project brief",
          output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
    )]
    async fn generate_llm_brief(
        &self,
        Parameters(params): Parameters<LlmBriefParams>,
    ) -> Result<Json<serde_json::Value>, ProtocolError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
            .map_err(to_protocol_error)?;
        let brief = render_llm_brief(model, params.budget_tokens)
            .map_err(|error| McpError::report_generation_failed(error.message()))
            .map_err(to_protocol_error)?;

        structured(serde_json::json!({ "markdown": brief })).map_err(to_protocol_error)
    }
}

fn structured(value: impl Serialize) -> McpResult<Json<serde_json::Value>> {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|error| McpError::serialization_failed(error.to_string()))
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

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("SCIP-backed RefactorRadar semantic query server")
    }
}

#[cfg(test)]
mod tests;
