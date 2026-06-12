# Plan: refactor-radar-mcp

## Objective

Create `crates/refactor-radar-mcp` as a local stdio MCP server for querying
SCIP-backed RefactorRadar semantic models. It provides compact structured tools
for loading or scanning Rust projects, querying elements and references, and
returning 5W summaries without returning raw source by default.

## Constraints

- Follow `AGENTS.md` Rust error handling rules.
- Do not use `anyhow`.
- Define crate-local `McpServerError` and `McpServerResult<T>`.
- Every error variant must carry `location: ErrorLocation`.
- Public error constructors must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Provide stable plain messages through `McpServerError::message()`.
- Use at most one named type definition per Rust source file; helper-only files
  such as `lib.rs`, `src/bin/refactor-radar-mcp.rs`, and conversion modules may
  have zero named type definitions.
- The MCP stdio server must never write logs or diagnostics to stdout.
- `src/bin/refactor-radar-mcp.rs` may write startup failures to stderr.
- Library code and MCP handlers must not write to stdout.
- Runtime query operation must be local-first and not require network access.
- Missing rust-analyzer must be returned as a typed MCP error, not installed.

## Prerequisites

Before creating this crate, verify that these prerequisite crates already exist
and compile in the workspace:

- `crates/refactor-radar-core`
- `crates/refactor-radar-scip`
- `crates/refactor-radar-report`

Also verify that the prerequisite crates expose the public APIs this plan uses:

- `refactor_radar_core::{ElementSummary, SemanticModel, SourceSpan, StableId,
  SymbolId, SymbolReferenceSummary}`; `SemanticModel` must expose the public
  fields and query methods used below, including `project`, `files`,
  `elements`, `references`, `spans`, `call_edges`, `element_by_id`,
  `outgoing_call_edges`, and `incoming_call_edges`. `element_by_id` must accept
  the string IDs passed by MCP handlers, and incoming/outgoing call-edge queries
  must accept the raw SCIP symbol string used by `resolve_symbol_id`. Core
  values returned directly by MCP tools, including elements, references, spans,
  and call-edge items returned by these methods, must implement
  `serde::Serialize`;
  `StableId`, `SymbolId`, and span ID values exposed by these types must provide
  `as_str()` accessors; `SourceSpan` must expose the `from_scip_range`
  constructor and `SymbolId` must expose the `new` constructor used by the MCP
  tests.
- `refactor_radar_scip::{generate_rust_scip, project_scip_index, Format,
  ScipError}`; `generate_rust_scip` must accept project path, output path,
  optional rust-analyzer path, optional config path, `exclude_vendored_libraries`,
  and optional thread count in the order used by the handlers below, and
  must be async and return the captured stderr text on success;
  `project_scip_index` must accept a path plus `Option<Format>`. `Format` must
  expose the `Binary` and `Json` variants used below, and `ScipError` must
  expose the typed variants used by the error mapping section.
- `refactor_radar_report::{build_element_brief, build_project_summary,
  generate_llm_brief}`; `build_project_summary` must accept `&SemanticModel`,
  `build_element_brief` must accept `&SemanticModel`, symbol ID, include-inferred
  flag, and reference limit, and `generate_llm_brief` must accept
  `&SemanticModel` plus optional token budget. The value returned by
  `build_element_brief` must expose the public `element`, `five_w`, `evidence`,
  `confirmed`, `inferred`, and `unknown` fields used by the MCP handlers. Report
  errors returned by these functions must expose stable `message()` text for MCP
  error conversion, and report values returned directly by MCP tools must
  implement `serde::Serialize`.

If any prerequisite crate is absent, stop this plan and implement the missing
prerequisite crate first. If any required public API is absent, stop this plan
and update the corresponding prerequisite crate before continuing. This MCP
crate depends on those public APIs and should not duplicate their model
projection, SCIP generation, or brief-generation behavior.

## Workspace Setup

Create the crate with:

```bash
cargo new --lib crates/refactor-radar-mcp --name refactor-radar-mcp --vcs none
mkdir -p crates/refactor-radar-mcp/src/bin
```

Add it to the root workspace member list after `crates/refactor-radar-report`.

Before using the crate manifest below, ensure the root `[workspace.dependencies]`
contains these entries in addition to any dependencies added by prerequisite
crates:

```toml
protobuf = "=3.7.2"
rmcp = { version = "1.7.0", default-features = false, features = ["server", "macros", "schemars", "transport-io"] }
schemars = { version = "1.1", features = ["derive"] }
scip = { path = "submodules/scip/bindings/rust" }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0" }
tempfile = { version = "3" }
tokio = { version = "1", features = ["macros", "process", "rt-multi-thread", "io-std", "io-util", "sync"] }
```

Use this crate manifest shape:

```toml
[package]
name = "refactor-radar-mcp"
version.workspace = true
edition.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
error-location.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
refactor-radar-report = { path = "../refactor-radar-report" }
refactor-radar-scip = { path = "../refactor-radar-scip" }
rmcp.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
tempfile.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
protobuf.workspace = true
scip.workspace = true
```

## Source Layout

Create these files under `crates/refactor-radar-mcp/src`:

```text
lib.rs
element_query_params.rs
find_references_params.rs
find_related_symbols_params.rs
generate_llm_brief_params.rs
generate_rust_scip_params.rs
generate_rust_scip_result.rs
get_5w_summary_params.rs
get_element_brief_params.rs
get_element_params.rs
get_function_signature_params.rs
get_project_summary_params.rs
get_source_span_params.rs
load_scip_project_params.rs
load_scip_project_result.rs
mcp_error_conversion.rs
mcp_server_error.rs
mcp_server_result.rs
refactor_radar_mcp_server.rs
scan_cache.rs
scan_project_params.rs
scan_project_result.rs
```

Create the binary entrypoint:

```text
crates/refactor-radar-mcp/src/bin/refactor-radar-mcp.rs
```

`lib.rs` should declare modules and re-export `McpServerError`,
`McpServerResult`, and `RefactorRadarMcpServer`.

Each params file should define exactly one params struct deriving
`serde::Deserialize` and `schemars::JsonSchema`. Each named result file should
define exactly one result struct deriving `serde::Serialize` and
`schemars::JsonSchema`; compact response shapes that use `serde_json::Value`
do not need dedicated result types.

## Cache

Implement `ScanCache` with:

- `models: HashMap<String, SemanticModel>`
- `scip_paths: HashMap<String, PathBuf>`
- `temporary_scip_paths: HashMap<String, tempfile::TempPath>` for
  omitted-output scans, keyed by project ID, so cached temporary SCIP files
  remain available while the cache owns them and are removed when the cache
  drops.
- `producer_metadata: HashMap<String, String>`
- `generation_diagnostics: HashMap<String, String>`

`RefactorRadarMcpServer` should own:

- `Arc<RwLock<ScanCache>>`
- `ToolRouter<Self>`

Provide `new()` and `Default`.

Implement rmcp integration in `refactor_radar_mcp_server.rs`:

- Import `ToolRouter` from `rmcp::handler::server::router::tool::ToolRouter`.
- Import `Json` and `Parameters` from `rmcp::handler::server::wrapper`.
- Use `#[tool_router]` on the `impl RefactorRadarMcpServer` block that defines
  the tool methods and initialize `tool_router` with `Self::tool_router()` in
  `new()`.
- Define each `#[tool]` method with a single `Parameters<...>` argument using
  the matching params type from the source layout; the tool signatures listed
  below describe MCP JSON fields, not multiple Rust function parameters.
- Return structured tool output through `Result<Json<T>, rmcp::ErrorData>`,
  where `T` is the planned result type or `serde_json::Value` for compact
  response shapes that do not need a dedicated named result type.
- For tools that return values from `refactor-radar-core` or
  `refactor-radar-report`, convert the serializable value to
  `serde_json::Value` and return `Json<serde_json::Value>`. Do not require this
  MCP crate to add `schemars::JsonSchema` derives to prerequisite crate types.
  Map serialization failures to a typed internal MCP error before conversion to
  `rmcp::ErrorData`.
- Use `#[tool_handler(router = self.tool_router)]` to implement
  `rmcp::ServerHandler` for `RefactorRadarMcpServer`, because rmcp 1.7's
  default router expression is `Self::tool_router()` and would not read the
  stored `tool_router` field.
- Return `ServerInfo` with
  `ServerCapabilities::builder().enable_tools().build()` so stdio serving can
  advertise and route tools.

## MCP Tools

Implement these initial tools:

- `generate_rust_scip(path, output_path, rust_analyzer_path?, config_path?,
  exclude_vendored_libraries?, num_threads?)`
- `load_scip_project(path, format?)`
- `scan_project(path, output_path?, rust_analyzer_path?, config_path?)`
- `get_project_summary(project_id)`
- `list_elements(project_id, kind?, path?, limit?)`
- `get_element(symbol_id)`
- `get_function_signature(symbol_id)`
- `get_element_brief(symbol_id, include_inferred?, reference_limit?)`
- `get_5w_summary(symbol_id, reference_limit?)`
- `find_references(symbol_id, limit?)`
- `find_related_symbols(symbol_id, limit?)`
- `get_source_span(span_id)`
- `generate_llm_brief(project_id, budget_tokens?)`

Response rules:

- Return compact structured JSON by default.
- Never return raw source by default.
- Include stable IDs and source span IDs for follow-up queries.
- Preserve SCIP IDs in responses for traceability.
- Label `confirmed`, `inferred`, and `unknown` explicitly where summaries are
  returned.
- Use `function_like_references` wording for SCIP-derived call data unless a
  future AST supplement confirms exact call expressions.

## Tool Behavior

`load_scip_project`:

- Accept `format` values `scip`, `json`, or omitted.
- Reject unsupported formats as invalid params.
- Load with `refactor_radar_scip::project_scip_index`.
- Cache the projected model.
- Return project ID, document count, symbol count, and occurrence count.

`generate_rust_scip`:

- Run `refactor_radar_scip::generate_rust_scip`.
- Return output path, stderr digest capped to 500 chars, producer metadata when
  the output can be loaded, and file size.
- Do not cache the model unless this becomes part of a later explicit change.

`scan_project`:

- Generate SCIP to supplied output path or a cache-owned temporary `.scip` path.
- When `output_path` is omitted, create a named temporary file with a `.scip`
  suffix, immediately convert it with `into_temp_path()` so the server owns
  cleanup without keeping the file handle open, clone `temp_path.to_path_buf()`
  for generation and response data, and pass that path to
  `refactor_radar_scip::generate_rust_scip`. After the generated SCIP is loaded
  into the model cache, store the `TempPath` in `temporary_scip_paths` under the
  project ID instead of calling `TempPath::keep()`. Do not cache or return only
  a `PathBuf` whose `TempPath` or `NamedTempFile` will be dropped at handler
  return.
- Load the generated SCIP into cache.
- Store generation diagnostics.
- Return project ID, output path, document count, symbol count, and occurrence
  count.

Query tools:

- Locate cached projects by project ID when the tool includes one.
- For symbol queries, accept stable IDs or raw SCIP IDs.
- Resolve symbol-only queries across all cached projects through a shared helper
  that returns a match only when the supplied stable ID or raw SCIP ID identifies
  exactly one cached element or reference set. If no cached project matches,
  return resource-not-found; if more than one cached project matches, return
  invalid params with the requested symbol ID and matching project IDs instead
  of choosing the first match. The helper must return both the matched cached
  model and the raw SCIP symbol ID so reference and related-symbol queries read
  only the matched project after resolution.
- `find_references` should also accept external raw SCIP IDs that appear only
  in references.
- `find_related_symbols` should return child symbols plus incoming and outgoing
  SCIP-derived function-like references.
- `get_source_span` returns metadata only.
- `get_project_summary` delegates to
  `refactor_radar_report::build_project_summary`.
- `get_element_brief` delegates to
  `refactor_radar_report::build_element_brief`.
- `get_5w_summary` delegates to
  `refactor_radar_report::build_element_brief` and returns the `five_w`
  portion with the relevant IDs, evidence, and explicit `confirmed`,
  `inferred`, and `unknown` arrays.
- `generate_llm_brief` delegates to
  `refactor_radar_report::generate_llm_brief`.

## Error Mapping

Implement MCP error conversion:

- Missing project, element, symbol, or span maps to MCP resource-not-found.
- Unsupported explicit `format` values map to MCP invalid params with the
  rejected format.
- Local validation and ambiguity failures must be represented as typed
  `McpServerError` variants carrying `ErrorLocation` before conversion to
  `rmcp::ErrorData`; do not construct local application errors directly as
  protocol errors inside handlers.
- `McpServerError` must use precise local domain variants rather than broad
  catch-all `NotFound`, `InvalidParams`, or `Internal` variants. Include
  variants for missing projects, missing elements, missing symbols, missing
  spans, unsupported formats, ambiguous symbols, serialization failures,
  generated SCIP metadata read failures, temporary SCIP output creation
  failures, report-generation failures, and server runtime failures.
- Unknown inferred SCIP format from the input path maps to MCP invalid params
  with the path.
- Missing rust-analyzer maps to internal error with executable path.
- Launch failure maps to internal error with executable and details.
- Nonzero rust-analyzer exit maps to internal error with status and stderr.
- Read and parse failures map to internal error with path.
- Temporary SCIP output creation failures map to internal error with path and
  details.
- Core projection failures map to internal error with stable message.

## Binary Entrypoint

Implement `src/bin/refactor-radar-mcp.rs`:

- Use `#[tokio::main]`.
- Import `rmcp::ServiceExt` so `.serve(...)` is in scope.
- Serve `RefactorRadarMcpServer::new()` over `rmcp::transport::stdio`.
- Await service completion with `service.waiting().await`.
- Return `ExitCode::SUCCESS` or `ExitCode::FAILURE`.
- Print startup/runtime failure messages only with `eprintln!`.

## Concrete Implementation Snippets

Use these snippets as the concrete starting implementation for the MCP crate.
Where the behavioral requirements above are stricter than a snippet, implement
the stricter requirement.

```rust
// crates/refactor-radar-mcp/src/lib.rs
pub mod element_query_params;
pub mod find_references_params;
pub mod find_related_symbols_params;
pub mod generate_llm_brief_params;
pub mod generate_rust_scip_params;
pub mod generate_rust_scip_result;
pub mod get_5w_summary_params;
pub mod get_element_brief_params;
pub mod get_element_params;
pub mod get_function_signature_params;
pub mod get_project_summary_params;
pub mod get_source_span_params;
pub mod load_scip_project_params;
pub mod load_scip_project_result;
pub mod mcp_error_conversion;
pub mod mcp_server_error;
pub mod mcp_server_result;
pub mod refactor_radar_mcp_server;
pub mod scan_cache;
pub mod scan_project_params;
pub mod scan_project_result;

pub use mcp_server_error::McpServerError;
pub use mcp_server_result::McpServerResult;
pub use refactor_radar_mcp_server::RefactorRadarMcpServer;
```

```rust
// crates/refactor-radar-mcp/src/load_scip_project_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LoadScipProjectParams {
    pub path: String,
    pub format: Option<String>,
}
```

```rust
// crates/refactor-radar-mcp/src/load_scip_project_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct LoadScipProjectResult {
    pub project_id: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

```rust
// crates/refactor-radar-mcp/src/generate_rust_scip_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GenerateRustScipParams {
    pub path: String,
    pub output_path: String,
    pub rust_analyzer_path: Option<String>,
    pub config_path: Option<String>,
    pub exclude_vendored_libraries: Option<bool>,
    pub num_threads: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/generate_rust_scip_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct GenerateRustScipResult {
    pub path: String,
    pub stderr_digest: String,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
    pub file_size: u64,
}
```

```rust
// crates/refactor-radar-mcp/src/scan_project_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ScanProjectParams {
    pub path: String,
    pub output_path: Option<String>,
    pub rust_analyzer_path: Option<String>,
    pub config_path: Option<String>,
}
```

```rust
// crates/refactor-radar-mcp/src/scan_project_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ScanProjectResult {
    pub project_id: String,
    pub output_path: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

```rust
// crates/refactor-radar-mcp/src/element_query_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ElementQueryParams {
    pub project_id: String,
    pub kind: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/find_references_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FindReferencesParams {
    pub symbol_id: String,
    pub limit: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/find_related_symbols_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FindRelatedSymbolsParams {
    pub symbol_id: String,
    pub limit: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/generate_llm_brief_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GenerateLlmBriefParams {
    pub project_id: String,
    pub budget_tokens: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/get_5w_summary_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Get5wSummaryParams {
    pub symbol_id: String,
    pub reference_limit: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/get_element_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetElementParams {
    pub symbol_id: String,
}
```

```rust
// crates/refactor-radar-mcp/src/get_element_brief_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetElementBriefParams {
    pub symbol_id: String,
    pub include_inferred: Option<bool>,
    pub reference_limit: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/get_function_signature_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetFunctionSignatureParams {
    pub symbol_id: String,
}
```

```rust
// crates/refactor-radar-mcp/src/get_project_summary_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetProjectSummaryParams {
    pub project_id: String,
}
```

```rust
// crates/refactor-radar-mcp/src/get_source_span_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanParams {
    pub span_id: String,
}
```

```rust
// crates/refactor-radar-mcp/src/scan_cache.rs
use refactor_radar_core::SemanticModel;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct ScanCache {
    pub models: HashMap<String, SemanticModel>,
    pub scip_paths: HashMap<String, PathBuf>,
    pub temporary_scip_paths: HashMap<String, tempfile::TempPath>,
    pub producer_metadata: HashMap<String, String>,
    pub generation_diagnostics: HashMap<String, String>,
}

impl ScanCache {
    pub fn insert(
        &mut self,
        model: SemanticModel,
        scip_path: PathBuf,
        diagnostics: Option<String>,
        temporary_scip_path: Option<tempfile::TempPath>,
    ) {
        let project_id = model.project.project_id.clone();
        if let Some(producer_name) = &model.project.producer_name {
            self.producer_metadata
                .insert(project_id.clone(), producer_name.clone());
        }
        if let Some(diagnostics) = diagnostics {
            self.generation_diagnostics
                .insert(project_id.clone(), diagnostics);
        }
        match temporary_scip_path {
            Some(temporary_scip_path) => {
                self.temporary_scip_paths
                    .insert(project_id.clone(), temporary_scip_path);
            }
            None => {
                self.temporary_scip_paths.remove(&project_id);
            }
        }
        self.scip_paths.insert(project_id.clone(), scip_path);
        self.models.insert(project_id, model);
    }
}
```

```rust
// crates/refactor-radar-mcp/src/mcp_server_error.rs
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("{message} at {location}")]
    MissingProject {
        message: &'static str,
        project_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingElement {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingSymbol {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    MissingSpan {
        message: &'static str,
        span_id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    UnsupportedFormat {
        message: &'static str,
        format: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    AmbiguousSymbol {
        message: &'static str,
        symbol_id: String,
        project_ids: Vec<String>,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    SerializationFailed {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    GeneratedScipMetadataReadFailed {
        message: &'static str,
        path: String,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    TemporaryScipOutputCreationFailed {
        message: &'static str,
        path: String,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ReportGenerationFailed {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ServerRuntime {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
}

impl McpServerError {
    #[track_caller]
    pub fn missing_project(project_id: impl Into<String>) -> Self {
        Self::MissingProject {
            message: "requested RefactorRadar project was not found",
            project_id: project_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_element(symbol_id: impl Into<String>) -> Self {
        Self::MissingElement {
            message: "requested RefactorRadar element was not found",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_symbol(symbol_id: impl Into<String>) -> Self {
        Self::MissingSymbol {
            message: "requested RefactorRadar symbol was not found",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn missing_span(span_id: impl Into<String>) -> Self {
        Self::MissingSpan {
            message: "requested RefactorRadar source span was not found",
            span_id: span_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            message: "unsupported SCIP format",
            format: format.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn ambiguous_symbol(symbol_id: impl Into<String>, project_ids: Vec<String>) -> Self {
        Self::AmbiguousSymbol {
            message: "symbol id matched more than one cached project",
            symbol_id: symbol_id.into(),
            project_ids,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn serialization_failed(details: impl Into<String>) -> Self {
        Self::SerializationFailed {
            message: "failed to serialize MCP tool result",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn generated_scip_metadata_read_failed(
        path: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::GeneratedScipMetadataReadFailed {
            message: "failed to read generated SCIP file metadata",
            path: path.into(),
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn temporary_scip_output_creation_failed(
        path: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::TemporaryScipOutputCreationFailed {
            message: "failed to create temporary SCIP output file",
            path: path.into(),
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn report_generation_failed(details: impl Into<String>) -> Self {
        Self::ReportGenerationFailed {
            message: "RefactorRadar report generation failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn server_runtime(details: impl Into<String>) -> Self {
        Self::ServerRuntime {
            message: "RefactorRadar MCP server failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::MissingProject { message, .. }
            | Self::MissingElement { message, .. }
            | Self::MissingSymbol { message, .. }
            | Self::MissingSpan { message, .. }
            | Self::UnsupportedFormat { message, .. }
            | Self::AmbiguousSymbol { message, .. }
            | Self::SerializationFailed { message, .. }
            | Self::GeneratedScipMetadataReadFailed { message, .. }
            | Self::TemporaryScipOutputCreationFailed { message, .. }
            | Self::ReportGenerationFailed { message, .. }
            | Self::ServerRuntime { message, .. } => message,
        }
    }
}
```

```rust
// crates/refactor-radar-mcp/src/mcp_server_result.rs
use crate::mcp_server_error::McpServerError;

pub type McpServerResult<T> = std::result::Result<T, McpServerError>;
```

```rust
// crates/refactor-radar-mcp/src/mcp_error_conversion.rs
use crate::McpServerError;
use refactor_radar_scip::ScipError;
use rmcp::ErrorData as McpError;
use serde_json::json;

pub fn to_mcp_error(error: McpServerError) -> McpError {
    match error {
        McpServerError::MissingProject {
            message,
            project_id,
            ..
        } => McpError::resource_not_found(message, Some(json!({ "project_id": project_id }))),
        McpServerError::MissingElement {
            message,
            symbol_id,
            ..
        }
        | McpServerError::MissingSymbol {
            message,
            symbol_id,
            ..
        } => McpError::resource_not_found(message, Some(json!({ "symbol_id": symbol_id }))),
        McpServerError::MissingSpan {
            message, span_id, ..
        } => McpError::resource_not_found(message, Some(json!({ "span_id": span_id }))),
        McpServerError::UnsupportedFormat {
            message, format, ..
        } => McpError::invalid_params(message, Some(json!({ "format": format }))),
        McpServerError::AmbiguousSymbol {
            message,
            symbol_id,
            project_ids,
            ..
        } => McpError::invalid_params(
            message,
            Some(json!({ "symbol_id": symbol_id, "project_ids": project_ids })),
        ),
        McpServerError::SerializationFailed {
            message, details, ..
        }
        | McpServerError::ReportGenerationFailed {
            message, details, ..
        }
        | McpServerError::ServerRuntime {
            message, details, ..
        } => {
            McpError::internal_error(message, Some(json!({ "details": details })))
        }
        McpServerError::GeneratedScipMetadataReadFailed {
            message,
            path,
            details,
            ..
        }
        | McpServerError::TemporaryScipOutputCreationFailed {
            message,
            path,
            details,
            ..
        } => {
            McpError::internal_error(message, Some(json!({ "path": path, "details": details })))
        }
    }
}

impl From<McpServerError> for McpError {
    fn from(error: McpServerError) -> Self {
        to_mcp_error(error)
    }
}

pub fn scip_to_mcp_error(error: ScipError) -> McpError {
    match error {
        ScipError::UnknownFormat { message, path, .. } => {
            McpError::invalid_params(message, Some(json!({ "path": path })))
        }
        ScipError::RustAnalyzerMissing {
            message,
            executable,
            ..
        } => McpError::internal_error(message, Some(json!({ "executable": executable }))),
        ScipError::RustAnalyzerLaunchFailed {
            message,
            executable,
            details,
            ..
        } => McpError::internal_error(
            message,
            Some(json!({ "executable": executable, "details": details })),
        ),
        ScipError::RustAnalyzerFailed {
            message,
            status,
            stderr,
            ..
        } => McpError::internal_error(
            message,
            Some(json!({ "status": status, "stderr": stderr })),
        ),
        ScipError::ReadFailed { message, path, .. }
        | ScipError::ParseFailed { message, path, .. } => {
            McpError::internal_error(message, Some(json!({ "path": path })))
        }
        ScipError::CoreProjectionFailed { message, .. } => {
            McpError::internal_error(message, None)
        }
    }
}
```

```rust
// crates/refactor-radar-mcp/src/refactor_radar_mcp_server.rs
use crate::{
    element_query_params::ElementQueryParams,
    find_references_params::FindReferencesParams,
    find_related_symbols_params::FindRelatedSymbolsParams,
    generate_llm_brief_params::GenerateLlmBriefParams,
    generate_rust_scip_params::GenerateRustScipParams,
    generate_rust_scip_result::GenerateRustScipResult,
    get_5w_summary_params::Get5wSummaryParams,
    get_element_brief_params::GetElementBriefParams,
    get_element_params::GetElementParams,
    get_function_signature_params::GetFunctionSignatureParams,
    get_project_summary_params::GetProjectSummaryParams,
    get_source_span_params::GetSourceSpanParams,
    load_scip_project_params::LoadScipProjectParams,
    load_scip_project_result::LoadScipProjectResult,
    mcp_error_conversion::scip_to_mcp_error,
    mcp_server_error::McpServerError,
    scan_cache::ScanCache,
    scan_project_params::ScanProjectParams,
    scan_project_result::ScanProjectResult,
};
use refactor_radar_report::{
    build_element_brief, build_project_summary, generate_llm_brief as render_llm_brief,
};
use refactor_radar_core::{ElementSummary, SemanticModel};
use refactor_radar_scip::{
    generate_rust_scip as run_rust_analyzer_scip, project_scip_index, Format,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::{Json, Parameters}},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RefactorRadarMcpServer {
    cache: Arc<RwLock<ScanCache>>,
    tool_router: ToolRouter<Self>,
}

impl RefactorRadarMcpServer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(ScanCache::default())),
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for RefactorRadarMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RefactorRadarMcpServer {
    #[tool(name = "load_scip_project", description = "Load a SCIP index into the RefactorRadar cache")]
    async fn load_scip_project(
        &self,
        Parameters(params): Parameters<LoadScipProjectParams>,
    ) -> Result<Json<LoadScipProjectResult>, McpError> {
        let path = PathBuf::from(&params.path);
        let format = match params.format.as_deref() {
            Some("scip") => Some(Format::Binary),
            Some("json") => Some(Format::Json),
            Some(other) => {
                return Err(McpServerError::unsupported_format(other).into());
            }
            None => None,
        };
        let model = project_scip_index(&path, format).map_err(scip_to_mcp_error)?;
        let result = LoadScipProjectResult {
            project_id: model.project.project_id.clone(),
            document_count: model.files.len(),
            symbol_count: model.elements.len(),
            occurrence_count: occurrence_count(&model),
        };
        self.cache.write().await.insert(model, path, None, None);
        Ok(Json(result))
    }

    #[tool(name = "generate_rust_scip", description = "Generate a rust-analyzer SCIP file")]
    async fn generate_rust_scip(
        &self,
        Parameters(params): Parameters<GenerateRustScipParams>,
    ) -> Result<Json<GenerateRustScipResult>, McpError> {
        let output_path = PathBuf::from(&params.output_path);
        let rust_analyzer_path = params.rust_analyzer_path.as_deref().map(PathBuf::from);
        let config_path = params.config_path.as_deref().map(PathBuf::from);
        let stderr = run_rust_analyzer_scip(
            &PathBuf::from(&params.path),
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            params.exclude_vendored_libraries.unwrap_or(false),
            params.num_threads,
        )
        .await
        .map_err(scip_to_mcp_error)?;
        let file_size = std::fs::metadata(&output_path)
            .map(|metadata| metadata.len())
            .map_err(|error| {
                McpServerError::generated_scip_metadata_read_failed(
                    output_path.display().to_string(),
                    error.to_string(),
                )
            })?;
        let (producer_name, producer_version) =
            project_scip_index(&output_path, Some(Format::Binary))
                .ok()
                .map(|model| (model.project.producer_name, model.project.producer_version))
                .unwrap_or((None, None));
        Ok(Json(GenerateRustScipResult {
            path: output_path.display().to_string(),
            stderr_digest: stderr.chars().take(500).collect(),
            producer_name,
            producer_version,
            file_size,
        }))
    }

    #[tool(name = "scan_project", description = "Generate SCIP and load it into the cache")]
    async fn scan_project(
        &self,
        Parameters(params): Parameters<ScanProjectParams>,
    ) -> Result<Json<ScanProjectResult>, McpError> {
        let (output_path, temporary_scip_path) = match &params.output_path {
            Some(output_path) => (PathBuf::from(output_path), None),
            None => {
                let temporary_directory = std::env::temp_dir();
                let temp_path = tempfile::Builder::new()
                    .suffix(".scip")
                    .tempfile_in(&temporary_directory)
                    .map_err(|error| {
                        McpServerError::temporary_scip_output_creation_failed(
                            temporary_directory.display().to_string(),
                            error.to_string(),
                        )
                    })?
                    .into_temp_path();
                let output_path = temp_path.to_path_buf();
                (output_path, Some(temp_path))
            }
        };
        let rust_analyzer_path = params.rust_analyzer_path.as_deref().map(PathBuf::from);
        let config_path = params.config_path.as_deref().map(PathBuf::from);
        let stderr = run_rust_analyzer_scip(
            &PathBuf::from(&params.path),
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            false,
            None,
        )
        .await
        .map_err(scip_to_mcp_error)?;
        let model =
            project_scip_index(&output_path, Some(Format::Binary)).map_err(scip_to_mcp_error)?;
        let result = ScanProjectResult {
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

    #[tool(name = "get_project_summary", description = "Return one cached project summary")]
    async fn get_project_summary(
        &self,
        Parameters(params): Parameters<GetProjectSummaryParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::missing_project(params.project_id.clone()))?;
        structured(build_project_summary(model))
    }

    #[tool(name = "list_elements", description = "List cached semantic elements")]
    async fn list_elements(
        &self,
        Parameters(params): Parameters<ElementQueryParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::missing_project(params.project_id.clone()))?;
        let limit = params.limit.unwrap_or(100);
        let elements: Vec<_> = model
            .elements
            .iter()
            .filter(|element| {
                params
                    .kind
                    .as_ref()
                    .map(|kind| normalize_kind(&format!("{:?}", element.kind)) == normalize_kind(kind))
                    .unwrap_or(true)
            })
            .filter(|element| {
                params.path.as_ref().map(|path| {
                    element
                        .definition_span
                        .as_ref()
                        .map(|span| span.document_path == *path)
                        .unwrap_or(false)
                }).unwrap_or(true)
            })
            .take(limit)
            .collect();
        structured(elements)
    }

    #[tool(name = "get_element", description = "Return one element by stable ID or SCIP symbol")]
    async fn get_element(
        &self,
        Parameters(params): Parameters<GetElementParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let (_model, element) = resolve_element(&cache, &params.symbol_id)?;
        structured(element)
    }

    #[tool(name = "get_function_signature", description = "Return function signature facts")]
    async fn get_function_signature(
        &self,
        Parameters(params): Parameters<GetFunctionSignatureParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let (_model, element) = resolve_element(&cache, &params.symbol_id)?;
        structured(serde_json::json!({
            "stable_id": element.stable_id.as_str(),
            "symbol_id": element.symbol_id.as_str(),
            "definition_span_id": element.definition_span.as_ref().map(|span| span.id.as_str()),
            "signature": &element.signature,
        }))
    }

    #[tool(name = "get_element_brief", description = "Return a 5W element brief")]
    async fn get_element_brief(
        &self,
        Parameters(params): Parameters<GetElementBriefParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let (model, element) = resolve_element(&cache, &params.symbol_id)?;
        let brief = build_element_brief(
            model,
            element.symbol_id.as_str(),
            params.include_inferred.unwrap_or(true),
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpServerError::report_generation_failed(error.message()))?;
        structured(brief)
    }

    #[tool(name = "get_5w_summary", description = "Return the 5W summary and follow-up IDs for an element")]
    async fn get_5w_summary(
        &self,
        Parameters(params): Parameters<Get5wSummaryParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let (model, element) = resolve_element(&cache, &params.symbol_id)?;
        let brief = build_element_brief(
            model,
            element.symbol_id.as_str(),
            true,
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpServerError::report_generation_failed(error.message()))?;
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
    }

    #[tool(name = "find_references", description = "Return span metadata for symbol references")]
    async fn find_references(
        &self,
        Parameters(params): Parameters<FindReferencesParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let (model, symbol_id) = resolve_symbol_id(&cache, &params.symbol_id)?;
        let references: Vec<_> = model
            .references
            .iter()
            .filter(|reference| reference.referenced_symbol_id.as_str() == symbol_id.as_str())
            .take(limit)
            .collect();
        structured(references)
    }

    #[tool(name = "find_related_symbols", description = "Return child symbols and SCIP-derived function-like relations")]
    async fn find_related_symbols(
        &self,
        Parameters(params): Parameters<FindRelatedSymbolsParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let (model, symbol_id) = resolve_symbol_id(&cache, &params.symbol_id)?;
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
    }

    #[tool(name = "get_source_span", description = "Return source span metadata without raw source")]
    async fn get_source_span(
        &self,
        Parameters(params): Parameters<GetSourceSpanParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let span = cache
            .models
            .values()
            .flat_map(|model| model.spans.iter())
            .find(|span| span.id.as_str() == params.span_id.as_str())
            .ok_or_else(|| McpServerError::missing_span(params.span_id.clone()))?;
        structured(span)
    }

    #[tool(name = "generate_llm_brief", description = "Return a compact Markdown project brief")]
    async fn generate_llm_brief(
        &self,
        Parameters(params): Parameters<GenerateLlmBriefParams>,
    ) -> Result<Json<serde_json::Value>, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::missing_project(params.project_id.clone()))?;
        let brief = render_llm_brief(model, params.budget_tokens)
            .map_err(|error| McpServerError::report_generation_failed(error.message()))?;
        structured(serde_json::json!({ "markdown": brief }))
    }
}

fn structured(value: impl Serialize) -> Result<Json<serde_json::Value>, McpError> {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|error| McpServerError::serialization_failed(error.to_string()).into())
}

fn occurrence_count(model: &SemanticModel) -> usize {
    model.files.iter().map(|file| file.occurrence_count).sum()
}

fn resolve_element<'a>(
    cache: &'a ScanCache,
    requested_id: &str,
) -> Result<(&'a SemanticModel, &'a ElementSummary), McpError> {
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
        [] => Err(McpServerError::missing_element(requested_id.to_owned()).into()),
        [(model, element)] => Ok((*model, *element)),
        _ => Err(ambiguous_symbol_error(
            requested_id,
            matches
                .iter()
                .map(|(model, _element)| model.project.project_id.as_str())
                .collect(),
        )),
    }
}

fn resolve_symbol_id<'a>(
    cache: &'a ScanCache,
    requested_id: &str,
) -> Result<(&'a SemanticModel, String), McpError> {
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
        [] => Err(McpServerError::missing_symbol(requested_id.to_owned()).into()),
        [(model, symbol_id)] => Ok((*model, symbol_id.clone())),
        _ => {
            let mut project_ids: Vec<_> = matches
                .iter()
                .map(|(model, _symbol_id)| model.project.project_id.as_str())
                .collect();
            project_ids.sort_unstable();
            project_ids.dedup();
            Err(ambiguous_symbol_error(requested_id, project_ids))
        }
    }
}

fn ambiguous_symbol_error(requested_id: &str, project_ids: Vec<&str>) -> McpError {
    let project_ids = project_ids.into_iter().map(str::to_owned).collect();
    McpServerError::ambiguous_symbol(requested_id, project_ids).into()
}

fn normalize_kind(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RefactorRadarMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("SCIP-backed RefactorRadar semantic query server")
    }
}
```

```rust
// crates/refactor-radar-mcp/src/bin/refactor-radar-mcp.rs
use refactor_radar_mcp::{McpServerError, McpServerResult, RefactorRadarMcpServer};
use rmcp::{transport::stdio, ServiceExt};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run refactor-radar MCP server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> McpServerResult<()> {
    let service = RefactorRadarMcpServer::new()
        .serve(stdio())
        .await
        .map_err(|error| McpServerError::server_runtime(error.to_string()))?;
    service
        .waiting()
        .await
        .map_err(|error| McpServerError::server_runtime(error.to_string()))?;
    Ok(())
}
```

## Tests

Place MCP unit tests in `refactor_radar_mcp_server.rs` under `#[cfg(test)]`.
Use given/when/then test names and in-memory SCIP fixtures.

Verify:

- `load_scip_project` caches a projected model.
- Missing project summary returns an MCP error.
- `generate_rust_scip` works with a fake rust-analyzer executable and returns
  output path, stderr digest, producer metadata, and file size.
- `scan_project` works with a fake rust-analyzer executable and caches the
  project when `output_path` is supplied and when it is omitted.
- The omitted-output `scan_project` test verifies the returned temporary SCIP
  path still exists and can be loaded after the handler returns.
- Fake rust-analyzer nonzero stderr becomes structured MCP error data.
- `get_project_summary` returns structured JSON.
- `list_elements` supports kind, path, and limit filters.
- `get_element` accepts stable IDs and SCIP IDs.
- `get_function_signature` returns signature facts.
- `get_element_brief` and `get_5w_summary` return five_w content and evidence.
- `find_references` accepts stable IDs, SCIP IDs, and external raw SCIP IDs.
- Symbol-only queries return invalid params instead of choosing an arbitrary
  project when the supplied symbol ID matches more than one cached project.
- Stable-ID `find_references` and `find_related_symbols` results are scoped to
  the matched cached project after symbol resolution.
- `find_related_symbols` returns function-like relationships.
- `get_source_span` returns span metadata only.
- `generate_llm_brief` returns Markdown in structured JSON.

Do not require a real rust-analyzer binary for these tests.

Concrete starting test module:

```rust
// crates/refactor-radar-mcp/src/refactor_radar_mcp_server.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        element_query_params::ElementQueryParams,
        find_references_params::FindReferencesParams,
        find_related_symbols_params::FindRelatedSymbolsParams,
        generate_llm_brief_params::GenerateLlmBriefParams,
        generate_rust_scip_params::GenerateRustScipParams,
        get_5w_summary_params::Get5wSummaryParams,
        get_element_brief_params::GetElementBriefParams,
        get_element_params::GetElementParams,
        get_function_signature_params::GetFunctionSignatureParams,
        get_project_summary_params::GetProjectSummaryParams,
        get_source_span_params::GetSourceSpanParams,
        load_scip_project_params::LoadScipProjectParams,
        scan_project_params::ScanProjectParams,
    };
    use protobuf::{Message, MessageField};
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use scip::types::{
        symbol_information, Document, Index, Metadata, Occurrence, Signature, SymbolInformation,
        SymbolRole, ToolInfo,
    };
    use tempfile::tempdir;

    const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
    const PRIVATE_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
    const UNIQUE_SYMBOL: &str = "rust-analyzer cargo unique_crate 0.1.0 unique_crate/public_sum().";
    const UNIQUE_PRIVATE_SYMBOL: &str =
        "rust-analyzer cargo unique_crate 0.1.0 unique_crate/private_offset().";
    const EXTERNAL_SYMBOL: &str = "rust-analyzer cargo std 0.0.0 std/option/Option#";

    #[tokio::test]
    async fn given_scip_file_when_loading_project_then_cache_contains_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let (_directory, path) = write_fixture_scip()?;

        let Json(result) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
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
    async fn given_missing_project_when_getting_summary_then_mcp_error_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();

        let error = server
            .get_project_summary(Parameters(GetProjectSummaryParams {
                project_id: "missing".to_owned(),
            }))
            .await
            .err();

        assert!(error.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn given_fake_rust_analyzer_when_generating_scip_then_result_metadata_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let fixture_path = directory.path().join("fixture.scip");
            std::fs::write(&fixture_path, sample_index().write_to_bytes()?)?;
            let output_path = directory.path().join("generated.scip");
            let script_path = directory.path().join("ra-generate");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho generated >&2\ncat \"$2/fixture.scip\" > \"$4\"\nexit 0\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let Json(result) = server
                .generate_rust_scip(Parameters(GenerateRustScipParams {
                    path: directory.path().display().to_string(),
                    output_path: output_path.display().to_string(),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                    exclude_vendored_libraries: Some(false),
                    num_threads: None,
                }))
                .await
                .map_err(to_io_error)?;

            assert_eq!(output_path.display().to_string(), result.path);
            assert!(result.stderr_digest.contains("generated"));
            assert!(result.file_size > 0);
            assert_eq!(Some("rust-analyzer"), result.producer_name.as_deref());
            assert_eq!(Some("test"), result.producer_version.as_deref());
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_fake_rust_analyzer_when_scanning_project_then_cache_contains_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let fixture_path = directory.path().join("fixture.scip");
            std::fs::write(&fixture_path, sample_index().write_to_bytes()?)?;
            let output_path = directory.path().join("generated.scip");
            let script_path = directory.path().join("ra-success");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho scanned >&2\ncat \"$2/fixture.scip\" > \"$4\"\nexit 0\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let Json(result) = server
                .scan_project(Parameters(ScanProjectParams {
                    path: directory.path().display().to_string(),
                    output_path: Some(output_path.display().to_string()),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                }))
                .await
                .map_err(to_io_error)?;

            assert_eq!("fixture", result.project_id);
            assert!(output_path.exists());
            assert!(server.cache.read().await.models.contains_key("fixture"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_fake_rust_analyzer_when_scanning_without_output_path_then_temporary_scip_remains_loadable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let fixture_path = directory.path().join("fixture.scip");
            std::fs::write(&fixture_path, sample_index().write_to_bytes()?)?;
            let script_path = directory.path().join("ra-temp-success");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho scanned-temp >&2\ncat \"$2/fixture.scip\" > \"$4\"\nexit 0\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let Json(result) = server
                .scan_project(Parameters(ScanProjectParams {
                    path: directory.path().display().to_string(),
                    output_path: None,
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                }))
                .await
                .map_err(to_io_error)?;
            let returned_path = std::path::PathBuf::from(&result.output_path);

            assert_eq!("fixture", result.project_id);
            assert!(returned_path.exists());

            let reloaded_model = refactor_radar_scip::project_scip_index(
                &returned_path,
                Some(refactor_radar_scip::Format::Binary),
            )?;

            assert_eq!("fixture", reloaded_model.project.project_id);
            assert!(server.cache.read().await.models.contains_key("fixture"));
            assert!(server
                .cache
                .read()
                .await
                .temporary_scip_paths
                .contains_key("fixture"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_failing_rust_analyzer_when_scanning_project_then_stderr_is_structured_error_data(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let output_path = directory.path().join("failed.scip");
            let script_path = directory.path().join("ra-fails");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho scan failed >&2\nexit 7\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let error = server
                .scan_project(Parameters(ScanProjectParams {
                    path: directory.path().display().to_string(),
                    output_path: Some(output_path.display().to_string()),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                }))
                .await
                .err();
            let data = error.and_then(|error| error.data);
            let stderr = data
                .as_ref()
                .and_then(|value| value.get("stderr"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();

            assert!(stderr.contains("scan failed"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_summary_then_structured_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_project_summary(Parameters(GetProjectSummaryParams {
                project_id: "fixture".to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some("fixture"), value.get("project_id").and_then(|value| value.as_str()));
        assert_eq!(Some(1), value.get("document_count").and_then(|value| value.as_u64()));
        assert_eq!(Some(2), value.get("element_count").and_then(|value| value.as_u64()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_listing_elements_then_kind_path_and_limit_filters_work(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .list_elements(Parameters(ElementQueryParams {
                project_id: "fixture".to_owned(),
                kind: Some("function".to_owned()),
                path: Some("src/lib.rs".to_owned()),
                limit: Some(1),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some(1), value.as_array().map(|items| items.len()));
        assert_eq!(
            Some("public_sum"),
            value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item.get("display_name"))
                .and_then(|value| value.as_str())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_element_then_stable_id_or_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;
        let stable_id = stable_id_for_symbol(&server, SYMBOL).await?;

        let by_scip = server
            .get_element(Parameters(GetElementParams {
                symbol_id: SYMBOL.to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .get_element(Parameters(GetElementParams {
                symbol_id: stable_id,
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(SYMBOL), by_scip.get("symbol_id").and_then(|value| value.as_str()));
        assert_eq!(Some(SYMBOL), by_stable.get("symbol_id").and_then(|value| value.as_str()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_function_when_getting_signature_then_signature_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_function_signature(Parameters(GetFunctionSignatureParams {
                symbol_id: SYMBOL.to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some(SYMBOL), value.get("symbol_id").and_then(|value| value.as_str()));
        assert_eq!(
            Some("fn public_sum(left: i32, right: i32) -> i32"),
            value
                .get("signature")
                .and_then(|signature| signature.get("signature_text"))
                .and_then(|value| value.as_str())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_function_when_getting_element_brief_then_five_w_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let brief = server
            .get_element_brief(Parameters(GetElementBriefParams {
                symbol_id: SYMBOL.to_owned(),
                include_inferred: Some(true),
                reference_limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let five_w = server
            .get_5w_summary(Parameters(Get5wSummaryParams {
                symbol_id: SYMBOL.to_owned(),
                reference_limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let brief = require_structured(brief)?;
        let five_w = require_structured(five_w)?;

        assert!(brief.get("five_w").and_then(|value| value.get("what")).is_some());
        assert!(brief.get("evidence").is_some());
        assert!(five_w.get("five_w").and_then(|value| value.get("what")).is_some());
        assert!(five_w.get("evidence").and_then(|value| value.as_array()).is_some());
        assert!(five_w.get("confirmed").and_then(|value| value.as_array()).is_some());
        assert!(five_w.get("inferred").and_then(|value| value.as_array()).is_some());
        assert!(five_w.get("unknown").and_then(|value| value.as_array()).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_references_then_stable_id_or_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;
        let stable_id = stable_id_for_symbol(&server, PRIVATE_SYMBOL).await?;

        let by_scip = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: PRIVATE_SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: stable_id,
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(1), by_scip.as_array().map(|items| items.len()));
        assert_eq!(Some(1), by_stable.as_array().map(|items| items.len()));
        Ok(())
    }

    #[tokio::test]
    async fn given_symbol_matches_multiple_projects_when_querying_then_invalid_params_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_two_project_server().await?;

        let error = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: PRIVATE_SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("missing ambiguity error"))?;
        let data = error
            .data
            .ok_or_else(|| std::io::Error::other("missing ambiguity error data"))?;
        let project_ids = data
            .get("project_ids")
            .and_then(|value| value.as_array())
            .ok_or_else(|| std::io::Error::other("missing project_ids"))?;

        assert_eq!(
            Some(PRIVATE_SYMBOL),
            data.get("symbol_id").and_then(|value| value.as_str())
        );
        assert!(project_ids
            .iter()
            .any(|value| value.as_str() == Some("fixture-one")));
        assert!(project_ids
            .iter()
            .any(|value| value.as_str() == Some("fixture-two")));
        Ok(())
    }

    #[tokio::test]
    async fn given_stable_id_with_multiple_cached_projects_when_querying_then_results_stay_project_scoped(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_unique_project_pair_server().await?;
        let private_stable_id =
            stable_id_for_project_symbol(&server, "fixture-one", UNIQUE_PRIVATE_SYMBOL).await?;
        let public_stable_id =
            stable_id_for_project_symbol(&server, "fixture-one", UNIQUE_SYMBOL).await?;

        let references = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: private_stable_id,
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let related = server
            .find_related_symbols(Parameters(FindRelatedSymbolsParams {
                symbol_id: public_stable_id,
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let references = require_structured(references)?;
        let related = require_structured(related)?;

        assert_eq!(Some(1), references.as_array().map(|items| items.len()));
        assert_eq!(
            Some(1),
            related
                .get("outgoing_function_like_references")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_external_references_then_raw_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;
        let external_span = refactor_radar_core::SourceSpan::from_scip_range(
            "fixture",
            "src/lib.rs",
            &[8, 2, 8],
        )?;
        {
            let mut cache = server.cache.write().await;
            let model = cache
                .models
                .get_mut("fixture")
                .ok_or_else(|| std::io::Error::other("missing fixture model"))?;
            model.references.push(refactor_radar_core::SymbolReferenceSummary {
                referenced_symbol_id: refactor_radar_core::SymbolId::new(EXTERNAL_SYMBOL),
                source_span: external_span.clone(),
                document_path: "src/lib.rs".to_owned(),
                symbol_roles: 0,
            });
            model.spans.push(external_span);
        }

        let by_scip = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: EXTERNAL_SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: format!("scip:{EXTERNAL_SYMBOL}"),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(1), by_scip.as_array().map(|items| items.len()));
        assert_eq!(Some(1), by_stable.as_array().map(|items| items.len()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_related_symbols_then_function_like_relations_are_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .find_related_symbols(Parameters(FindRelatedSymbolsParams {
                symbol_id: SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(
            Some(1),
            value
                .get("outgoing_function_like_references")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_source_span_then_span_metadata_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_source_span(Parameters(GetSourceSpanParams {
                span_id: "fixture::src/lib.rs:1:1".to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some("src/lib.rs"), value.get("document_path").and_then(|value| value.as_str()));
        assert_eq!(Some(1), value.get("start_line").and_then(|value| value.as_u64()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_generating_llm_brief_then_markdown_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .generate_llm_brief(Parameters(GenerateLlmBriefParams {
                project_id: "fixture".to_owned(),
                budget_tokens: Some(100),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;
        let markdown = value
            .get("markdown")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        assert!(markdown.contains("# Project fixture"));
        Ok(())
    }

    async fn load_sample_server() -> Result<RefactorRadarMcpServer, Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let (_directory, path) = write_fixture_scip()?;
        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        Ok(server)
    }

    async fn load_two_project_server(
    ) -> Result<RefactorRadarMcpServer, Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let directory = tempdir()?;
        let first_path = directory.path().join("fixture-one.scip");
        let second_path = directory.path().join("fixture-two.scip");
        std::fs::write(
            &first_path,
            sample_index_for_project("fixture-one").write_to_bytes()?,
        )?;
        std::fs::write(
            &second_path,
            sample_index_for_project("fixture-two").write_to_bytes()?,
        )?;

        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: first_path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: second_path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        Ok(server)
    }

    async fn load_unique_project_pair_server(
    ) -> Result<RefactorRadarMcpServer, Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let directory = tempdir()?;
        let first_path = directory.path().join("fixture-one-unique.scip");
        let second_path = directory.path().join("fixture-two.scip");
        std::fs::write(
            &first_path,
            sample_index_for_project_symbols("fixture-one", UNIQUE_SYMBOL, UNIQUE_PRIVATE_SYMBOL)
                .write_to_bytes()?,
        )?;
        std::fs::write(
            &second_path,
            sample_index_for_project("fixture-two").write_to_bytes()?,
        )?;

        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: first_path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: second_path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        Ok(server)
    }

    fn write_fixture_scip(
    ) -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("fixture.scip");
        std::fs::write(&path, sample_index().write_to_bytes()?)?;
        Ok((directory, path))
    }

    async fn stable_id_for_symbol(
        server: &RefactorRadarMcpServer,
        symbol: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        stable_id_for_project_symbol(server, "fixture", symbol).await
    }

    async fn stable_id_for_project_symbol(
        server: &RefactorRadarMcpServer,
        project_id: &str,
        symbol: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let cache = server.cache.read().await;
        let model = cache
            .models
            .get(project_id)
            .ok_or_else(|| std::io::Error::other("missing fixture model"))?;
        let element = model
            .elements
            .iter()
            .find(|element| element.symbol_id.as_str() == symbol)
            .ok_or_else(|| std::io::Error::other("missing fixture element"))?;
        Ok(element.stable_id.as_str().to_owned())
    }

    fn sample_index() -> Index {
        sample_index_for_project("fixture")
    }

    fn sample_index_for_project(project_root: &str) -> Index {
        sample_index_for_project_symbols(project_root, SYMBOL, PRIVATE_SYMBOL)
    }

    fn sample_index_for_project_symbols(
        project_root: &str,
        public_symbol: &str,
        private_symbol: &str,
    ) -> Index {
        Index {
            metadata: MessageField::some(Metadata {
                project_root: project_root.to_owned(),
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
                        symbol: public_symbol.to_owned(),
                        kind: symbol_information::Kind::Function.into(),
                        display_name: "public_sum".to_owned(),
                        signature_documentation: MessageField::some(Signature {
                            language: "rust".to_owned(),
                            text: "fn public_sum(left: i32, right: i32) -> i32".to_owned(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    SymbolInformation {
                        symbol: private_symbol.to_owned(),
                        kind: symbol_information::Kind::Function.into(),
                        display_name: "private_offset".to_owned(),
                        ..Default::default()
                    },
                ],
                occurrences: vec![
                    Occurrence {
                        range: vec![0, 0, 10],
                        symbol: public_symbol.to_owned(),
                        symbol_roles: SymbolRole::Definition as i32,
                        enclosing_range: vec![0, 0, 3, 1],
                        ..Default::default()
                    },
                    Occurrence {
                        range: vec![1, 4, 18],
                        symbol: private_symbol.to_owned(),
                        symbol_roles: 0,
                        ..Default::default()
                    },
                    Occurrence {
                        range: vec![4, 0, 14],
                        symbol: private_symbol.to_owned(),
                        symbol_roles: SymbolRole::Definition as i32,
                        enclosing_range: vec![4, 0, 6, 1],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn require_structured(
        result: Json<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(result.0)
    }

    fn to_io_error(error: rmcp::ErrorData) -> std::io::Error {
        std::io::Error::other(format!("{error:?}"))
    }
}
```

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-mcp
cargo clippy -p refactor-radar-mcp --all-targets -- -D warnings
```

Also verify:

```bash
for file in $(rg --files crates/refactor-radar-mcp/src -g '*.rs'); do
    count=$(rg -n '^(pub(\([^)]*\))? )?(struct|enum|trait|type) ' "$file" | wc -l)
    if [ "$count" -gt 1 ]; then
        echo "$file has $count named type definitions"
        exit 1
    fi
done

if rg -n '\b(print|println)!|std::io::stdout|tokio::io::stdout' crates/refactor-radar-mcp/src; then
    echo "stdout write found in refactor-radar-mcp"
    exit 1
fi

if rg -n '\b(anyhow|unwrap\(|expect\(|panic!|dbg!)' crates/refactor-radar-mcp/src; then
    echo "forbidden error handling or debug macro found in refactor-radar-mcp"
    exit 1
fi
```

## Done Criteria

- `refactor-radar-mcp` exposes all initial structured MCP tools.
- It can load existing SCIP files and scan projects through rust-analyzer.
- Cached queries return compact metadata and IDs without raw source by default.
- MCP stdout remains reserved for JSON-RPC traffic.
- SCIP-derived call/function relationships are labeled conservatively.
- Tests and clippy pass without `anyhow`, `unwrap()`, `expect()`, `panic!`,
  `dbg!`, or `println!` in the MCP crate.
