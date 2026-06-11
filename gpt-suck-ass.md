# Implementation Plan: Self-Bootstrapping Codex Work Plans And Sampling Removal

## Target

`generate_brief_work_plan` must return tasks that a fresh Codex sub-agent can
execute without relying on the parent MCP session's in-memory `ProjectCache`.

The Codex path is deterministic:

```text
parent calls scan_project or load_scip_project
parent calls generate_brief_work_plan
each sub-agent executes the bootstrap tool call first
each sub-agent executes get_element_brief calls
each sub-agent returns JSON matching expected_output_schema
parent merges outputs by merge_key
parent records generated output through rr-mcp cache tools
```

Remove the host-sampling path from `rr-mcp`. Do not keep
`generate_llm_brief`, `sampling/createMessage`, `BriefLlmClient`,
`SamplingBriefLlmClient`, generated-brief model hints, or sampling capability
routing.

Use `.refactor-radar/`, not `.refactor-rador/`. The ADR uses
`.refactor-radar/` for local-first artifacts.

## Non-Negotiable Implementation Constraints

Check the worktree first and do not touch submodules:

```bash
git status --short
git submodule status --recursive
```

The current repo already has a dirty submodule marker:

```text
 m submodules/scip
```

Leave that alone. Do not run `git submodule update`, do not edit files under
`submodules/`, and do not revert the pre-existing submodule state.

Follow `AGENTS.md` error handling:

```rust
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("{message} at {location}")]
    ProjectBootstrapMissing {
        message: &'static str,
        project_id: String,
        location: ErrorLocation,
    },
}

impl McpError {
    #[track_caller]
    pub fn project_bootstrap_missing(project_id: impl Into<String>) -> Self {
        Self::ProjectBootstrapMissing {
            message: "project bootstrap metadata was not found",
            project_id: project_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }
}
```

Keep the existing one-type-per-file convention. Add each new enum or struct in
its own file:

```text
crates/rr-mcp/src/project_bootstrap_kind.rs
crates/rr-mcp/src/project_bootstrap_source.rs
crates/rr-mcp/src/project_cache_manifest.rs
crates/rr-mcp/src/project_cache_paths.rs
crates/rr-mcp/src/brief_task_five_w.rs
crates/rr-mcp/src/brief_task_output_record_params.rs
crates/rr-mcp/src/brief_task_output_record_result.rs
crates/rr-mcp/src/brief_task_output_cache_entry.rs
```

Do not add `anyhow`, provider HTTP clients, implementation `unwrap`,
implementation `expect`, or implementation `panic`.

```bash
! rg -n "anyhow|reqwest|async-openai|OPENAI_API_KEY|unwrap\\(|expect\\(|panic!\\(" crates/rr-mcp/src crates/rr-mcp/tests
```

## Current Repo Facts To Preserve

Use these commands before editing implementation code:

```bash
rg -n "generate_llm_brief|get_brief_generation_capabilities|sampling|createMessage|BriefLlm|GeneratedBrief|BriefCacheKey|LlmBriefParams" crates/rr-mcp/src crates/rr-mcp/tests README.md
rg -n "pub struct .*\\{|pub enum .*\\{|pub trait .*\\{" crates/rr-mcp/src -g '*.rs'
rg -n "std::env::temp_dir|temporary_scip_paths|ProjectCache|BriefTaskToolCall|generate_brief_work_plan" crates/rr-mcp/src
```

The current state includes:

```text
crates/rr-mcp/src/sampling_brief_llm_client.rs
crates/rr-mcp/src/brief_llm_client.rs
crates/rr-mcp/src/brief_llm_request.rs
crates/rr-mcp/src/brief_cache_key.rs
crates/rr-mcp/src/generated_brief.rs
crates/rr-mcp/src/generated_brief_cache_entry.rs
crates/rr-mcp/src/generated_brief_content.rs
crates/rr-mcp/src/generated_five_w.rs
crates/rr-mcp/src/llm_brief_params.rs
crates/rr-mcp/src/brief_generation_capabilities.rs
crates/rr-mcp/tests/generated_brief_protocol.rs
crates/rr-mcp/tests/support/sampling_test_client.rs
```

Those files support the old sampling path and should be removed or replaced.

## Phase 1: Remove Host-Sampling Code And Public Tool Surface

Delete the sampling-only source files:

```bash
git rm crates/rr-mcp/src/sampling_brief_llm_client.rs
git rm crates/rr-mcp/src/brief_llm_client.rs
git rm crates/rr-mcp/src/brief_llm_request.rs
git rm crates/rr-mcp/src/brief_cache_key.rs
git rm crates/rr-mcp/src/generated_brief.rs
git rm crates/rr-mcp/src/generated_brief_cache_entry.rs
git rm crates/rr-mcp/src/generated_brief_content.rs
git rm crates/rr-mcp/src/generated_five_w.rs
git rm crates/rr-mcp/src/llm_brief_params.rs
git rm crates/rr-mcp/src/special/brief_llm_user_prompt.rs
git rm crates/rr-mcp/src/special/mod.rs
git rm crates/rr-mcp/tests/support/sampling_test_client.rs
```

Do not delete `crates/rr-mcp/src/brief_generation_capabilities.rs` yet.
`generate_brief_work_plan` still depends on `BriefGenerationCapabilities` until
Phase 5 removes `generation_capabilities` from `BriefWorkPlan` and removes the
`RequestContext<RoleServer>` work-plan path.

Remove sampling-only protocol tests or replace the file with deterministic
work-plan protocol tests:

```bash
git mv crates/rr-mcp/tests/generated_brief_protocol.rs crates/rr-mcp/tests/brief_work_plan_protocol.rs
```

Update `crates/rr-mcp/tests/support/mod.rs`:

```rust
pub mod fixture_scip;
```

Remove sampling dependencies from `Cargo.toml` when no remaining code uses
them:

```toml
[workspace.dependencies]
# remove if only sampling used them
# async-trait = { version = "0.1.89" }
# futures     = { version = "0.3.32" }
```

Update `crates/rr-mcp/Cargo.toml`:

```toml
[dependencies]
# remove if only sampling used them
# async-trait = { workspace = true }
# futures     = { workspace = true }
```

Remove sampling modules and re-exports from `crates/rr-mcp/src/lib.rs`.
During Phase 1, do not add `lib.rs` declarations for new files that are
created in later phases; add those declarations in the phase that creates the
files.

Remove these from `lib.rs`:

```rust
pub mod brief_cache_key;
pub mod brief_llm_client;
pub mod brief_llm_request;
pub mod generated_brief;
pub mod generated_brief_cache_entry;
pub mod generated_brief_content;
pub mod generated_five_w;
pub mod llm_brief_params;
pub mod sampling_brief_llm_client;
pub mod special;

pub use brief_cache_key::BriefCacheKey;
pub use brief_llm_client::BriefLlmClient;
pub use brief_llm_request::BriefLlmRequest;
pub use generated_brief::GeneratedBrief;
pub use generated_brief_cache_entry::GeneratedBriefCacheEntry;
pub use generated_brief_content::GeneratedBriefContent;
pub use generated_five_w::GeneratedFiveW;
pub use llm_brief_params::LlmBriefParams;
pub use sampling_brief_llm_client::SamplingBriefLlmClient;
pub use special::brief_llm_user_prompt::GENERATED_BRIEF_USER_PROMPT_TEMPLATE;
```

Remove sampling CLI flags from `crates/rr-mcp/src/bin/rr-mcp.rs`:

```rust
#[derive(Parser)]
struct Args {
    #[arg(long)]
    rust_analyzer_path: Option<PathBuf>,
}

async fn run() -> McpResult<()> {
    let args = Args::parse();

    let service = McpServer::with_rust_analyzer_path(args.rust_analyzer_path)
        .serve(stdio())
        .await
        .map_err(|error| McpError::server_runtime(error.to_string()))?;

    service
        .waiting()
        .await
        .map_err(|error| McpError::server_runtime(error.to_string()))?;

    Ok(())
}
```

Remove sampling fields from `McpServer`:

```rust
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
```

Remove `with_runtime_config`; it only exists to carry sampling runtime options.

Delete the `get_brief_generation_capabilities` and `generate_llm_brief` tool
handlers from `crates/rr-mcp/src/mcp_server.rs`.

Also delete these helper functions from `mcp_server.rs`:

```rust
fn select_llm_brief_elements<'a>(
    model: &'a SemanticModel,
    params: &LlmBriefParams,
) -> McpResult<Vec<&'a ElementSummary>> {
    // delete
}

fn generated_brief_from_content(
    content: GeneratedBriefContent,
    evidence_hash: String,
    cache_key: BriefCacheKey,
    generated_by_model: String,
    prompt_version: &str,
) -> GeneratedBrief {
    // delete
}
```

Keep `brief_generation_capabilities_from_peer` until Phase 5 removes the last
work-plan caller that depends on it.

After deleting the sampling handlers and helpers, remove their imports from
`crates/rr-mcp/src/mcp_server.rs`:

```rust
BriefCacheKey, BriefLlmClient, BriefLlmRequest, GeneratedBrief,
GeneratedBriefCacheEntry, GeneratedBriefContent, LlmBriefParams,
SamplingBriefLlmClient,
```

Also remove the `futures::{StreamExt, stream}` import and the
sampling-only `LLM_BRIEF_INSTRUCTIONS` constant. Keep
`BriefGenerationCapabilities`, `RequestContext`, and `RoleServer` until
Phase 5 removes the last work-plan capability caller.

Remove sampling-only errors from `crates/rr-mcp/src/error.rs` and
`crates/rr-mcp/src/error_conversion.rs` after their callers are deleted:

```rust
BriefSamplingUnavailable
BriefGenerationRequestFailed
BriefGenerationResponseInvalid
brief_sampling_unavailable
brief_generation_request_failed
brief_generation_response_invalid
```

Also remove their `message()` match arms and protocol-conversion match arms.

After Phase 5 removes the work-plan sampling/model fields, this command must
return no matches in `rr-mcp/src`:

```bash
! rg -n "generate_llm_brief|get_brief_generation_capabilities|sampling|createMessage|BriefLlm|SamplingBrief|GeneratedBrief|BriefCacheKey|LlmBriefParams|brief_model|max_concurrent_requests" crates/rr-mcp/src
```

## Phase 2: Add Persistent `.refactor-radar/` Cache Metadata

Add the Phase 2 `McpError` variants, constructors, `message()` arms, and
`error_conversion.rs` match arms before adding `project_bootstrap_source.rs`,
`project_cache_paths.rs`, or their `lib.rs` declarations. Those new source
files call the cache/bootstrap constructors.

Add typed cache/path errors to `crates/rr-mcp/src/error.rs`:

```rust
#[error("{message} at {location}")]
ProjectPathCanonicalizationFailed {
    message: &'static str,
    path: String,
    details: String,
    location: ErrorLocation,
},
#[error("{message} at {location}")]
ProjectCacheDirectoryCreationFailed {
    message: &'static str,
    path: String,
    details: String,
    location: ErrorLocation,
},
#[error("{message} at {location}")]
ProjectCacheFileWriteFailed {
    message: &'static str,
    path: String,
    details: String,
    location: ErrorLocation,
},
#[error("{message} at {location}")]
ProjectCacheCopyFailed {
    message: &'static str,
    source: String,
    destination: String,
    details: String,
    location: ErrorLocation,
},
#[error("{message} at {location}")]
ProjectBootstrapMissing {
    message: &'static str,
    project_id: String,
    location: ErrorLocation,
},
```

Add constructors to `impl McpError`:

```rust
#[track_caller]
pub fn project_path_canonicalization_failed(
    path: impl Into<String>,
    details: impl Into<String>,
) -> Self {
    Self::ProjectPathCanonicalizationFailed {
        message: "failed to canonicalize project path",
        path: path.into(),
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn project_cache_directory_creation_failed(
    path: impl Into<String>,
    details: impl Into<String>,
) -> Self {
    Self::ProjectCacheDirectoryCreationFailed {
        message: "failed to create RefactorRadar cache directory",
        path: path.into(),
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn project_cache_file_write_failed(
    path: impl Into<String>,
    details: impl Into<String>,
) -> Self {
    Self::ProjectCacheFileWriteFailed {
        message: "failed to write RefactorRadar cache file",
        path: path.into(),
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn project_cache_copy_failed(
    source: impl Into<String>,
    destination: impl Into<String>,
    details: impl Into<String>,
) -> Self {
    Self::ProjectCacheCopyFailed {
        message: "failed to copy RefactorRadar cache artifact",
        source: source.into(),
        destination: destination.into(),
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn project_bootstrap_missing(project_id: impl Into<String>) -> Self {
    Self::ProjectBootstrapMissing {
        message: "project bootstrap metadata was not found",
        project_id: project_id.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}
```

Update `message()`:

```rust
Self::ProjectPathCanonicalizationFailed { message, .. }
| Self::ProjectCacheDirectoryCreationFailed { message, .. }
| Self::ProjectCacheFileWriteFailed { message, .. }
| Self::ProjectCacheCopyFailed { message, .. }
| Self::ProjectBootstrapMissing { message, .. } => message,
```

Update `crates/rr-mcp/src/error_conversion.rs`:

```rust
McpError::ProjectBootstrapMissing {
    message,
    project_id,
    ..
} => ProtocolError::internal_error(message, Some(json!({ "project_id": project_id }))),
McpError::ProjectPathCanonicalizationFailed {
    message,
    path,
    details,
    ..
}
| McpError::ProjectCacheDirectoryCreationFailed {
    message,
    path,
    details,
    ..
}
| McpError::ProjectCacheFileWriteFailed {
    message,
    path,
    details,
    ..
} => ProtocolError::internal_error(
    message,
    Some(json!({ "path": path, "details": details })),
),
McpError::ProjectCacheCopyFailed {
    message,
    source,
    destination,
    details,
    ..
} => ProtocolError::internal_error(
    message,
    Some(json!({
        "source": source,
        "destination": destination,
        "details": details
    })),
),
```

Ignore the repo-local cache:

```gitignore
# RefactorRadar local artifacts
.refactor-radar/
```

Add `crates/rr-mcp/src/project_bootstrap_kind.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectBootstrapKind {
    LoadScipProject,
    ScanProject,
}
```

Add `crates/rr-mcp/src/project_bootstrap_source.rs`:

```rust
use crate::{BriefTaskToolCall, McpError, McpResult, ProjectBootstrapKind};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectBootstrapSource {
    pub project_id: String,
    pub kind: ProjectBootstrapKind,
    pub project_path: Option<PathBuf>,
    pub scip_path: Option<PathBuf>,
    pub cached_scip_path: Option<PathBuf>,
    pub scip_format: Option<String>,
    pub project_cache_dir: PathBuf,
    pub rust_analyzer_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
}

impl ProjectBootstrapSource {
    pub fn bootstrap_tool_call(&self) -> McpResult<BriefTaskToolCall> {
        if let Some(path) = self.cached_scip_path.as_ref().or(self.scip_path.as_ref()) {
            let format = self.scip_format.as_deref().unwrap_or("scip");
            return Ok(BriefTaskToolCall {
                tool_name: "load_scip_project".to_owned(),
                arguments: serde_json::json!({
                    "path": path.display().to_string(),
                    "format": format
                }),
            });
        }

        if let Some(path) = self.project_path.as_ref() {
            return Ok(BriefTaskToolCall {
                tool_name: "scan_project".to_owned(),
                arguments: serde_json::json!({
                    "path": path.display().to_string(),
                    "config_path": self
                        .config_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                }),
            });
        }

        Err(McpError::project_bootstrap_missing(self.project_id.clone()))
    }
}
```

Add `crates/rr-mcp/src/project_cache_manifest.rs`:

```rust
use crate::ProjectBootstrapKind;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectCacheManifest {
    pub project_id: String,
    pub project_path: Option<PathBuf>,
    pub scip_path: Option<PathBuf>,
    pub cached_scip_path: PathBuf,
    pub scip_format: String,
    pub bootstrap_kind: ProjectBootstrapKind,
    pub rust_analyzer_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
}
```

Add `crates/rr-mcp/src/project_cache_paths.rs`:

```rust
use crate::{McpError, McpResult};

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCachePaths {
    pub cache_root: PathBuf,
    pub project_dir: PathBuf,
    pub project_manifest: PathBuf,
    pub index_scip: PathBuf,
    pub work_plans_dir: PathBuf,
    pub evidence_briefs_dir: PathBuf,
    pub generated_briefs_dir: PathBuf,
    pub summaries_dir: PathBuf,
}

impl ProjectCachePaths {
    pub fn for_path(path: &Path, project_id: &str) -> McpResult<Self> {
        let root = repository_root_for(path)?;
        let project_hash = stable_hash(&serde_json::json!({
            "project_id": project_id,
            "root": root.display().to_string()
        }))?;
        let cache_root = root.join(".refactor-radar");
        let project_dir = cache_root.join("projects").join(project_hash);

        Ok(Self {
            project_manifest: project_dir.join("project.json"),
            index_scip: project_dir.join("index.scip"),
            work_plans_dir: project_dir.join("work-plans"),
            evidence_briefs_dir: project_dir.join("evidence-briefs"),
            generated_briefs_dir: project_dir.join("generated-briefs"),
            summaries_dir: project_dir.join("summaries"),
            cache_root,
            project_dir,
        })
    }

    pub fn ensure_directories(&self) -> McpResult<()> {
        for path in [
            &self.project_dir,
            &self.work_plans_dir,
            &self.evidence_briefs_dir,
            &self.generated_briefs_dir,
            &self.summaries_dir,
        ] {
            std::fs::create_dir_all(path).map_err(|error| {
                McpError::project_cache_directory_creation_failed(
                    path.display().to_string(),
                    error.to_string(),
                )
            })?;
        }

        Ok(())
    }
}

fn repository_root_for(path: &Path) -> McpResult<PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        McpError::project_path_canonicalization_failed(
            path.display().to_string(),
            error.to_string(),
        )
    })?;
    let start = if canonical.is_file() {
        canonical
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                McpError::project_path_canonicalization_failed(
                    canonical.display().to_string(),
                    "canonical file path had no parent",
                )
            })?
    } else {
        canonical
    };
    let fallback_root = start.clone();
    let mut current = start;

    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Ok(fallback_root),
        }
    }
}

fn stable_hash(value: &impl Serialize) -> McpResult<String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| McpError::serialization_failed(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
```

After the project bootstrap/cache files and their required errors exist, add
their modules and re-exports to `crates/rr-mcp/src/lib.rs`:

```rust
pub mod project_bootstrap_kind;
pub mod project_bootstrap_source;
pub mod project_cache_manifest;
pub mod project_cache_paths;

pub use project_bootstrap_kind::ProjectBootstrapKind;
pub use project_bootstrap_source::ProjectBootstrapSource;
pub use project_cache_manifest::ProjectCacheManifest;
pub use project_cache_paths::ProjectCachePaths;
```

Before editing `load_scip_project` or `scan_project`, add these Phase 2 types
to the top `use crate::{ ... }` list in `crates/rr-mcp/src/mcp_server.rs`:

```rust
ProjectBootstrapKind, ProjectBootstrapSource, ProjectCacheManifest,
ProjectCachePaths,
```

Update `crates/rr-mcp/src/project_cache.rs`:

```rust
use crate::ProjectBootstrapSource;

use rr_core::SemanticModel;

use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default)]
pub struct ProjectCache {
    pub models: HashMap<String, SemanticModel>,
    pub scip_paths: HashMap<String, PathBuf>,
    pub bootstrap_sources: HashMap<String, ProjectBootstrapSource>,
    pub producer_metadata: HashMap<String, String>,
    pub generation_diagnostics: HashMap<String, String>,
}

impl ProjectCache {
    pub fn insert(
        &mut self,
        model: SemanticModel,
        scip_path: PathBuf,
        bootstrap_source: ProjectBootstrapSource,
        diagnostics: Option<String>,
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

        self.scip_paths.insert(project_id.clone(), scip_path);
        self.bootstrap_sources
            .insert(project_id.clone(), bootstrap_source);
        self.models.insert(project_id, model);
    }
}
```

Add cache write helpers in `mcp_server.rs`:

```rust
fn write_json_cache_file(path: &std::path::Path, value: &impl Serialize) -> McpResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| McpError::serialization_failed(error.to_string()))?;
    std::fs::write(path, bytes).map_err(|error| {
        McpError::project_cache_file_write_failed(
            path.display().to_string(),
            error.to_string(),
        )
    })
}

fn copy_cache_file(source: &std::path::Path, destination: &std::path::Path) -> McpResult<()> {
    if source == destination {
        return Ok(());
    }

    std::fs::copy(source, destination).map(|_bytes| ()).map_err(|error| {
        McpError::project_cache_copy_failed(
            source.display().to_string(),
            destination.display().to_string(),
            error.to_string(),
        )
    })
}

fn cache_hash(value: &impl Serialize) -> McpResult<String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| McpError::serialization_failed(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn scip_format_name(format: Format) -> &'static str {
    match format {
        Format::Binary => "scip",
        Format::Json => "json",
    }
}
```

## Phase 3: Persist Bootstrap Metadata From `load_scip_project`

Update `ScipProjectResult` to expose the cached SCIP path:

```rust
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ScipProjectResult {
    pub project_id: String,
    pub cached_scip_path: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

Update `load_scip_project`:

```rust
async fn load_scip_project(
    &self,
    Parameters(params): Parameters<ScipProjectParams>,
) -> Result<Json<ScipProjectResult>, ProtocolError> {
    let source_path = PathBuf::from(&params.path);
    let format = match params.format.as_deref() {
        Some("scip") => Format::Binary,
        Some("json") => Format::Json,
        Some(other) => return Err(to_protocol_error(McpError::unsupported_format(other))),
        None => Format::detect(&source_path).map_err(scip_to_protocol_error)?,
    };

    let model = project_scip_index(&source_path, Some(format)).map_err(scip_to_protocol_error)?;
    let cache_paths = ProjectCachePaths::for_path(&source_path, &model.project.project_id)
        .map_err(to_protocol_error)?;
    cache_paths.ensure_directories().map_err(to_protocol_error)?;
    copy_cache_file(&source_path, &cache_paths.index_scip).map_err(to_protocol_error)?;

    let bootstrap_source = ProjectBootstrapSource {
        project_id: model.project.project_id.clone(),
        kind: ProjectBootstrapKind::LoadScipProject,
        project_path: None,
        scip_path: Some(source_path.clone()),
        cached_scip_path: Some(cache_paths.index_scip.clone()),
        scip_format: Some(scip_format_name(format).to_owned()),
        project_cache_dir: cache_paths.project_dir.clone(),
        rust_analyzer_path: None,
        config_path: None,
    };
    let manifest = ProjectCacheManifest {
        project_id: model.project.project_id.clone(),
        project_path: None,
        scip_path: Some(source_path),
        cached_scip_path: cache_paths.index_scip.clone(),
        scip_format: scip_format_name(format).to_owned(),
        bootstrap_kind: ProjectBootstrapKind::LoadScipProject,
        rust_analyzer_path: None,
        config_path: None,
        document_count: model.files.len(),
        symbol_count: model.elements.len(),
        occurrence_count: occurrence_count(&model),
        producer_name: model.project.producer_name.clone(),
        producer_version: model.project.producer_version.clone(),
    };
    write_json_cache_file(&cache_paths.project_manifest, &manifest)
        .map_err(to_protocol_error)?;

    let result = ScipProjectResult {
        project_id: model.project.project_id.clone(),
        cached_scip_path: cache_paths.index_scip.display().to_string(),
        document_count: model.files.len(),
        symbol_count: model.elements.len(),
        occurrence_count: occurrence_count(&model),
    };

    self.cache.write().await.insert(
        model,
        cache_paths.index_scip,
        bootstrap_source,
        None,
    );

    Ok(Json(result))
}
```

## Phase 4: Persist Bootstrap Metadata From `scan_project`

Remove `/tmp` default output from `scan_project`. When `output_path` is omitted,
write the SCIP file directly under `.refactor-radar/projects/<project-hash>/`.

Update `ProjectScanResult`:

```rust
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectScanResult {
    pub project_id: String,
    pub output_path: String,
    pub cached_scip_path: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

Update `scan_project`:

```rust
async fn scan_project(
    &self,
    Parameters(params): Parameters<ProjectScanParams>,
) -> Result<Json<ProjectScanResult>, ProtocolError> {
    let project_path = PathBuf::from(&params.path);
    let rust_analyzer_path = self.rust_analyzer_path.clone();
    let config_path = params.config_path.as_deref().map(PathBuf::from);

    let provisional_project_id = project_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_owned());
    let provisional_cache_paths = ProjectCachePaths::for_path(
        &project_path,
        provisional_project_id.as_str(),
    )
    .map_err(to_protocol_error)?;
    provisional_cache_paths
        .ensure_directories()
        .map_err(to_protocol_error)?;

    let output_path = params
        .output_path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| provisional_cache_paths.index_scip.clone());

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
    let cache_paths = ProjectCachePaths::for_path(&project_path, &model.project.project_id)
        .map_err(to_protocol_error)?;
    cache_paths.ensure_directories().map_err(to_protocol_error)?;
    copy_cache_file(&output_path, &cache_paths.index_scip).map_err(to_protocol_error)?;

    let bootstrap_source = ProjectBootstrapSource {
        project_id: model.project.project_id.clone(),
        kind: ProjectBootstrapKind::ScanProject,
        project_path: Some(project_path.clone()),
        scip_path: Some(output_path.clone()),
        cached_scip_path: Some(cache_paths.index_scip.clone()),
        scip_format: Some("scip".to_owned()),
        project_cache_dir: cache_paths.project_dir.clone(),
        rust_analyzer_path: rust_analyzer_path.clone(),
        config_path: config_path.clone(),
    };
    let manifest = ProjectCacheManifest {
        project_id: model.project.project_id.clone(),
        project_path: Some(project_path),
        scip_path: Some(output_path.clone()),
        cached_scip_path: cache_paths.index_scip.clone(),
        scip_format: "scip".to_owned(),
        bootstrap_kind: ProjectBootstrapKind::ScanProject,
        rust_analyzer_path,
        config_path,
        document_count: model.files.len(),
        symbol_count: model.elements.len(),
        occurrence_count: occurrence_count(&model),
        producer_name: model.project.producer_name.clone(),
        producer_version: model.project.producer_version.clone(),
    };
    write_json_cache_file(&cache_paths.project_manifest, &manifest)
        .map_err(to_protocol_error)?;

    let result = ProjectScanResult {
        project_id: model.project.project_id.clone(),
        output_path: output_path.display().to_string(),
        cached_scip_path: cache_paths.index_scip.display().to_string(),
        document_count: model.files.len(),
        symbol_count: model.elements.len(),
        occurrence_count: occurrence_count(&model),
    };

    self.cache.write().await.insert(
        model,
        cache_paths.index_scip,
        bootstrap_source,
        Some(stderr),
    );

    Ok(Json(result))
}
```

## Phase 5: Make Work Plans Bootstrap-First

Remove sampling/model fields from `BriefWorkPlanParams`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BriefWorkPlanParams {
    pub project_id: String,
    pub symbol_ids: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub reference_limit: Option<usize>,
    pub include_inferred: Option<bool>,
    pub budget_tokens: Option<usize>,
    pub max_subagent_tasks: Option<usize>,
    pub preferred_agent: Option<String>,
}
```

Remove `generation_capabilities` from `BriefWorkPlan`:

```rust
use crate::BriefWorkTask;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefWorkPlan {
    pub project_id: String,
    pub prompt_version: String,
    pub execution_model: String,
    pub parent_instructions: Vec<String>,
    pub tasks: Vec<BriefWorkTask>,
    pub merge_instructions: Vec<String>,
    pub verification_commands: Vec<String>,
    pub unknowns: Vec<String>,
}
```

After the `BriefWorkPlan`, `generate_brief_work_plan`,
`build_brief_work_plan`, and task-construction changes in this phase remove the
last `BriefGenerationCapabilities` references, delete the remaining capability
type and last capability-routing helpers:

```bash
git rm crates/rr-mcp/src/brief_generation_capabilities.rs
```

Remove these declarations from `crates/rr-mcp/src/lib.rs`:

```rust
pub mod brief_generation_capabilities;
pub use brief_generation_capabilities::BriefGenerationCapabilities;
```

Remove `BriefGenerationCapabilities` from `crates/rr-mcp/src/mcp_server.rs`
imports, delete `generate_brief_work_plan_with_capabilities`, delete
`brief_generation_capabilities_from_peer`, and remove `RequestContext` and
`RoleServer` from the `rmcp` imports after `generate_brief_work_plan` no longer
receives `RequestContext<RoleServer>`.

Add `prompt_version` to `crates/rr-mcp/src/brief_work_task.rs` so a sub-agent
that receives only its task object has the value required by the generated
output schema:

```rust
pub struct BriefWorkTask {
    pub task_id: String,
    pub prompt_version: String,
    pub title: String,
    pub preferred_agent: String,
    pub scope: String,
    pub symbol_ids: Vec<String>,
    pub tool_calls: Vec<BriefTaskToolCall>,
    pub expected_output_schema: BriefTaskOutputSchema,
    pub merge_key: String,
    pub budget_tokens: usize,
    pub dependencies: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}
```

Update the `generate_brief_work_plan` tool signature so it no longer receives
`RequestContext<RoleServer>`:

```rust
#[tool(
    name = "generate_brief_work_plan",
    description = "Return a Codex-ready sub-agent work manifest for decentralized brief generation",
    output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
)]
async fn generate_brief_work_plan(
    &self,
    Parameters(params): Parameters<BriefWorkPlanParams>,
) -> Result<Json<serde_json::Value>, ProtocolError> {
    let cache = self.cache.read().await;
    let model = cache
        .models
        .get(&params.project_id)
        .ok_or_else(|| McpError::missing_project(params.project_id.clone()))
        .map_err(to_protocol_error)?;
    let bootstrap_source = cache
        .bootstrap_sources
        .get(&params.project_id)
        .ok_or_else(|| McpError::project_bootstrap_missing(params.project_id.clone()))
        .map_err(to_protocol_error)?;
    let bootstrap_call = bootstrap_source
        .bootstrap_tool_call()
        .map_err(to_protocol_error)?;
    let elements = select_brief_work_plan_elements(model, &params).map_err(to_protocol_error)?;
    let plan = build_brief_work_plan(model, &params, elements, bootstrap_call)
        .map_err(to_protocol_error)?;

    let plan_hash = cache_hash(&plan).map_err(to_protocol_error)?;
    let work_plan_path = bootstrap_source
        .project_cache_dir
        .join("work-plans")
        .join(format!("{plan_hash}.json"));
    write_json_cache_file(&work_plan_path, &plan).map_err(to_protocol_error)?;

    structured(plan).map_err(to_protocol_error)
}
```

Update `build_brief_work_plan`:

```rust
fn build_brief_work_plan(
    model: &SemanticModel,
    params: &BriefWorkPlanParams,
    elements: Vec<&ElementSummary>,
    bootstrap_call: BriefTaskToolCall,
) -> McpResult<BriefWorkPlan> {
    let max_tasks = params.max_subagent_tasks.unwrap_or(6).max(1);
    let chunk_size = elements.len().div_ceil(max_tasks).max(1);
    let preferred_agent = params
        .preferred_agent
        .clone()
        .unwrap_or_else(|| "explorer".to_owned());
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
                &bootstrap_call,
            )
        })
        .collect::<Vec<_>>();

    Ok(BriefWorkPlan {
        project_id: model.project.project_id.clone(),
        prompt_version: LLM_BRIEF_PROMPT_VERSION.to_owned(),
        execution_model: "codex-parent-spawns-subagents".to_owned(),
        parent_instructions: vec![
            "Spawn one Codex sub-agent per task_id.".to_owned(),
            "Pass each sub-agent only its task object and repository constraints.".to_owned(),
            "Each sub-agent must execute the first tool call before any evidence call.".to_owned(),
            "If the bootstrap call returns a different project_id, use that returned project_id in subsequent evidence calls.".to_owned(),
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
    })
}
```

Update task construction:

```rust
fn build_brief_work_task(
    model: &SemanticModel,
    params: &BriefWorkPlanParams,
    elements: &[&ElementSummary],
    index: usize,
    preferred_agent: &str,
    bootstrap_call: &BriefTaskToolCall,
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
        bootstrap_call,
        model.project.project_id.as_str(),
        symbol_ids.as_slice(),
        reference_limit,
        include_inferred,
    );

    BriefWorkTask {
        task_id: task_id.clone(),
        prompt_version: LLM_BRIEF_PROMPT_VERSION.to_owned(),
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
            "Execute the bootstrap tool call before evidence calls.".to_owned(),
            "Use only evidence returned by listed MCP tool calls.".to_owned(),
            "Separate confirmed facts from inference.".to_owned(),
            "Put unsupported facts in unknowns.".to_owned(),
            "Return JSON matching expected_output_schema.".to_owned(),
            "Return this task's prompt_version value in the prompt_version field.".to_owned(),
            "Include every evidence_hash returned by get_element_brief.".to_owned(),
        ],
    }
}
```

Prepend the bootstrap call in `brief_work_tool_calls` and remove
`generate_llm_brief`:

```rust
fn brief_work_tool_calls(
    bootstrap_call: &BriefTaskToolCall,
    project_id: &str,
    symbol_ids: &[String],
    reference_limit: usize,
    include_inferred: bool,
) -> Vec<BriefTaskToolCall> {
    let mut calls = Vec::with_capacity(symbol_ids.len() + 1);
    calls.push(bootstrap_call.clone());
    calls.extend(symbol_ids.iter().map(|symbol_id| BriefTaskToolCall {
        tool_name: "get_element_brief".to_owned(),
        arguments: serde_json::json!({
            "symbol_id": symbol_id,
            "project_id": project_id,
            "include_inferred": include_inferred,
            "reference_limit": reference_limit
        }),
    }));
    calls
}
```

The first tool call in every task must look like this for cached SCIP:

```json
{
  "tool_name": "load_scip_project",
  "arguments": {
    "path": "/absolute/repo/.refactor-radar/projects/<project-hash>/index.scip",
    "format": "scip"
  }
}
```

Use `"format": "json"` instead when `load_scip_project` cached a JSON SCIP
input. Do not force `"scip"` unless the cached file is binary SCIP.

Only fall back to `scan_project` if no cached SCIP path exists:

```json
{
  "tool_name": "scan_project",
  "arguments": {
    "path": "/absolute/repo/or/project/path",
    "config_path": null
  }
}
```

## Phase 6: Add Deterministic Evidence Hashes And Cache Files

`get_element_brief` must return an `evidence_hash` and persist the deterministic
evidence packet under `.refactor-radar/projects/<project-hash>/evidence-briefs/`.

Patch the tail of `get_element_brief`:

```rust
let brief = build_element_brief(
    model,
    element.symbol_id.as_str(),
    params.include_inferred.unwrap_or(true),
    params.reference_limit.unwrap_or(20),
)
.map_err(|error| McpError::report_generation_failed(error.message()))
.map_err(to_protocol_error)?;

let evidence_hash = cache_hash(&brief).map_err(to_protocol_error)?;
let evidence_project_id = model.project.project_id.clone();
if let Some(bootstrap_source) = cache.bootstrap_sources.get(&evidence_project_id) {
    let path = bootstrap_source
        .project_cache_dir
        .join("evidence-briefs")
        .join(format!("{evidence_hash}.json"));
    write_json_cache_file(&path, &brief).map_err(to_protocol_error)?;
}

let mut value = serde_json::to_value(&brief)
    .map_err(|error| McpError::serialization_failed(error.to_string()))
    .map_err(to_protocol_error)?;
if let serde_json::Value::Object(map) = &mut value {
    map.insert(
        "evidence_hash".to_owned(),
        serde_json::Value::String(evidence_hash),
    );
}

Ok(Json(value))
```

`get_project_summary` should persist deterministic summaries:

```rust
let summary = build_project_summary(model);
let summary_hash = cache_hash(&summary).map_err(to_protocol_error)?;
if let Some(bootstrap_source) = cache.bootstrap_sources.get(&params.project_id) {
    let path = bootstrap_source
        .project_cache_dir
        .join("summaries")
        .join(format!("{summary_hash}.json"));
    write_json_cache_file(&path, &summary).map_err(to_protocol_error)?;
}

let mut value = serde_json::to_value(&summary)
    .map_err(|error| McpError::serialization_failed(error.to_string()))
    .map_err(to_protocol_error)?;
if let serde_json::Value::Object(map) = &mut value {
    map.insert(
        "summary_hash".to_owned(),
        serde_json::Value::String(summary_hash),
    );
}

Ok(Json(value))
```

## Phase 7: Add A Deterministic Output Recording Tool

Codex-generated text is produced outside `rr-mcp`, so `rr-mcp` needs an
explicit tool to persist that output. This replaces the deleted sampling cache.

Add `crates/rr-mcp/src/brief_task_five_w.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BriefTaskFiveW {
    pub who: String,
    pub what: String,
    pub when: String,
    #[serde(rename = "where")]
    pub where_: String,
    pub why: String,
    pub how: String,
}
```

Add `crates/rr-mcp/src/brief_task_output_record_params.rs`:

```rust
use crate::BriefTaskFiveW;

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BriefTaskOutputRecordParams {
    pub project_id: String,
    pub task_id: String,
    pub merge_key: String,
    pub prompt_version: String,
    pub generation_context_key: String,
    pub request_parameters: serde_json::Value,
    pub summary: String,
    pub five_w: BriefTaskFiveW,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknowns: Vec<String>,
    pub evidence_hashes: Vec<String>,
    pub source_span_ids: Vec<String>,
}
```

Add `crates/rr-mcp/src/brief_task_output_cache_entry.rs`:

```rust
use crate::BriefTaskFiveW;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BriefTaskOutputCacheEntry {
    pub project_id: String,
    pub task_id: String,
    pub merge_key: String,
    pub prompt_version: String,
    pub generation_context_key: String,
    pub request_parameters: serde_json::Value,
    pub output_hash: String,
    pub summary: String,
    pub five_w: BriefTaskFiveW,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknowns: Vec<String>,
    pub evidence_hashes: Vec<String>,
    pub source_span_ids: Vec<String>,
}
```

Add `crates/rr-mcp/src/brief_task_output_record_result.rs`:

```rust
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct BriefTaskOutputRecordResult {
    pub project_id: String,
    pub task_id: String,
    pub output_hash: String,
    pub path: String,
}
```

After the brief task output files exist, add their modules and re-exports to
`crates/rr-mcp/src/lib.rs`:

```rust
pub mod brief_task_five_w;
pub mod brief_task_output_cache_entry;
pub mod brief_task_output_record_params;
pub mod brief_task_output_record_result;

pub use brief_task_five_w::BriefTaskFiveW;
pub use brief_task_output_cache_entry::BriefTaskOutputCacheEntry;
pub use brief_task_output_record_params::BriefTaskOutputRecordParams;
pub use brief_task_output_record_result::BriefTaskOutputRecordResult;
```

Before adding the recording tool, add these Phase 7 types to the top
`use crate::{ ... }` list in `crates/rr-mcp/src/mcp_server.rs`:

```rust
BriefTaskOutputCacheEntry, BriefTaskOutputRecordParams,
BriefTaskOutputRecordResult,
```

Add the MCP tool:

```rust
#[tool(
    name = "record_brief_task_output",
    description = "Persist Codex-generated brief task output under the RefactorRadar repo cache"
)]
async fn record_brief_task_output(
    &self,
    Parameters(params): Parameters<BriefTaskOutputRecordParams>,
) -> Result<Json<BriefTaskOutputRecordResult>, ProtocolError> {
    let generation_context_key = params.generation_context_key.clone();
    let output_hash = cache_hash(&serde_json::json!({
        "project_id": &params.project_id,
        "task_id": &params.task_id,
        "merge_key": &params.merge_key,
        "prompt_version": &params.prompt_version,
        "generation_context_key": &generation_context_key,
        "request_parameters": &params.request_parameters,
        "evidence_hashes": &params.evidence_hashes,
        "source_span_ids": &params.source_span_ids,
        "summary": &params.summary,
        "five_w": &params.five_w,
        "confirmed": &params.confirmed,
        "inferred": &params.inferred,
        "unknowns": &params.unknowns
    }))
    .map_err(to_protocol_error)?;

    let entry = BriefTaskOutputCacheEntry {
        project_id: params.project_id.clone(),
        task_id: params.task_id.clone(),
        merge_key: params.merge_key,
        prompt_version: params.prompt_version,
        generation_context_key,
        request_parameters: params.request_parameters,
        output_hash: output_hash.clone(),
        summary: params.summary,
        five_w: params.five_w,
        confirmed: params.confirmed,
        inferred: params.inferred,
        unknowns: params.unknowns,
        evidence_hashes: params.evidence_hashes,
        source_span_ids: params.source_span_ids,
    };

    let cache = self.cache.read().await;
    let bootstrap_source = cache
        .bootstrap_sources
        .get(&entry.project_id)
        .ok_or_else(|| McpError::project_bootstrap_missing(entry.project_id.clone()))
        .map_err(to_protocol_error)?;
    let path = bootstrap_source
        .project_cache_dir
        .join("generated-briefs")
        .join(format!("{output_hash}.json"));
    write_json_cache_file(&path, &entry).map_err(to_protocol_error)?;

    Ok(Json(BriefTaskOutputRecordResult {
        project_id: entry.project_id,
        task_id: entry.task_id,
        output_hash,
        path: path.display().to_string(),
    }))
}
```

After `record_brief_task_output` exists, add this parent instruction in
`build_brief_work_plan`:

```rust
"After each sub-agent returns JSON, call record_brief_task_output with this plan's project_id, the task output fields, a stable generation context key, and request_parameters copied from the task/tool-call settings to persist the generated output.".to_owned(),
```

Update the expected sub-agent output schema so every generated output has the
fields the sub-agent must return before the parent calls
`record_brief_task_output`. The parent supplies the plan's `project_id` and the
generation context key and copies request parameters from the task/tool-call
settings when recording:

```rust
fn brief_task_output_schema() -> BriefTaskOutputSchema {
    BriefTaskOutputSchema {
        format: "json".to_owned(),
        required_fields: vec![
            "task_id".to_owned(),
            "merge_key".to_owned(),
            "prompt_version".to_owned(),
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
                "prompt_version",
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
                "prompt_version": { "type": "string" },
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
```

## Phase 8: Update Tests

Remove sampling-specific unit tests from `crates/rr-mcp/src/mcp_server/tests.rs`.
Delete the fake LLM client and all tests calling
`generate_llm_brief_with_client`.
Delete test helpers that depend on `BriefGenerationCapabilities`, including
`unsupported_sampling_capabilities` and `sampling_capabilities`. Update
surviving work-plan tests to call `generate_brief_work_plan(Parameters(...))`
directly.

Use this import shape after cleanup:

```rust
use crate::{
    BriefTaskOutputRecordParams, BriefTaskFiveW, BriefWorkPlanParams, ElementBriefParams,
    McpError, McpServer, ProjectScanParams, ProjectSummaryParams, RustScipGenerateParams,
    ScipProjectParams,
};
use protobuf::{Message, MessageField};
use rmcp::handler::server::wrapper::{Json, Parameters};
use scip::types::{
    Document, Index, Metadata, Occurrence, SymbolInformation, SymbolRole, ToolInfo,
    symbol_information,
};
use tempfile::tempdir;
```

Add `protobuf-json-mapping = { workspace = true }` to
`crates/rr-mcp/Cargo.toml` under `[dev-dependencies]` for the JSON SCIP
bootstrap test.

Update schema tests so `LlmBriefParams` is gone:

```rust
#[test]
fn given_tool_params_when_generating_schema_then_runtime_paths_are_not_exposed()
-> Result<(), Box<dyn std::error::Error>> {
    let scan_schema = schemars::schema_for!(ProjectScanParams);
    let generate_schema = schemars::schema_for!(RustScipGenerateParams);
    let work_plan_schema = schemars::schema_for!(BriefWorkPlanParams);

    let scan_schema_json = serde_json::to_string(&scan_schema)?;
    let generate_schema_json = serde_json::to_string(&generate_schema)?;
    let work_plan_schema_json = serde_json::to_string(&work_plan_schema)?;

    assert!(!scan_schema_json.contains("rust_analyzer_path"));
    assert!(!generate_schema_json.contains("rust_analyzer_path"));
    assert!(!work_plan_schema_json.contains("rust_analyzer_path"));
    assert!(!work_plan_schema_json.contains("brief_model"));
    assert!(!work_plan_schema_json.contains("max_concurrent_requests"));
    Ok(())
}
```

Update the runtime-path deserialization test so it no longer references
`LlmBriefParams` and also proves removed model/concurrency fields are rejected
from `BriefWorkPlanParams`:

```rust
#[test]
fn given_tool_params_when_deserializing_then_runtime_paths_are_rejected() {
    let scan_result = serde_json::from_value::<ProjectScanParams>(serde_json::json!({
        "path": "/tmp/project",
        "rust_analyzer_path": "/tmp/rust-analyzer"
    }));
    let generate_result = serde_json::from_value::<RustScipGenerateParams>(serde_json::json!({
        "path": "/tmp/project",
        "output_path": "/tmp/project.scip",
        "rust_analyzer_path": "/tmp/rust-analyzer"
    }));
    let work_plan_runtime_result =
        serde_json::from_value::<BriefWorkPlanParams>(serde_json::json!({
            "project_id": "fixture",
            "rust_analyzer_path": "/tmp/rust-analyzer"
        }));
    let work_plan_model_result =
        serde_json::from_value::<BriefWorkPlanParams>(serde_json::json!({
            "project_id": "fixture",
            "brief_model": "test-model"
        }));
    let work_plan_concurrency_result =
        serde_json::from_value::<BriefWorkPlanParams>(serde_json::json!({
            "project_id": "fixture",
            "max_concurrent_requests": 1
        }));

    assert!(scan_result.is_err());
    assert!(generate_result.is_err());
    assert!(work_plan_runtime_result.is_err());
    assert!(work_plan_model_result.is_err());
    assert!(work_plan_concurrency_result.is_err());
}
```

Update any surviving non-sampling tests that construct
`McpServer::with_runtime_config(None, None, None)` to use `McpServer::new()`
or `McpServer::with_rust_analyzer_path(...)`. Remove the
`server_with_fake_client` helper with the fake LLM client tests.

Add a unit test for bootstrap-first work plans:

```rust
#[tokio::test]
async fn given_loaded_project_when_generating_work_plan_then_first_tool_call_bootstraps_cache()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;

    let Json(result) = server
        .generate_brief_work_plan(Parameters(work_plan_params(vec![SYMBOL], Some(1))))
        .await
        .map_err(to_io_error)?;

    let tool_calls = result["tasks"][0]["tool_calls"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing tool calls"))?;
    assert_eq!("load_scip_project", tool_calls[0]["tool_name"]);
    assert_eq!("scip", tool_calls[0]["arguments"]["format"]);
    assert!(
        tool_calls[0]["arguments"]["path"]
            .as_str()
            .map(|path| path.contains(".refactor-radar/projects"))
            .unwrap_or(false)
    );
    assert_eq!("get_element_brief", tool_calls[1]["tool_name"]);
    assert_eq!("5w-summary-v1", result["tasks"][0]["prompt_version"]);
    Ok(())
}
```

Add a unit test proving cached JSON SCIP inputs preserve their explicit
bootstrap format:

```rust
#[tokio::test]
async fn given_json_scip_when_generating_work_plan_then_bootstrap_preserves_json_format()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let directory = tempdir()?;
    let path = directory.path().join("fixture.json");
    let json = protobuf_json_mapping::print_to_string(&sample_index())?;
    std::fs::write(&path, json)?;

    let Json(load_result) = server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: Some("json".to_owned()),
        }))
        .await
        .map_err(to_io_error)?;
    assert_eq!("fixture", load_result.project_id);

    let Json(result) = server
        .generate_brief_work_plan(Parameters(work_plan_params(vec![SYMBOL], Some(1))))
        .await
        .map_err(to_io_error)?;

    let bootstrap_call = &result["tasks"][0]["tool_calls"][0];
    assert_eq!("load_scip_project", bootstrap_call["tool_name"]);
    assert_eq!("json", bootstrap_call["arguments"]["format"]);
    let cached_path = bootstrap_call["arguments"]["path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing cached path"))?;
    assert!(cached_path.contains(".refactor-radar/projects"));
    assert!(std::path::Path::new(cached_path).exists());

    let fresh_server = McpServer::new();
    let Json(fresh_load_result) = fresh_server
        .load_scip_project(Parameters(ScipProjectParams {
            path: cached_path.to_owned(),
            format: Some("json".to_owned()),
        }))
        .await
        .map_err(to_io_error)?;
    assert_eq!("fixture", fresh_load_result.project_id);
    Ok(())
}
```

Add a unit test for `scan_project` cache persistence. This covers the
`scan_project` entrypoint without requiring a real `rust-analyzer` binary by
using a Unix test script that copies a fixture SCIP file to the requested
`--output` path:

```rust
#[tokio::test]
async fn given_scan_project_when_output_path_is_omitted_then_cache_metadata_is_written()
-> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let directory = tempdir()?;
        let project_path = directory.path().join("project");
        std::fs::create_dir(&project_path)?;
        std::fs::write(
            project_path.join("fixture.scip"),
            sample_index().write_to_bytes()?,
        )?;
        let script_path = directory.path().join("ra-copies-fixture");
        write_executable_script(
            &script_path,
            "#!/bin/sh\necho indexed >&2\ncp \"$2/fixture.scip\" \"$4\"\n",
        )?;
        let server = McpServer::with_rust_analyzer_path(Some(script_path));

        let Json(result) = server
            .scan_project(Parameters(ProjectScanParams {
                path: project_path.display().to_string(),
                output_path: None,
                config_path: None,
            }))
            .await
            .map_err(to_io_error)?;

        assert_eq!("fixture", result.project_id);
        assert!(result.output_path.contains(".refactor-radar/projects"));
        assert!(result.cached_scip_path.contains(".refactor-radar/projects"));
        let cached_scip_path = std::path::PathBuf::from(&result.cached_scip_path);
        assert!(cached_scip_path.exists());

        let cache = server.cache.read().await;
        let bootstrap_source = cache
            .bootstrap_sources
            .get("fixture")
            .ok_or_else(|| std::io::Error::other("missing bootstrap source"))?;
        assert_eq!(Some(&cached_scip_path), bootstrap_source.cached_scip_path.as_ref());
        assert!(bootstrap_source.project_cache_dir.join("project.json").exists());
    }
    Ok(())
}
```

Add a unit test for missing bootstrap metadata:

```rust
#[tokio::test]
async fn given_project_without_bootstrap_metadata_when_generating_work_plan_then_typed_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;
    server
        .cache
        .write()
        .await
        .bootstrap_sources
        .remove("fixture");

    let error = server
        .generate_brief_work_plan(Parameters(work_plan_params(vec![SYMBOL], Some(1))))
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("expected bootstrap error"))?;
    let message = format!("{error:?}");

    assert!(message.contains("project bootstrap metadata was not found"));
    Ok(())
}
```

Add a unit test for evidence hash output:

```rust
#[tokio::test]
async fn given_element_brief_when_returned_then_evidence_hash_is_present_and_cache_file_is_written()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;

    let Json(result) = server
        .get_element_brief(Parameters(ElementBriefParams {
            symbol_id: SYMBOL.to_owned(),
            project_id: Some("fixture".to_owned()),
            include_inferred: Some(true),
            reference_limit: Some(3),
        }))
        .await
        .map_err(to_io_error)?;

    let evidence_hash = result["evidence_hash"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing evidence hash"))?;
    assert_eq!(64, evidence_hash.len());

    let cache = server.cache.read().await;
    let bootstrap_source = cache
        .bootstrap_sources
        .get("fixture")
        .ok_or_else(|| std::io::Error::other("missing bootstrap source"))?;
    let path = bootstrap_source
        .project_cache_dir
        .join("evidence-briefs")
        .join(format!("{evidence_hash}.json"));
    assert!(path.exists());
    Ok(())
}
```

Add a unit test for deterministic project summary caching:

```rust
#[tokio::test]
async fn given_project_summary_when_returned_then_summary_hash_is_present_and_cache_file_is_written()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;

    let Json(result) = server
        .get_project_summary(Parameters(ProjectSummaryParams {
            project_id: "fixture".to_owned(),
        }))
        .await
        .map_err(to_io_error)?;

    assert_eq!("fixture", result["project_id"]);
    let summary_hash = result["summary_hash"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing summary hash"))?;
    assert_eq!(64, summary_hash.len());

    let cache = server.cache.read().await;
    let bootstrap_source = cache
        .bootstrap_sources
        .get("fixture")
        .ok_or_else(|| std::io::Error::other("missing bootstrap source"))?;
    let path = bootstrap_source
        .project_cache_dir
        .join("summaries")
        .join(format!("{summary_hash}.json"));
    assert!(path.exists());
    Ok(())
}
```

Add a unit test for recording generated output:

```rust
#[tokio::test]
async fn given_generated_task_output_when_recorded_then_cache_file_is_written()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;

    let Json(result) = server
        .record_brief_task_output(Parameters(BriefTaskOutputRecordParams {
            project_id: "fixture".to_owned(),
            task_id: "brief-task-001".to_owned(),
            merge_key: "fixture::brief-task-001".to_owned(),
            prompt_version: "5w-summary-v1".to_owned(),
            generation_context_key: "codex-test".to_owned(),
            request_parameters: serde_json::json!({
                "symbol_ids": [SYMBOL],
                "reference_limit": 3,
                "include_inferred": true,
                "budget_tokens": 500
            }),
            summary: "generated summary".to_owned(),
            five_w: BriefTaskFiveW {
                who: "unknown".to_owned(),
                what: "fixture evidence".to_owned(),
                when: "unknown".to_owned(),
                where_: "src/lib.rs".to_owned(),
                why: "unknown".to_owned(),
                how: "get_element_brief".to_owned(),
            },
            confirmed: vec!["fixture symbol exists".to_owned()],
            inferred: Vec::new(),
            unknowns: vec!["runtime behavior".to_owned()],
            evidence_hashes: vec!["0".repeat(64)],
            source_span_ids: vec!["span:fixture".to_owned()],
        }))
        .await
        .map_err(to_io_error)?;

    let path = result.path;
    assert!(std::path::Path::new(&path).exists());
    assert_eq!("brief-task-001", result.task_id);
    assert_eq!(64, result.output_hash.len());
    Ok(())
}
```

Add a unit test proving relevant request parameters are part of the
generated-output cache key:

```rust
#[tokio::test]
async fn given_generated_task_output_when_request_parameters_change_then_output_hash_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::new();
    let _fixture_dir = load_fixture_project(&server).await?;

    let Json(first) = server
        .record_brief_task_output(Parameters(BriefTaskOutputRecordParams {
            project_id: "fixture".to_owned(),
            task_id: "brief-task-001".to_owned(),
            merge_key: "fixture::brief-task-001".to_owned(),
            prompt_version: "5w-summary-v1".to_owned(),
            generation_context_key: "codex-test".to_owned(),
            request_parameters: serde_json::json!({
                "symbol_ids": [SYMBOL],
                "reference_limit": 1,
                "include_inferred": true,
                "budget_tokens": 500
            }),
            summary: "generated summary".to_owned(),
            five_w: BriefTaskFiveW {
                who: "unknown".to_owned(),
                what: "fixture evidence".to_owned(),
                when: "unknown".to_owned(),
                where_: "src/lib.rs".to_owned(),
                why: "unknown".to_owned(),
                how: "get_element_brief".to_owned(),
            },
            confirmed: vec!["fixture symbol exists".to_owned()],
            inferred: Vec::new(),
            unknowns: vec!["runtime behavior".to_owned()],
            evidence_hashes: vec!["0".repeat(64)],
            source_span_ids: vec!["span:fixture".to_owned()],
        }))
        .await
        .map_err(to_io_error)?;

    let Json(second) = server
        .record_brief_task_output(Parameters(BriefTaskOutputRecordParams {
            project_id: "fixture".to_owned(),
            task_id: "brief-task-001".to_owned(),
            merge_key: "fixture::brief-task-001".to_owned(),
            prompt_version: "5w-summary-v1".to_owned(),
            generation_context_key: "codex-test".to_owned(),
            request_parameters: serde_json::json!({
                "symbol_ids": [SYMBOL],
                "reference_limit": 3,
                "include_inferred": true,
                "budget_tokens": 500
            }),
            summary: "generated summary".to_owned(),
            five_w: BriefTaskFiveW {
                who: "unknown".to_owned(),
                what: "fixture evidence".to_owned(),
                when: "unknown".to_owned(),
                where_: "src/lib.rs".to_owned(),
                why: "unknown".to_owned(),
                how: "get_element_brief".to_owned(),
            },
            confirmed: vec!["fixture symbol exists".to_owned()],
            inferred: Vec::new(),
            unknowns: vec!["runtime behavior".to_owned()],
            evidence_hashes: vec!["0".repeat(64)],
            source_span_ids: vec!["span:fixture".to_owned()],
        }))
        .await
        .map_err(to_io_error)?;

    assert_ne!(first.output_hash, second.output_hash);
    Ok(())
}
```

Update the unit-test `load_fixture_project` helper so cache files created from a
temporary SCIP fixture remain available for bootstrap assertions:

```rust
async fn load_fixture_project(
    server: &McpServer,
) -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let (directory, path) = write_fixture_scip()?;
    server
        .load_scip_project(Parameters(ScipProjectParams {
            path: path.display().to_string(),
            format: None,
        }))
        .await
        .map_err(to_io_error)?;
    Ok(directory)
}
```

Add this Unix helper for the `scan_project` test:

```rust
#[cfg(unix)]
fn write_executable_script(
    path: &std::path::Path,
    contents: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let temporary_path = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&temporary_path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }

    let mut permissions = std::fs::metadata(&temporary_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&temporary_path, permissions)?;
    std::fs::rename(temporary_path, path)?;
    Ok(())
}
```

Update every surviving unit-test call site that uses this helper before
`generate_brief_work_plan`, `get_element_brief`, `get_project_summary`, or
`record_brief_task_output` so the returned temporary directory stays alive for
the rest of the test:

```rust
let _fixture_dir = load_fixture_project(&server).await?;
```

Do not leave a bare `load_fixture_project(&server).await?;` before a
cache-backed tool call, because dropping the returned `TempDir` deletes the
fixture-local `.refactor-radar/` cache that the tool must read or write.

Update `work_plan_params` helper:

```rust
fn work_plan_params(
    symbol_ids: Vec<&str>,
    max_subagent_tasks: Option<usize>,
) -> BriefWorkPlanParams {
    BriefWorkPlanParams {
        project_id: "fixture".to_owned(),
        symbol_ids: Some(symbol_ids.into_iter().map(str::to_owned).collect()),
        limit: None,
        reference_limit: Some(3),
        include_inferred: Some(true),
        budget_tokens: Some(500),
        max_subagent_tasks,
        preferred_agent: Some("explorer".to_owned()),
    }
}
```

Replace `crates/rr-mcp/tests/brief_work_plan_protocol.rs` with deterministic
protocol tests:

```rust
mod support;

use support::fixture_scip::{PROJECT_ID, SYMBOL, write_fixture_scip};

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, JsonObject},
};
use rr_mcp::McpServer;

#[tokio::test]
async fn given_mcp_client_when_generating_work_plan_then_task_starts_with_bootstrap()
-> Result<(), Box<dyn std::error::Error>> {
    let client = start_protocol_pair().await?;
    let _fixture_dir = load_fixture_project(&client).await?;

    let arguments = json_object(serde_json::json!({
        "project_id": PROJECT_ID,
        "symbol_ids": [SYMBOL],
        "reference_limit": 1,
        "include_inferred": true,
        "budget_tokens": 500,
        "max_subagent_tasks": 4,
        "preferred_agent": "explorer"
    }))?;
    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("generate_brief_work_plan").with_arguments(arguments))
        .await?;
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;

    assert_eq!("load_scip_project", structured["tasks"][0]["tool_calls"][0]["tool_name"]);
    assert_eq!("get_element_brief", structured["tasks"][0]["tool_calls"][1]["tool_name"]);

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn given_fresh_client_when_executing_task_bootstrap_then_evidence_call_succeeds()
-> Result<(), Box<dyn std::error::Error>> {
    let parent = start_protocol_pair().await?;
    let _fixture_dir = load_fixture_project(&parent).await?;

    let arguments = json_object(serde_json::json!({
        "project_id": PROJECT_ID,
        "symbol_ids": [SYMBOL],
        "reference_limit": 1,
        "include_inferred": true,
        "max_subagent_tasks": 1
    }))?;
    let result = parent
        .peer()
        .call_tool(CallToolRequestParams::new("generate_brief_work_plan").with_arguments(arguments))
        .await?;
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;
    let bootstrap_args = structured["tasks"][0]["tool_calls"][0]["arguments"]
        .as_object()
        .cloned()
        .ok_or_else(|| std::io::Error::other("missing bootstrap args"))?;

    let subagent = start_protocol_pair().await?;
    let bootstrap_result = subagent
        .peer()
        .call_tool(
            CallToolRequestParams::new("load_scip_project")
                .with_arguments(JsonObject::from_iter(bootstrap_args)),
        )
        .await?;
    assert_eq!(Some(false), bootstrap_result.is_error);

    let evidence_args = json_object(serde_json::json!({
        "project_id": PROJECT_ID,
        "symbol_id": SYMBOL,
        "reference_limit": 1,
        "include_inferred": true
    }))?;
    let evidence_result = subagent
        .peer()
        .call_tool(CallToolRequestParams::new("get_element_brief").with_arguments(evidence_args))
        .await?;
    assert_eq!(Some(false), evidence_result.is_error);

    parent.cancel().await?;
    subagent.cancel().await?;
    Ok(())
}
```

Keep the local client and helper functions in that protocol test file:

```rust
#[derive(Clone)]
struct WorkPlanTestClient;

impl rmcp::ClientHandler for WorkPlanTestClient {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        rmcp::model::ClientInfo::new(
            rmcp::model::ClientCapabilities::default(),
            rmcp::model::Implementation::new("rr-mcp-work-plan-test-client", "0.0.0"),
        )
    }
}

async fn start_protocol_pair()
-> Result<rmcp::service::RunningService<rmcp::RoleClient, WorkPlanTestClient>, Box<dyn std::error::Error>>
{
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let _server_task = tokio::spawn(async move {
        if let Ok(server) = McpServer::new().serve(server_transport).await {
            let _ = server.waiting().await;
        }
    });

    let client = WorkPlanTestClient.serve(client_transport).await?;
    Ok(client)
}

async fn load_fixture_project(
    client: &rmcp::service::RunningService<rmcp::RoleClient, WorkPlanTestClient>,
) -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let (directory, path) = write_fixture_scip()?;
    let arguments = json_object(serde_json::json!({
        "path": path.display().to_string()
    }))?;

    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("load_scip_project").with_arguments(arguments))
        .await?;

    if result.is_error == Some(true) {
        return Err(std::io::Error::other(format!("load_scip_project failed: {result:?}")).into());
    }

    Ok(directory)
}

fn json_object(value: serde_json::Value) -> Result<JsonObject, Box<dyn std::error::Error>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| std::io::Error::other("expected JSON object").into())
}
```

## Phase 9: Update README

Replace the host-sampling README section with deterministic work-plan language:

````markdown
## Codex Work Plans

The working Codex flow is:

```text
scan_project or load_scip_project
generate_brief_work_plan
each sub-agent executes the first bootstrap tool_call
each sub-agent executes the returned get_element_brief tool_calls
each sub-agent returns JSON matching expected_output_schema
parent Codex merges task outputs by merge_key
parent Codex records outputs with record_brief_task_output
```

`generate_brief_work_plan` is the Codex path. It is deterministic and returns
task objects with exact MCP tool-call recipes. The first tool call bootstraps a
fresh MCP session from `.refactor-radar/projects/<project-hash>/index.scip`
when a cached SCIP file exists.

Codex sub-agents may not share the parent MCP server's in-memory project cache,
so every task is self-bootstrapping.

`rr-mcp` only returns deterministic tool-call recipes and persists returned task
output. It does not request host-generated text, and it does not spawn or manage
Codex sub-agents.
```
````

Document the cache:

````markdown
## RefactorRadar Cache

`rr-mcp` writes deterministic local artifacts under:

```text
.refactor-radar/
  projects/
    <project-hash>/
      project.json
      index.scip
      work-plans/
      evidence-briefs/
      generated-briefs/
      summaries/
```

The cache is ignored by git. It can be deleted and regenerated.

Generated output cache keys include the project id, task id, merge key, prompt
version, generation context key, evidence hashes, source span ids, and output
content hash. If evidence, prompt version, generation context, or relevant
request parameters change, the cache key changes and stale generated output is
not reused.
```
````

Remove README text mentioning:

```text
get_brief_generation_capabilities
generate_llm_brief
host-mediated sampling
sampling/createMessage
sampling happy path
unsupported-host verified
--brief-model
--max-concurrent-requests
```

## Phase 10: Verification Commands

Format and test:

```bash
cargo fmt --check
cargo test -p rr-mcp
cargo check -p rr-mcp --all-targets
cargo test --workspace --all-targets
```

Confirm sampling is gone:

```bash
! rg -n "generate_llm_brief|get_brief_generation_capabilities|sampling/createMessage|SamplingBrief|BriefLlm|BriefCacheKey|LlmBriefParams|GeneratedBrief" crates/rr-mcp/src crates/rr-mcp/tests README.md
! rg -n "brief_model|max_concurrent_requests" crates/rr-mcp/src README.md
```

Confirm no forbidden error handling or provider path was added:

```bash
! rg -n "anyhow|reqwest|async-openai|OPENAI_API_KEY|unwrap\\(|expect\\(|panic!\\(" crates/rr-mcp/src crates/rr-mcp/tests
```

Confirm the cache path is consistently spelled:

```bash
! rg -n "\\.refactor-rador" . --glob '!gpt-suck-ass.md'
rg -n "\\.refactor-radar" .gitignore README.md crates/rr-mcp/src crates/rr-mcp/tests
```

Confirm submodules were not changed by the implementation:

```bash
git status --short
git submodule status --recursive
```

The pre-existing `m submodules/scip` marker may still be present. There must be
no new file-level changes under `submodules/`.

## Acceptance Criteria

The implementation is complete when these checks pass:

```text
- generate_brief_work_plan is exposed.
- generate_llm_brief is not exposed.
- get_brief_generation_capabilities is not exposed.
- Host sampling/createMessage code is removed from rr-mcp.
- Sampling-only model hint/cache code is removed.
- scan_project writes generated SCIP and project metadata under .refactor-radar/.
- load_scip_project copies loaded SCIP into .refactor-radar/.
- Every generated task starts with load_scip_project or scan_project.
- Work-plan tasks prefer load_scip_project from .refactor-radar/ when available.
- JSON SCIP cache bootstrap calls preserve `format: json` and load in a fresh MCP server.
- A fresh MCP client can execute a returned task from bootstrap through get_element_brief.
- get_element_brief returns an evidence_hash.
- Deterministic evidence briefs are cached under .refactor-radar/.
- Deterministic project summaries are cached under .refactor-radar/.
- record_brief_task_output writes Codex-generated output under .refactor-radar/.
- Generated-output cache keys include evidence hashes, prompt version, generation context, and relevant params.
- Stale generated brief/summary cache entries are not reused after evidence or params change.
- .refactor-radar/ is gitignored.
- README states the actual deterministic Codex sub-agent flow.
- README does not present MCP sampling as a Codex feature path.
- cargo fmt --check passes.
- cargo test -p rr-mcp passes.
- cargo check -p rr-mcp --all-targets passes.
- cargo test --workspace --all-targets passes.
- No code file uses anyhow for application/library errors.
- New rr-mcp errors use thiserror, ErrorLocation, and #[track_caller] constructors.
- No new file contains more than one enum/struct/trait type.
- No file under submodules/ is modified by this implementation.
```
