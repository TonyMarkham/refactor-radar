# Implementation Plan: Host-Sampled `generate_llm_brief`

## Target Outcome

Implement `rr-mcp` host-mediated generated brief support end to end.

```text
cached SCIP semantic model
  -> host sampling capability probe
  -> deterministic element/evidence selection
  -> Codex-ready decentralized work manifest
  -> parent Codex session can spawn one sub-agent per manifest task
  -> sub-agents receive exact MCP tool-call recipes and expected output schema
  -> MCP host sampling/createMessage request
  -> strict JSON response validation
  -> generated 5W brief structs
  -> evidence-hash/model/prompt cache
  -> typed unsupported-host errors when sampling is unavailable
  -> unit and MCP protocol tests with no network/model calls
```

This is not a provider implementation. Do not add OpenAI HTTP calls. `rr-mcp`
must ask the connected MCP host to sample through `rmcp` and
`sampling/createMessage`.

This is also not server-side Codex orchestration. `rr-mcp` cannot spawn or
control Codex sub-agents. The implementation must instead provide Codex with a
complete decentralized work package that the parent Codex session can use to
spawn and coordinate sub-agents explicitly.

## Hard Constraints

Follow the repository rules exactly:

```text
- Do not edit anything under submodules/.
- Keep one Rust type per file. A file may contain helper functions, constants,
  and tests, but only one enum/struct/trait/type alias.
- Use crate-local typed errors with thiserror, error_location::ErrorLocation,
  and #[track_caller] constructors.
- Do not add anyhow for application/library errors.
- Do not add reqwest, async-openai, openai, OPENAI_API_KEY, or direct provider code.
- Do not introduce implementation unwrap, expect, or panic.
- Tests must not call Codex, OpenAI, the network, or a paid model.
- Deterministic MCP tools must remain useful without generated LLM summaries.
- The MCP server must provide enough structured task data for Codex to
  decentralize summary/brief work without guessing task boundaries.
- The MCP server must expose the connected host's sampling capability as
  structured data so Codex can choose between generated briefs and deterministic
  sub-agent work without trial-and-error.
```

Pre-flight commands:

```bash
git status --short
git submodule status --recursive
rg -n "generate_llm_brief|Brief|sampling|create_message" crates/rr-mcp/src -g '*.rs'
rg -n "pub struct .*\\{|pub enum .*\\{|pub trait .*\\{|pub type " crates/rr-mcp/src -g '*.rs'
rg -n "reqwest|async-openai|OPENAI_API_KEY|anyhow" Cargo.toml crates -g '*.toml' -g '*.rs'
```

The last command should only find intentional plan/check text, not production
dependencies or code.

## Confirmed Facts

The design is supported by local code and `rmcp`:

```bash
rg -n "rmcp\\s*=|async-trait|futures|sha2|tempfile" Cargo.toml crates/rr-mcp/Cargo.toml
rg -n "RequestContext<RoleServer>|Peer<RoleServer>|CreateMessageRequestParams|ClientHandler" ~/.cargo/registry/src/index.crates.io-*/rmcp-1.7.0/src ~/.cargo/registry/src/index.crates.io-*/rmcp-1.7.0/tests
rg -n "McpError|ErrorLocation|message\\(" crates/rr-mcp/src/error.rs crates/rr-mcp/src/error_conversion.rs
```

Known integration points:

```text
- rr-mcp already uses rmcp for the MCP server.
- rmcp 1.7.0 exposes Peer<RoleServer>::create_message.
- rmcp ClientHandler can implement create_message for deterministic tests.
- Tool handlers can receive RequestContext<RoleServer>.
- rr-mcp owns crate-local McpError and McpResult.
- rr-core owns deterministic element brief generation.
- Codex sub-agents are orchestrated by Codex, not by MCP servers.
- MCP can still return a deterministic manifest that tells Codex exactly how to
  split, execute, and merge decentralized brief/summary tasks.
- Host support for `sampling/createMessage` must be detected from the current
  MCP request context because it is a client/host capability, not a server
  config value.
```

## Implementation Checklist

### 1. Dependencies

Keep production dependencies provider-neutral.

`Cargo.toml`:

Keep the existing workspace dependency table intact. Confirm these relevant
entries and add only missing entries or missing features:

```toml
[workspace.dependencies]
async-trait           = { version = "0.1.89" }
clap                  = { version = "4.6.1", features = ["derive"] }
error-location        = { version = "0.1.0" }
futures               = { version = "0.3.32" }
protobuf              = { version = "3.7.2" }
protobuf-json-mapping = { version = "3.7.2" }
rmcp                  = { version = "1.7.0", default-features = false, features = ["server", "macros", "schemars", "transport-io"] }
schemars              = { version = "1.2.1", features = ["derive"] }
serde                 = { version = "1.0.228", features = ["derive"] }
serde_json            = { version = "1.0.150" }
sha2                  = { version = "0.11.0" }
tempfile              = { version = "3.27.0" }
thiserror             = { version = "2.0.18" }
tokio                 = { version = "1.52.3", features = ["io-util", "macros", "process", "rt-multi-thread"] }

rr-core               = { path = "crates/rr-core" }
rr-report             = { path = "crates/rr-report" }
rr-scip               = { path = "crates/rr-scip" }
scip                  = { path = "submodules/scip/bindings/rust" }
```

`crates/rr-mcp/Cargo.toml`:

Keep the existing `rr-mcp` dependency table intact. Confirm these relevant
entries and add only missing entries or missing features:

```toml
[dependencies]
async-trait    = { workspace = true }
clap           = { workspace = true }
error-location = { workspace = true }
futures        = { workspace = true }
rmcp           = { workspace = true }
schemars       = { workspace = true }
serde          = { workspace = true }
serde_json     = { workspace = true }
sha2           = { workspace = true }
tempfile       = { workspace = true }
thiserror      = { workspace = true }
tokio          = { workspace = true }

rr-core        = { workspace = true }
rr-report      = { workspace = true }
rr-scip        = { workspace = true }

[dev-dependencies]
protobuf       = { workspace = true }
rmcp           = { workspace = true, features = ["client"] }
scip           = { workspace = true }
```

Do not add:

```toml
anyhow = "..."
reqwest = "..."
async-openai = "..."
openai = "..."
```

Validation:

```bash
cargo check -p rr-mcp --all-targets
```

### 2. Runtime CLI Config

Add model-hint and concurrency flags to the MCP binary.

`crates/rr-mcp/src/bin/rr-mcp.rs`:

```rust
use rr_mcp::{McpError, McpResult, McpServer};

use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    rust_analyzer_path: Option<PathBuf>,

    #[arg(long)]
    brief_model: Option<String>,

    #[arg(long)]
    max_concurrent_requests: Option<usize>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run RefactorRadar MCP server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> McpResult<()> {
    let args = Args::parse();

    let service = McpServer::with_runtime_config(
        args.rust_analyzer_path,
        args.brief_model,
        args.max_concurrent_requests,
    )
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

Validation:

```bash
cargo run -p rr-mcp -- --help
```

Expected help contains:

```text
--brief-model
--max-concurrent-requests
```

### 3. Tool Parameter Shape

Ensure the MCP tool params include model and concurrency knobs.

`crates/rr-mcp/src/llm_brief_params.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmBriefParams {
    pub project_id: String,
    pub symbol_ids: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub reference_limit: Option<usize>,
    pub include_inferred: Option<bool>,
    pub budget_tokens: Option<usize>,
    pub brief_model: Option<String>,
    pub max_concurrent_requests: Option<usize>,
}
```

Validation:

```bash
rg -n "brief_model|max_concurrent_requests|deny_unknown_fields" crates/rr-mcp/src/llm_brief_params.rs
```

### 3a. Project-Qualified Element Brief Params

Make deterministic element-brief calls optionally project-qualified before the
work-plan tool starts emitting them. The existing resolver searches every
cached project and can return an ambiguity error when two cached projects share
the same SCIP symbol. Work-plan task recipes must include `project_id` so the
parent Codex session can replay exact deterministic calls without depending on
single-project cache state.

`crates/rr-mcp/src/element_brief_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ElementBriefParams {
    pub symbol_id: String,
    pub project_id: Option<String>,
    pub include_inferred: Option<bool>,
    pub reference_limit: Option<usize>,
}
```

Update the existing `get_element_brief` handler in
`crates/rr-mcp/src/mcp_server.rs` so `project_id` scopes lookup when present
and the current cross-project resolver remains the compatibility path when it
is absent:

```rust
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
```

Validation:

```bash
rg -n "project_id" crates/rr-mcp/src/element_brief_params.rs crates/rr-mcp/src/mcp_server.rs
cargo test -p rr-mcp mcp_server
```

### 4. Decentralized Work-Plan Params

Add params for a deterministic MCP tool that builds the Codex sub-agent work
package. This tool must not call sampling. It prepares work for Codex to
distribute.

`crates/rr-mcp/src/brief_work_plan_params.rs`:

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
    pub brief_model: Option<String>,
}
```

Validation:

```bash
rg -n "BriefWorkPlanParams|max_subagent_tasks|preferred_agent|deny_unknown_fields" crates/rr-mcp/src/brief_work_plan_params.rs
```

### 5. Decentralized Work-Plan Data Shapes

Add one type per file. These types are the contract between `rr-mcp` and the
Codex parent session. They make decentralized work explicit instead of relying
on prose.

`crates/rr-mcp/src/brief_generation_capabilities.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BriefGenerationCapabilities {
    pub sampling_supported: bool,
    pub legacy_sampling_supported: bool,
    pub task_sampling_create_message_supported: bool,
    pub generate_llm_brief_available: bool,
    pub deterministic_work_plan_available: bool,
    pub fallback_tool: String,
    pub unsupported_error_message: String,
    pub notes: Vec<String>,
}
```

`crates/rr-mcp/src/brief_task_tool_call.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefTaskToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}
```

`crates/rr-mcp/src/brief_task_output_schema.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefTaskOutputSchema {
    pub format: String,
    pub required_fields: Vec<String>,
    pub json_schema: serde_json::Value,
}
```

`crates/rr-mcp/src/brief_work_task.rs`:

```rust
use crate::{BriefTaskOutputSchema, BriefTaskToolCall};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefWorkTask {
    pub task_id: String,
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

`crates/rr-mcp/src/brief_work_plan.rs`:

```rust
use crate::{BriefGenerationCapabilities, BriefWorkTask};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefWorkPlan {
    pub project_id: String,
    pub prompt_version: String,
    pub execution_model: String,
    pub generation_capabilities: BriefGenerationCapabilities,
    pub parent_instructions: Vec<String>,
    pub tasks: Vec<BriefWorkTask>,
    pub merge_instructions: Vec<String>,
    pub verification_commands: Vec<String>,
    pub unknowns: Vec<String>,
}
```

Required semantic meaning:

```text
execution_model:
  "codex-parent-spawns-subagents"

parent_instructions:
  must state that Codex, not rr-mcp, spawns one sub-agent per task

tasks[].tool_calls:
  exact MCP calls each sub-agent should make

tasks[].merge_key:
  stable key the parent uses to merge task outputs

tasks[].expected_output_schema:
  JSON contract each sub-agent must return to the parent

generation_capabilities:
  current MCP host support for sampling/createMessage, plus deterministic fallback
```

Validation:

```bash
rg -n "^(pub )?(struct|enum|trait|type) " crates/rr-mcp/src/brief_generation_capabilities.rs crates/rr-mcp/src/brief_task_tool_call.rs crates/rr-mcp/src/brief_task_output_schema.rs crates/rr-mcp/src/brief_work_task.rs crates/rr-mcp/src/brief_work_plan.rs crates/rr-mcp/src/brief_work_plan_params.rs
```

Expected output has exactly one type line per listed file.

### 6. Generated-Brief Data Shapes

Add one type per source file under `crates/rr-mcp/src/`.

`crates/rr-mcp/src/brief_cache_key.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BriefCacheKey {
    pub project_id: String,
    pub symbol_id: String,
    pub evidence_hash: String,
    pub prompt_version: String,
    pub brief_model: String,
}
```

`crates/rr-mcp/src/generated_five_w.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFiveW {
    pub who: String,
    pub what: String,
    pub when: String,
    #[serde(rename = "where")]
    pub where_: String,
    pub why: String,
    pub how: String,
}
```

`crates/rr-mcp/src/generated_brief_content.rs`:

```rust
use crate::GeneratedFiveW;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedBriefContent {
    pub summary: String,
    pub five_w: GeneratedFiveW,
    pub unknowns: Vec<String>,
}
```

`crates/rr-mcp/src/generated_brief.rs`:

```rust
use crate::{BriefCacheKey, GeneratedFiveW};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeneratedBrief {
    pub summary: String,
    pub five_w: GeneratedFiveW,
    pub unknowns: Vec<String>,
    pub evidence_hash: String,
    pub cache_key: BriefCacheKey,
    pub generated_by_model: String,
    pub prompt_version: String,
}
```

`crates/rr-mcp/src/generated_brief_cache_entry.rs`:

```rust
use crate::GeneratedBrief;

#[derive(Clone, Debug)]
pub struct GeneratedBriefCacheEntry {
    pub brief: GeneratedBrief,
    pub evidence_hash: String,
    pub prompt_version: String,
    pub model_hint: String,
    pub generated_by_model: String,
}
```

`crates/rr-mcp/src/brief_llm_request.rs`:

```rust
#[derive(Clone, Debug)]
pub struct BriefLlmRequest {
    pub model_hint: Option<String>,
    pub instructions: String,
    pub evidence_packet: serde_json::Value,
    pub max_tokens: u32,
}
```

`crates/rr-mcp/src/brief_llm_client.rs`:

```rust
use crate::{BriefLlmRequest, GeneratedBriefContent, McpResult};

#[async_trait::async_trait]
pub trait BriefLlmClient: Send + Sync {
    async fn generate(
        &self,
        request: BriefLlmRequest,
    ) -> McpResult<(GeneratedBriefContent, String)>;
}
```

Validation:

```bash
rg -n "^(pub )?(struct|enum|trait|type) " crates/rr-mcp/src/brief_cache_key.rs crates/rr-mcp/src/generated_five_w.rs crates/rr-mcp/src/generated_brief_content.rs crates/rr-mcp/src/generated_brief.rs crates/rr-mcp/src/generated_brief_cache_entry.rs crates/rr-mcp/src/brief_llm_request.rs crates/rr-mcp/src/brief_llm_client.rs
```

Expected output has exactly one type line per listed file.

### 7. Prompt Template

Keep prompt text in a constants-only file, not in a type file.

`crates/rr-mcp/src/special/mod.rs`:

```rust
pub mod brief_llm_user_prompt;
```

`crates/rr-mcp/src/special/brief_llm_user_prompt.rs`:

```rust
pub const GENERATED_BRIEF_USER_PROMPT_TEMPLATE: &str = "\
Return only a JSON object matching this schema:
{\"summary\":\"string\",\"five_w\":{\"who\":\"string\",\"what\":\"string\",\"when\":\"string\",\"where\":\"string\",\"why\":\"string\",\"how\":\"string\"},\"unknowns\":[\"string\"]}

Use only this evidence packet. Unsupported facts must go in unknowns.

Evidence packet:
{evidence_json}";
```

Validation:

```bash
rg -n "pub struct|pub enum|pub trait|pub type" crates/rr-mcp/src/special -g '*.rs'
rg -n "GENERATED_BRIEF_USER_PROMPT_TEMPLATE" crates/rr-mcp/src/special crates/rr-mcp/src
```

Expected first command:

```text
<no matches>
```

### 8. Typed MCP Errors

Extend the existing crate-local error enum. Do not introduce `anyhow`.

`crates/rr-mcp/src/error.rs`:

```rust
#[derive(Debug, Error)]
pub enum McpError {
    // keep existing variants

    #[error("{message} at {location}")]
    BriefSamplingUnavailable {
        message: &'static str,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    BriefGenerationRequestFailed {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    BriefGenerationResponseInvalid {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
}
```

Add constructors in the same `impl McpError`:

```rust
#[track_caller]
pub fn brief_sampling_unavailable() -> Self {
    Self::BriefSamplingUnavailable {
        message: "MCP client does not advertise sampling support",
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn brief_generation_request_failed(details: impl Into<String>) -> Self {
    Self::BriefGenerationRequestFailed {
        message: "brief sampling request failed",
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}

#[track_caller]
pub fn brief_generation_response_invalid(details: impl Into<String>) -> Self {
    Self::BriefGenerationResponseInvalid {
        message: "brief sampling response was invalid",
        details: details.into(),
        location: ErrorLocation::from(Location::caller()),
    }
}
```

Update `message()`:

```rust
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
        | Self::ServerRuntime { message, .. }
        | Self::BriefSamplingUnavailable { message, .. }
        | Self::BriefGenerationRequestFailed { message, .. }
        | Self::BriefGenerationResponseInvalid { message, .. } => message,
    }
}
```

Update protocol conversion.

`crates/rr-mcp/src/error_conversion.rs`:

```rust
McpError::BriefSamplingUnavailable { message, .. } => {
    ProtocolError::invalid_request(message, None)
}
McpError::SerializationFailed {
    message, details, ..
}
| McpError::ReportGenerationFailed {
    message, details, ..
}
| McpError::ServerRuntime {
    message, details, ..
}
| McpError::BriefGenerationRequestFailed {
    message, details, ..
}
| McpError::BriefGenerationResponseInvalid {
    message, details, ..
} => ProtocolError::internal_error(message, Some(json!({ "details": details }))),
```

Validation:

```bash
rg -n "BriefSamplingUnavailable|brief_sampling_unavailable|BriefGenerationRequestFailed|BriefGenerationResponseInvalid" crates/rr-mcp/src/error.rs crates/rr-mcp/src/error_conversion.rs
```

### 9. Sampling Adapter

Add the production host-mediated LLM client.

`crates/rr-mcp/src/sampling_brief_llm_client.rs`:

```rust
use crate::{
    BriefLlmClient, BriefLlmRequest, GENERATED_BRIEF_USER_PROMPT_TEMPLATE, GeneratedBriefContent,
    McpError, McpResult,
};

use rmcp::{
    Peer, RoleServer,
    model::{
        ContextInclusion, CreateMessageRequestParams, CreateMessageResult, ModelHint,
        ModelPreferences, Role, SamplingMessage, SamplingMessageContent,
    },
};

#[derive(Clone, Debug)]
pub struct SamplingBriefLlmClient {
    peer: Peer<RoleServer>,
}

impl SamplingBriefLlmClient {
    pub fn new(peer: Peer<RoleServer>) -> Self {
        Self { peer }
    }
}

#[async_trait::async_trait]
impl BriefLlmClient for SamplingBriefLlmClient {
    async fn generate(
        &self,
        request: BriefLlmRequest,
    ) -> McpResult<(GeneratedBriefContent, String)> {
        let supports_sampling = self
            .peer
            .peer_info()
            .map(|info| {
                info.capabilities.sampling.is_some()
                    || info
                        .capabilities
                        .tasks
                        .as_ref()
                        .map(|tasks| tasks.supports_sampling_create_message())
                        .unwrap_or(false)
            })
            .unwrap_or(false);

        if !supports_sampling {
            return Err(McpError::brief_sampling_unavailable());
        }

        let evidence_json = serde_json::to_string_pretty(&request.evidence_packet)
            .map_err(|error| McpError::serialization_failed(error.to_string()))?;
        let user_prompt =
            GENERATED_BRIEF_USER_PROMPT_TEMPLATE.replace("{evidence_json}", &evidence_json);

        let mut params = CreateMessageRequestParams::new(
            vec![SamplingMessage::user_text(user_prompt)],
            request.max_tokens,
        )
        .with_system_prompt(request.instructions)
        .with_include_context(ContextInclusion::None)
        .with_temperature(0.0);

        if let Some(model_hint) = request.model_hint {
            params = params.with_model_preferences(
                ModelPreferences::new().with_hints(vec![ModelHint::new(model_hint)]),
            );
        }

        let result = self
            .peer
            .create_message(params)
            .await
            .map_err(|error| McpError::brief_generation_request_failed(error.to_string()))?;
        let output_text = assistant_text(&result).ok_or_else(|| {
            McpError::brief_generation_response_invalid(
                "sampling response did not contain assistant text",
            )
        })?;
        let content = parse_generated_brief_content(output_text)?;

        Ok((content, result.model))
    }
}

fn assistant_text(result: &CreateMessageResult) -> Option<&str> {
    if result.message.role != Role::Assistant {
        return None;
    }

    result.message.content.iter().find_map(|content| match content {
        SamplingMessageContent::Text(text) => Some(text.text.as_str()),
        _ => None,
    })
}

fn parse_generated_brief_content(output_text: &str) -> McpResult<GeneratedBriefContent> {
    serde_json::from_str::<GeneratedBriefContent>(output_text)
        .map_err(|error| McpError::brief_generation_response_invalid(error.to_string()))
}

#[cfg(test)]
#[test]
fn given_invalid_sampling_json_when_parsing_then_typed_error_is_returned() {
    let error = parse_generated_brief_content("not json").err();

    assert!(matches!(
        error,
        Some(McpError::BriefGenerationResponseInvalid { .. })
    ));
}
```

This file has one type: `SamplingBriefLlmClient`.

Validation:

```bash
rg -n "^(pub )?(struct|enum|trait|type) " crates/rr-mcp/src/sampling_brief_llm_client.rs
cargo test -p rr-mcp sampling_brief_llm_client
```

### 10. Library Exports

Export the new modules and types.

`crates/rr-mcp/src/lib.rs`:

```rust
pub mod brief_cache_key;
pub mod brief_generation_capabilities;
pub mod brief_llm_client;
pub mod brief_llm_request;
pub mod brief_task_output_schema;
pub mod brief_task_tool_call;
pub mod brief_work_plan;
pub mod brief_work_plan_params;
pub mod brief_work_task;
pub mod generated_brief;
pub mod generated_brief_cache_entry;
pub mod generated_brief_content;
pub mod generated_five_w;
pub mod sampling_brief_llm_client;
pub mod special;

pub(crate) use special::brief_llm_user_prompt::GENERATED_BRIEF_USER_PROMPT_TEMPLATE;

pub use brief_cache_key::BriefCacheKey;
pub use brief_generation_capabilities::BriefGenerationCapabilities;
pub use brief_llm_client::BriefLlmClient;
pub use brief_llm_request::BriefLlmRequest;
pub use brief_task_output_schema::BriefTaskOutputSchema;
pub use brief_task_tool_call::BriefTaskToolCall;
pub use brief_work_plan::BriefWorkPlan;
pub use brief_work_plan_params::BriefWorkPlanParams;
pub use brief_work_task::BriefWorkTask;
pub use generated_brief::GeneratedBrief;
pub use generated_brief_cache_entry::GeneratedBriefCacheEntry;
pub use generated_brief_content::GeneratedBriefContent;
pub use generated_five_w::GeneratedFiveW;
pub use sampling_brief_llm_client::SamplingBriefLlmClient;
```

Keep existing modules and exports in place.

Validation:

```bash
rg -n "brief_cache_key|BriefGenerationCapabilities|BriefWorkPlan|BriefWorkTask|GeneratedBrief|SamplingBriefLlmClient|GENERATED_BRIEF_USER_PROMPT_TEMPLATE" crates/rr-mcp/src/lib.rs
```

### 11. Generated-Brief Cache

Extend the project cache with generated briefs.

`crates/rr-mcp/src/project_cache.rs`:

```rust
use crate::{BriefCacheKey, GeneratedBriefCacheEntry};
use rr_core::SemanticModel;

use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Default)]
pub struct ProjectCache {
    pub models: HashMap<String, SemanticModel>,
    pub scip_paths: HashMap<String, PathBuf>,
    pub temporary_scip_paths: HashMap<String, tempfile::TempPath>,
    pub producer_metadata: HashMap<String, String>,
    pub generation_diagnostics: HashMap<String, String>,
    pub generated_briefs: HashMap<BriefCacheKey, GeneratedBriefCacheEntry>,
}
```

Do not clear `generated_briefs` on project insert. Cache misses are driven by
`project_id`, `symbol_id`, `evidence_hash`, `prompt_version`, and model hint key.

Validation:

```bash
rg -n "generated_briefs|BriefCacheKey|GeneratedBriefCacheEntry" crates/rr-mcp/src/project_cache.rs crates/rr-mcp/src/mcp_server.rs
```

### 12. MCP Server Config Fields

Add runtime config fields and constructors.

`crates/rr-mcp/src/mcp_server.rs`:

```rust
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
```

Add constants:

```rust
const LLM_BRIEF_PROMPT_VERSION: &str = "5w-summary-v1";
const LLM_BRIEF_INSTRUCTIONS: &str = "Generate one concise 5W summary per item using only evidence_packet facts. Preserve confirmed, inferred, and unknown distinctions. Use unknown when the evidence does not support a claim.";
```

Update `McpServer`:

```rust
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
```

Validation:

```bash
rg -n "brief_model|max_concurrent_requests|with_runtime_config|LLM_BRIEF_PROMPT_VERSION" crates/rr-mcp/src/mcp_server.rs crates/rr-mcp/src/bin/rr-mcp.rs
```

### 13. MCP Server Helpers

Add helpers below the server impl before adding the context-aware tool handlers
that call them.

`crates/rr-mcp/src/mcp_server.rs`:

```rust
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

fn brief_generation_capabilities_from_peer(peer: &rmcp::Peer<RoleServer>) -> BriefGenerationCapabilities {
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
    let sampling_supported =
        legacy_sampling_supported || task_sampling_create_message_supported;
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
    let chunk_size = ((elements.len() + max_tasks - 1) / max_tasks).max(1);
    let preferred_agent = params
        .preferred_agent
        .clone()
        .unwrap_or_else(|| "explorer".to_owned());
    let budget_tokens = params.budget_tokens.unwrap_or(800);
    let reference_limit = params.reference_limit.unwrap_or(5);
    let include_inferred = params.include_inferred.unwrap_or(true);
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
                budget_tokens,
                reference_limit,
                include_inferred,
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
    budget_tokens: usize,
    reference_limit: usize,
    include_inferred: bool,
    include_generated_brief_call: bool,
) -> BriefWorkTask {
    let task_number = index + 1;
    let task_id = format!("brief-task-{task_number:03}");
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
```

Validation:

```bash
rg -n "fn evidence_hash|fn select_llm_brief_elements|fn brief_generation_capabilities_from_peer|fn generated_brief_from_content|fn select_brief_work_plan_elements|fn build_brief_work_plan|fn build_brief_work_task|fn brief_work_tool_calls|fn brief_task_output_schema" crates/rr-mcp/src/mcp_server.rs
```

### 14. Host Capability Probe Tool

Add a deterministic MCP tool that tells Codex whether the current MCP host
advertises `sampling/createMessage`. This resolves host capability detection at
runtime.

`crates/rr-mcp/src/mcp_server.rs`:

```rust
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
```

Expected response when the host supports sampling:

```json
{
  "sampling_supported": true,
  "legacy_sampling_supported": true,
  "task_sampling_create_message_supported": false,
  "generate_llm_brief_available": true,
  "deterministic_work_plan_available": true,
  "fallback_tool": "generate_brief_work_plan",
  "unsupported_error_message": "MCP client does not advertise sampling support",
  "notes": [
    "generate_llm_brief may request sampling/createMessage from this host."
  ]
}
```

Expected response when the host does not support sampling:

```json
{
  "sampling_supported": false,
  "legacy_sampling_supported": false,
  "task_sampling_create_message_supported": false,
  "generate_llm_brief_available": false,
  "deterministic_work_plan_available": true,
  "fallback_tool": "generate_brief_work_plan",
  "unsupported_error_message": "MCP client does not advertise sampling support",
  "notes": [
    "Use generate_brief_work_plan and Codex-managed sub-agents instead of generate_llm_brief."
  ]
}
```

Validation:

```bash
rg -n "get_brief_generation_capabilities|brief_generation_capabilities_from_peer|sampling_supported" crates/rr-mcp/src/mcp_server.rs
```

### 15. Decentralized Work-Plan Tool Handler

Add a deterministic MCP tool that returns a Codex-ready sub-agent work package.
This tool does not call sampling and does not spawn agents. It returns the exact
manifest the Codex parent session should use to spawn sub-agents.

`crates/rr-mcp/src/mcp_server.rs`:

```rust
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
    let elements = select_brief_work_plan_elements(model, &params).map_err(to_protocol_error)?;
    let plan = build_brief_work_plan(model, &params, elements, generation_capabilities);

    structured(plan).map_err(to_protocol_error)
}
```

The returned plan must be explicit enough for Codex to act without inventing
task boundaries:

When `generation_capabilities.generate_llm_brief_available` is `false`, task
tool calls must stay deterministic and must not include `generate_llm_brief`.
Include `generate_llm_brief` task calls only when the same request context
advertises sampling support.

```json
{
  "project_id": "fixture",
  "prompt_version": "5w-summary-v1",
  "execution_model": "codex-parent-spawns-subagents",
  "generation_capabilities": {
    "sampling_supported": false,
    "generate_llm_brief_available": false,
    "deterministic_work_plan_available": true,
    "fallback_tool": "generate_brief_work_plan",
    "unsupported_error_message": "MCP client does not advertise sampling support"
  },
  "parent_instructions": [
    "Spawn one Codex sub-agent per task_id.",
    "Each sub-agent must execute only the MCP tool calls listed in its task.",
    "The MCP server does not spawn or manage Codex sub-agents."
  ],
  "tasks": [
    {
      "task_id": "brief-task-001",
      "title": "Build focused brief for public_sum",
      "preferred_agent": "explorer",
      "scope": "One independent symbol group from project fixture.",
      "symbol_ids": ["rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum()."],
      "tool_calls": [
        {
          "tool_name": "get_element_brief",
          "arguments": {
            "symbol_id": "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().",
            "project_id": "fixture",
            "include_inferred": true,
            "reference_limit": 5
          }
        }
      ],
      "expected_output_schema": {
        "format": "json",
        "required_fields": [
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
        "json_schema": {
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
        }
      },
      "merge_key": "fixture::brief-task-001",
      "budget_tokens": 800,
      "dependencies": [],
      "acceptance_criteria": [
        "Use only evidence returned by listed MCP tool calls.",
        "Separate confirmed facts from inference.",
        "Put unsupported facts in unknowns."
      ]
    }
  ],
  "merge_instructions": [
    "Merge task outputs by merge_key.",
    "Preserve confirmed, inferred, and unknown distinctions.",
    "Do not collapse different symbol scopes into one claim unless both task outputs support it."
  ],
  "verification_commands": [
    "cargo test -p rr-mcp",
    "cargo check -p rr-mcp --all-targets"
  ],
  "unknowns": [
    "Codex host sub-agent execution is outside rr-mcp and must be explicitly requested by the parent Codex session."
  ]
}
```

Validation:

```bash
rg -n "generate_brief_work_plan|generate_brief_work_plan_with_capabilities|BriefWorkPlanParams|build_brief_work_plan|generation_capabilities|codex-parent-spawns-subagents" crates/rr-mcp/src/mcp_server.rs
```

### 16. Generated Brief Tool Handler And Internal Helper

Change `generate_llm_brief` to use request-context sampling and a testable
internal helper.

`crates/rr-mcp/src/mcp_server.rs`:

```rust
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
```

Do not return `instructions`, `summary_schema`, or `evidence_packet` in the
generated response. Raw evidence remains available through deterministic tools.

Validation:

```bash
rg -n "RequestContext<RoleServer>|generate_llm_brief_with_client|SamplingBriefLlmClient|summary_schema|evidence_packet" crates/rr-mcp/src/mcp_server.rs
```

Expected:

```text
RequestContext<RoleServer> is present
generate_llm_brief_with_client is present
SamplingBriefLlmClient is present
summary_schema is absent from generated response
evidence_packet is only used while building requests, not returned
```

### 17. Unit Tests For Generation Behavior

Use fake clients for unit tests. Do not call an MCP host here.

`crates/rr-mcp/src/mcp_server/tests.rs`:

```rust
use crate::{
    BriefGenerationCapabilities, BriefLlmClient, BriefLlmRequest, BriefWorkPlanParams,
    GeneratedBriefContent, GeneratedFiveW, LlmBriefParams, McpError, McpResult, McpServer,
    ScipProjectParams,
};

use rmcp::handler::server::wrapper::{Json, Parameters};
use std::sync::Arc;

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
```

If this file already has `FakeBriefLlmClient`, update it instead of adding a
second type.

Required unit test coverage:

```rust
#[tokio::test]
async fn given_loaded_project_when_generating_brief_work_plan_then_codex_subagent_tasks_are_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::with_runtime_config(None, None, None);
    load_fixture_project(&server).await?;

    let Json(result) = server
        .generate_brief_work_plan_with_capabilities(
            BriefWorkPlanParams {
                project_id: "fixture".to_owned(),
                symbol_ids: Some(vec![SYMBOL.to_owned()]),
                limit: None,
                reference_limit: Some(3),
                include_inferred: Some(true),
                budget_tokens: Some(500),
                max_subagent_tasks: Some(4),
                preferred_agent: Some("explorer".to_owned()),
                brief_model: Some("test-model-hint".to_owned()),
            },
            unsupported_sampling_capabilities(),
        )
        .await
        .map_err(to_io_error)?;

    assert_eq!("fixture", result["project_id"]);
    assert_eq!("5w-summary-v1", result["prompt_version"]);
    assert_eq!(
        "codex-parent-spawns-subagents",
        result["execution_model"]
    );
    assert_eq!(false, result["generation_capabilities"]["sampling_supported"]);
    assert_eq!(
        "generate_brief_work_plan",
        result["generation_capabilities"]["fallback_tool"]
    );
    assert_eq!("brief-task-001", result["tasks"][0]["task_id"]);
    assert_eq!("explorer", result["tasks"][0]["preferred_agent"]);
    assert_eq!(SYMBOL, result["tasks"][0]["symbol_ids"][0]);
    assert_eq!(
        "get_element_brief",
        result["tasks"][0]["tool_calls"][0]["tool_name"]
    );
    assert_eq!(
        "fixture",
        result["tasks"][0]["tool_calls"][0]["arguments"]["project_id"]
    );
    assert_eq!(
        1,
        result["tasks"][0]["tool_calls"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default()
    );
    assert_eq!(
        "fixture::brief-task-001",
        result["tasks"][0]["merge_key"]
    );
    assert!(result["parent_instructions"]
        .as_array()
        .map(|items| items.iter().any(|item| {
            item.as_str()
                .map(|text| text.contains("MCP server does not spawn"))
                .unwrap_or(false)
        }))
        .unwrap_or(false));
    Ok(())
}

fn unsupported_sampling_capabilities() -> BriefGenerationCapabilities {
    BriefGenerationCapabilities {
        sampling_supported: false,
        legacy_sampling_supported: false,
        task_sampling_create_message_supported: false,
        generate_llm_brief_available: false,
        deterministic_work_plan_available: true,
        fallback_tool: "generate_brief_work_plan".to_owned(),
        unsupported_error_message: "MCP client does not advertise sampling support".to_owned(),
        notes: vec!["Use deterministic work-plan path in this test.".to_owned()],
    }
}
```

```rust
#[tokio::test]
async fn given_request_model_hint_when_generating_llm_brief_then_generated_summary_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let server = McpServer::with_runtime_config(None, None, None);
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
    assert_eq!("test-model-hint", result["items"][0]["generated_by_model"]);

    let calls = fake.calls.lock().await;
    assert_eq!(1, calls.len());
    assert_eq!(Some("test-model-hint"), calls[0].model_hint.as_deref());
    assert_eq!(500, calls[0].max_tokens);
    Ok(())
}
```

Add focused tests for:

```text
- work plan returns execution_model = codex-parent-spawns-subagents
- work plan embeds the injected generation_capabilities used by the internal helper
- work plan contains one task per selected chunk and respects max_subagent_tasks
- project-qualified get_element_brief preserves existing stable ID, raw SCIP
  symbol, and scip:-prefixed SCIP symbol lookup behavior
- each work-plan task contains project-qualified get_element_brief tool calls for scoped symbols
- unsupported-host work-plan tasks omit generate_llm_brief and rely on deterministic get_element_brief calls
- sampling-capable work-plan tasks contain a generate_llm_brief call scoped to the same symbols
- work-plan task output schema lists required merge fields
- work-plan parent instructions explicitly say rr-mcp does not spawn sub-agents
- server-level brief_model fallback
- host-default cache key when no model hint exists
- request concurrency overriding server config
- server concurrency when request omits it
- zero concurrency clamped to one
- cache hit avoids a second fake-client call
- different model hint causes cache miss
- changed evidence hash causes cache miss
- concurrency cap is respected
- invalid fake response maps to BriefGenerationResponseInvalid
```

Validation:

```bash
cargo test -p rr-mcp mcp_server
```

### 18. MCP Protocol Integration Tests

These tests verify the real MCP boundary, not just the internal helper.

Create support files:

```bash
mkdir -p crates/rr-mcp/tests/support
```

`crates/rr-mcp/tests/support/mod.rs`:

```rust
pub mod fixture_scip;
pub mod sampling_test_client;
```

`crates/rr-mcp/tests/support/fixture_scip.rs`:

```rust
use protobuf::{Message, MessageField};
use scip::types::{
    Document, Index, Metadata, Occurrence, SymbolInformation, SymbolRole, ToolInfo,
    symbol_information,
};
use tempfile::tempdir;

pub const PROJECT_ID: &str = "fixture";
pub const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";

pub fn write_fixture_scip()
-> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;
    Ok((directory, path))
}

fn sample_index() -> Index {
    Index {
        metadata: MessageField::some(Metadata {
            project_root: PROJECT_ID.to_owned(),
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
            occurrences: vec![
                Occurrence {
                    range: vec![0, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: SymbolRole::Definition as i32,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![1, 0, 10],
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
```

`crates/rr-mcp/tests/support/sampling_test_client.rs`:

```rust
use rmcp::{
    ClientHandler, ErrorData as ProtocolError, RoleClient,
    model::{
        ClientCapabilities, ClientInfo, CreateMessageRequestParams, CreateMessageResult,
        Implementation, SamplingMessage,
    },
    service::RequestContext,
};

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
pub struct SamplingTestClient {
    capabilities: ClientCapabilities,
    sampling_calls: Arc<AtomicUsize>,
}

impl SamplingTestClient {
    pub fn with_sampling() -> Self {
        Self {
            capabilities: ClientCapabilities::builder().enable_sampling().build(),
            sampling_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn without_sampling() -> Self {
        Self {
            capabilities: ClientCapabilities::default(),
            sampling_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn sampling_call_count(&self) -> usize {
        self.sampling_calls.load(Ordering::SeqCst)
    }
}

impl ClientHandler for SamplingTestClient {
    async fn create_message(
        &self,
        _params: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ProtocolError> {
        self.sampling_calls.fetch_add(1, Ordering::SeqCst);
        let json = serde_json::json!({
            "summary": "generated by protocol test client",
            "five_w": {
                "who": "unknown",
                "what": "generated from MCP sampling request",
                "when": "unknown",
                "where": "src/lib.rs",
                "why": "unknown",
                "how": "deterministic integration test response"
            },
            "unknowns": ["runtime behavior is unknown from SCIP"]
        });

        Ok(CreateMessageResult::new(
            SamplingMessage::assistant_text(json.to_string()),
            "protocol-test-model".to_owned(),
        )
        .with_stop_reason(CreateMessageResult::STOP_REASON_END_TURN))
    }

    fn get_info(&self) -> ClientInfo {
        ClientInfo::new(
            self.capabilities.clone(),
            Implementation::new("rr-mcp-protocol-test-client", "0.0.0"),
        )
    }
}
```

`crates/rr-mcp/tests/generated_brief_protocol.rs`:

```rust
mod support;

use support::{
    fixture_scip::{PROJECT_ID, SYMBOL, write_fixture_scip},
    sampling_test_client::SamplingTestClient,
};

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, JsonObject},
};
use rr_mcp::McpServer;

#[tokio::test]
async fn given_sampling_client_when_getting_generation_capabilities_then_sampling_is_available()
-> Result<(), Box<dyn std::error::Error>> {
    let client_handler = SamplingTestClient::with_sampling();
    let client = start_protocol_pair(client_handler).await?;

    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("get_brief_generation_capabilities"))
        .await?;
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;

    assert_eq!(true, structured["sampling_supported"]);
    assert_eq!(true, structured["generate_llm_brief_available"]);
    assert_eq!(true, structured["deterministic_work_plan_available"]);

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn given_non_sampling_client_when_getting_generation_capabilities_then_work_plan_fallback_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let client_handler = SamplingTestClient::without_sampling();
    let client = start_protocol_pair(client_handler).await?;

    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("get_brief_generation_capabilities"))
        .await?;
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;

    assert_eq!(false, structured["sampling_supported"]);
    assert_eq!(false, structured["generate_llm_brief_available"]);
    assert_eq!(true, structured["deterministic_work_plan_available"]);
    assert_eq!("generate_brief_work_plan", structured["fallback_tool"]);
    assert_eq!(
        "MCP client does not advertise sampling support",
        structured["unsupported_error_message"]
    );

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn given_mcp_client_when_generating_brief_work_plan_then_subagent_manifest_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let client_handler = SamplingTestClient::without_sampling();
    let client = start_protocol_pair(client_handler.clone()).await?;

    load_fixture_project(&client).await?;
    let arguments = json_object(serde_json::json!({
        "project_id": PROJECT_ID,
        "symbol_ids": [SYMBOL],
        "reference_limit": 1,
        "include_inferred": true,
        "budget_tokens": 500,
        "max_subagent_tasks": 4,
        "preferred_agent": "explorer",
        "brief_model": "test-model-hint"
    }))?;
    let result = client
        .peer()
        .call_tool(CallToolRequestParams::new("generate_brief_work_plan").with_arguments(arguments))
        .await?;

    assert_eq!(Some(false), result.is_error);
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;

    assert_eq!("codex-parent-spawns-subagents", structured["execution_model"]);
    assert_eq!(false, structured["generation_capabilities"]["sampling_supported"]);
    assert_eq!(
        "generate_brief_work_plan",
        structured["generation_capabilities"]["fallback_tool"]
    );
    assert_eq!("brief-task-001", structured["tasks"][0]["task_id"]);
    assert_eq!(
        "get_element_brief",
        structured["tasks"][0]["tool_calls"][0]["tool_name"]
    );
    assert_eq!(
        PROJECT_ID,
        structured["tasks"][0]["tool_calls"][0]["arguments"]["project_id"]
    );
    assert_eq!(
        1,
        structured["tasks"][0]["tool_calls"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default()
    );
    assert_eq!(0, client_handler.sampling_call_count());

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn given_sampling_client_when_generating_llm_brief_then_protocol_happy_path_returns_generated_items()
-> Result<(), Box<dyn std::error::Error>> {
    let client_handler = SamplingTestClient::with_sampling();
    let client = start_protocol_pair(client_handler.clone()).await?;

    load_fixture_project(&client).await?;
    let result = call_generate_llm_brief(&client, Some("test-model-hint")).await?;

    assert_eq!(Some(false), result.is_error);
    let structured = result
        .structured_content
        .ok_or_else(|| std::io::Error::other("missing structured content"))?;

    assert_eq!(PROJECT_ID, structured["project_id"]);
    assert_eq!("test-model-hint", structured["brief_model"]);
    assert_eq!("test-model-hint", structured["model_hint"]);
    assert_eq!("5w-summary-v1", structured["prompt_version"]);
    assert_eq!(
        "generated by protocol test client",
        structured["items"][0]["summary"]
    );
    assert_eq!(
        "generated from MCP sampling request",
        structured["items"][0]["five_w"]["what"]
    );
    assert_eq!(
        "protocol-test-model",
        structured["items"][0]["generated_by_model"]
    );
    assert_eq!(1, client_handler.sampling_call_count());

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn given_non_sampling_client_when_generating_llm_brief_then_typed_unsupported_host_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let client_handler = SamplingTestClient::without_sampling();
    let client = start_protocol_pair(client_handler.clone()).await?;

    load_fixture_project(&client).await?;
    let error = call_generate_llm_brief(&client, None)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("expected unsupported-host error"))?;
    let message = format!("{error:?}");

    assert!(message.contains("MCP client does not advertise sampling support"));
    assert_eq!(0, client_handler.sampling_call_count());

    client.cancel().await?;
    Ok(())
}

async fn start_protocol_pair(
    client_handler: SamplingTestClient,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, SamplingTestClient>, Box<dyn std::error::Error>>
{
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let _server_task = tokio::spawn(async move {
        if let Ok(server) = McpServer::new().serve(server_transport).await {
            let _ = server.waiting().await;
        }
    });

    let client = client_handler.serve(client_transport).await?;
    Ok(client)
}

async fn load_fixture_project(
    client: &rmcp::service::RunningService<rmcp::RoleClient, SamplingTestClient>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, path) = write_fixture_scip()?;
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

    Ok(())
}

async fn call_generate_llm_brief(
    client: &rmcp::service::RunningService<rmcp::RoleClient, SamplingTestClient>,
    brief_model: Option<&str>,
) -> Result<rmcp::model::CallToolResult, Box<dyn std::error::Error>> {
    let arguments = json_object(serde_json::json!({
        "project_id": PROJECT_ID,
        "symbol_ids": [SYMBOL],
        "reference_limit": 1,
        "include_inferred": true,
        "budget_tokens": 500,
        "brief_model": brief_model,
        "max_concurrent_requests": 1
    }))?;

    Ok(client
        .peer()
        .call_tool(CallToolRequestParams::new("generate_llm_brief").with_arguments(arguments))
        .await?)
}

fn json_object(
    value: serde_json::Value,
) -> Result<JsonObject, Box<dyn std::error::Error>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| std::io::Error::other("expected JSON object").into())
}
```

Validation:

```bash
cargo test -p rr-mcp --test generated_brief_protocol
```

### 19. README Capability Note

Document what the feature does and what Codex config does not guarantee.

`README.md`:

````markdown
## Host-Sampled LLM Briefs And Codex Work Plans

`generate_llm_brief` uses MCP host-mediated sampling. The `rr-mcp` server
selects cached RefactorRadar evidence and asks the connected MCP host to
generate JSON through `sampling/createMessage`.

`generate_brief_work_plan` is deterministic. It returns a Codex-ready task
manifest with one or more independent brief/summary tasks, exact MCP tool-call
recipes for each task, expected sub-agent output schemas, merge keys, and parent
merge instructions. Codex can use that manifest to spawn sub-agents, but
`rr-mcp` does not spawn or manage Codex sub-agents.

`get_brief_generation_capabilities` reports whether the connected MCP host
advertises sampling support. Call it before `generate_llm_brief`. If it reports
`sampling_supported: false`, use `generate_brief_work_plan` as the primary path.

`--brief-model` and `.codex/config.toml` configure the server and model hint
only. They do not make an MCP host advertise sampling support.

Verification status should be reported with these terms:

- `implemented`: server code compiles, unit tests pass, and fake-client generation works.
- `registered`: the MCP host can see and call the tool.
- `unsupported-host verified`: a host without sampling returns the typed unsupported-host error.
- `sampling happy path verified`: a sampling-capable MCP client completes `sampling/createMessage`.
- `Codex happy path verified`: Codex advertises sampling and completes the generated brief call.

If Codex does not advertise sampling, the expected `generate_llm_brief` result is
the typed unsupported-host error:

```text
MCP client does not advertise sampling support
```
````

Validation:

```bash
rg -n "Host-Sampled LLM Briefs And Codex Work Plans|get_brief_generation_capabilities|generate_brief_work_plan|unsupported-host verified|Codex happy path verified" README.md
```

## Validation

Run all checks in this order:

```bash
cargo fmt --check
cargo test -p rr-mcp sampling_brief_llm_client
cargo test -p rr-mcp mcp_server
cargo test -p rr-mcp --test generated_brief_protocol
cargo test -p rr-mcp
cargo check -p rr-mcp --all-targets
cargo test --workspace --all-targets
cargo build --release -p rr-mcp
```

Check dependency and provider boundaries:

```bash
rg -n "reqwest|async-openai|OPENAI_API_KEY|anyhow" Cargo.toml crates/rr-mcp crates/rr-core crates/rr-report crates/rr-scip
```

Check one-type-per-file:

```bash
rg -n "^(pub )?(struct|enum|trait|type) " crates/rr-mcp/src crates/rr-mcp/tests -g '*.rs'
```

For every new file, expected count is one or zero:

```text
brief_cache_key.rs: one struct
brief_generation_capabilities.rs: one struct
brief_llm_client.rs: one trait
brief_llm_request.rs: one struct
brief_task_output_schema.rs: one struct
brief_task_tool_call.rs: one struct
brief_work_plan.rs: one struct
brief_work_plan_params.rs: one struct
brief_work_task.rs: one struct
generated_brief.rs: one struct
generated_brief_cache_entry.rs: one struct
generated_brief_content.rs: one struct
generated_five_w.rs: one struct
sampling_brief_llm_client.rs: one struct
special/*.rs: zero types
tests/support/fixture_scip.rs: zero types
tests/support/mod.rs: zero types
tests/support/sampling_test_client.rs: one struct
```

Check submodule safety:

```bash
git status --short submodules
git submodule status --recursive
```

Check work-plan response shape through tests or an MCP client:

```json
{
  "project_id": "fixture",
  "prompt_version": "5w-summary-v1",
  "execution_model": "codex-parent-spawns-subagents",
  "parent_instructions": [
    "Spawn one Codex sub-agent per task_id.",
    "The MCP server does not spawn or manage Codex sub-agents."
  ],
  "tasks": [
    {
      "task_id": "brief-task-001",
      "preferred_agent": "explorer",
      "symbol_ids": ["..."],
      "tool_calls": [
        {
          "tool_name": "get_element_brief",
          "arguments": {
            "symbol_id": "...",
            "project_id": "fixture",
            "reference_limit": 5,
            "include_inferred": true
          }
        }
      ],
      "expected_output_schema": {
        "format": "json",
        "required_fields": [
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
        "json_schema": {
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
        }
      },
      "merge_key": "fixture::brief-task-001",
      "dependencies": []
    }
  ],
  "merge_instructions": ["Merge task outputs by merge_key."],
  "unknowns": [
    "Codex host sub-agent execution is outside rr-mcp and must be explicitly requested by the parent Codex session."
  ]
}
```

Check generated response shape through tests or a sampling-capable MCP host:

```json
{
  "project_id": "fixture",
  "brief_model": "host-default",
  "model_hint": null,
  "prompt_version": "5w-summary-v1",
  "max_concurrent_requests": 1,
  "items": [
    {
      "summary": "...",
      "five_w": {
        "who": "...",
        "what": "...",
        "when": "...",
        "where": "...",
        "why": "...",
        "how": "..."
      },
      "unknowns": ["..."],
      "evidence_hash": "<64 hex chars>",
      "cache_key": {
        "project_id": "fixture",
        "symbol_id": "...",
        "evidence_hash": "<same 64 hex chars>",
        "prompt_version": "5w-summary-v1",
        "brief_model": "host-default"
      },
      "generated_by_model": "<host reported model>",
      "prompt_version": "5w-summary-v1"
    }
  ]
}
```

## Acceptance Criteria

Implementation is complete only when all of this is true:

```text
- get_brief_generation_capabilities reports the current MCP host's sampling/createMessage support.
- get_brief_generation_capabilities reports generate_llm_brief_available=false and fallback_tool=generate_brief_work_plan when sampling is unsupported.
- generate_brief_work_plan returns a deterministic Codex-ready sub-agent work manifest.
- generate_brief_work_plan does not call sampling/createMessage.
- generate_brief_work_plan embeds generation_capabilities from the same MCP request context.
- generate_brief_work_plan returns exact project-qualified MCP tool-call recipes for each task.
- generate_brief_work_plan omits generate_llm_brief task calls when the current host does not advertise sampling support.
- generate_brief_work_plan includes generate_llm_brief task calls only when the current host advertises sampling support.
- generate_brief_work_plan returns expected output schema, merge key, task budget, dependencies, and acceptance criteria for each task.
- generate_brief_work_plan parent instructions explicitly say Codex spawns sub-agents and rr-mcp does not.
- generate_llm_brief builds evidence packets from cached deterministic element briefs.
- generate_llm_brief sends sampling/createMessage through the MCP host.
- Generated content is accepted only as GeneratedBriefContent JSON with deny_unknown_fields.
- Model-returned metadata is not trusted; server-owned cache/evidence metadata is attached.
- Unsupported sampling returns the typed BriefSamplingUnavailable MCP error.
- Sampling request failures return BriefGenerationRequestFailed.
- Invalid sampling responses return BriefGenerationResponseInvalid.
- Cache key includes project_id, symbol_id, evidence_hash, prompt_version, and model hint key.
- Same cache key avoids a second fake-client call.
- Different model hint causes a cache miss.
- Changed evidence hash causes a cache miss.
- Bounded concurrency preserves item order.
- Unit tests cover work-plan manifest generation, fake-client generation, caching, parsing, errors, and concurrency.
- MCP protocol tests cover capability probing, work-plan retrieval, sampling-capable generation, and non-sampling clients.
- README explains that Codex config/model hints do not create sampling support.
- No temporary smoke harness remains in the repo.
- No provider HTTP dependency or API-key path is added.
- No file under submodules/ is modified.
- No new Rust file violates one-type-per-file.
- cargo fmt --check passes.
- cargo test -p rr-mcp passes.
- cargo check -p rr-mcp --all-targets passes.
- cargo test --workspace --all-targets passes.
- cargo build --release -p rr-mcp passes.
```

## Final Status Language

Report completion with these exact categories:

```text
implemented:
  server code compiles, unit tests pass, work-plan generation works, and fake-client generation works

work-plan verified:
  generate_brief_work_plan returns Codex-ready task manifests with tool-call recipes,
  expected output schemas, merge keys, and parent merge instructions

registered:
  the MCP host can see and call the tool

unsupported-host verified:
  a host/client without sampling returns the typed unsupported-host error

sampling happy path verified:
  a sampling-capable MCP client receives sampling/createMessage and returns valid JSON

Codex happy path verified:
  Codex itself advertises sampling and completes generate_llm_brief
```

If Codex still lacks sampling, report:

```text
Codex happy path is blocked because this Codex MCP client does not advertise
sampling. Server happy path is verified by maintained MCP integration tests.
Unsupported-host behavior is verified through the non-sampling MCP integration
test, and through Codex only if the Codex tool call was actually attempted.
```

## Runtime Capability Resolution

Codex host sampling support is a runtime capability. The implementation must
resolve it with `get_brief_generation_capabilities` before choosing a
generated-brief path.

Probe first:

```json
{
  "tool": "get_brief_generation_capabilities",
  "arguments": {}
}
```

If the probe returns:

```json
{
  "sampling_supported": false,
  "generate_llm_brief_available": false,
  "deterministic_work_plan_available": true,
  "fallback_tool": "generate_brief_work_plan"
}
```

then Codex should call:

```json
{
  "tool": "generate_brief_work_plan",
  "arguments": {
    "project_id": "<project_id returned by scan_project or load_scip_project>",
    "limit": 6,
    "reference_limit": 5,
    "include_inferred": true,
    "budget_tokens": 800,
    "max_subagent_tasks": 6,
    "preferred_agent": "explorer"
  }
}
```

If the probe returns:

```json
{
  "sampling_supported": true,
  "generate_llm_brief_available": true
}
```

then Codex may call:

```json
{
  "tool": "generate_llm_brief",
  "arguments": {
    "project_id": "<project_id returned by scan_project or load_scip_project>",
    "limit": 1,
    "reference_limit": 5,
    "include_inferred": true,
    "budget_tokens": 500
  }
}
```

Interpret results exactly:

```text
capability probe says sampling_supported=false:
  do not call generate_llm_brief as the primary path; use generate_brief_work_plan

generated items returned:
  Codex happy path verified

"MCP client does not advertise sampling support":
  unsupported-host verified; capability probe or host behavior says use work-plan fallback

other MCP/protocol error:
  implementation or host integration still needs debugging
```
