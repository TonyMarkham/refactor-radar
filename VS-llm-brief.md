# Implementation Plan: Host-Sampled LLM Brief Generation

## Corrected Direction

`rr-mcp` should not hard-code an OpenAI HTTP client for this feature.

This repo is being used through Codex as the MCP host. A Codex subscription gives the host a model surface; it does not automatically give a local Rust binary an API key or permission to call OpenAI over HTTP. The right implementation for this feature is MCP host-mediated sampling:

1. `rr-mcp` deterministically selects elements and builds grounded evidence packets.
2. `rr-mcp` asks the connected MCP client/host to generate text via `sampling/createMessage`.
3. Codex, or another capable MCP host, chooses and runs the model.
4. `rr-mcp` validates the returned JSON into typed generated-brief structs.
5. `rr-mcp` returns generated summaries with evidence/cache metadata.

Do not add `reqwest`, `OPENAI_API_KEY`, raw Responses API code, or an OpenAI provider client for this milestone. Direct provider adapters can be a later opt-in feature, not the core implementation.

## Hard Constraints

- Edit only top-level workspace files during implementation. Do not edit anything under `submodules/`.
- Follow `AGENTS.md`: crate-local typed errors, `thiserror`, `error_location::ErrorLocation`, `#[track_caller]` constructors, and no `anyhow`.
- Preserve the repo's one-type-per-file convention. Do not add a grouped `types.rs`.
- Keep deterministic scan/query paths useful without LLM generation.
- Tests must not call any network or paid model.
- Do not introduce implementation `unwrap`, `expect`, or `panic`.

## Current Repo Facts

- `crates/rr-mcp/src/mcp_server.rs` already implements the `generate_llm_brief` tool, but it only returns evidence packets, prompt text, and a schema.
- `rr-mcp` already depends on `rmcp = 1.7.0`.
- Local `rmcp` 1.7.0 supports MCP sampling from a server through `Peer<RoleServer>::create_message`.
- Tool handlers can receive `rmcp::service::RequestContext<RoleServer>` as an extracted argument.
- `crates/rr-mcp/src/project_cache.rs` has no generated-brief cache yet.
- `crates/rr-mcp/src/error.rs` already owns the crate-local `McpError`.
- `crates/rr-core/src/summaries/five_w.rs` uses deterministic `Vec<String>` 5W fields. Generated summaries should use concise string fields.

## Pre-Flight Commands

```bash
git status --short
git submodule status --recursive
rg -n "generate_llm_brief|brief_model|max_concurrent_requests|evidence_hash|cache_key" crates/rr-mcp/src
rg -n "create_message|SamplingMessage|CreateMessageRequestParams|RequestContext<RoleServer>" ~/.cargo/registry/src/index.crates.io-*/rmcp-1.7.0/src ~/.cargo/registry/src/index.crates.io-*/rmcp-1.7.0/tests
rg -n "pub struct .*\\{|pub enum .*\\{|pub trait .*\\{" crates/rr-mcp/src -g '*.rs'
```

The final `rg` should confirm the current one-primary-type-per-file pattern.

## Phase 1: Dependencies

Add only the dependencies needed for host-mediated sampling and bounded concurrency.

`Cargo.toml`:

```toml
[workspace.dependencies]
async-trait                    = { version = "0.1.89" }
futures                        = { version = "0.3.31" }
```

`crates/rr-mcp/Cargo.toml`:

```toml
[dependencies]
async-trait   = { workspace = true }
futures       = { workspace = true }
```

Do not add:

```toml
reqwest = "..."
async-openai = "..."
openai = "..."
anyhow = "..."
```

`rmcp` is already present and is the provider boundary for this milestone.

## Phase 2: Add One-Type-Per-File Data Shapes

Create one new type per file under `crates/rr-mcp/src/`.

### `brief_cache_key.rs`

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

Use `brief_model` as the requested/server model hint when present. If no hint is provided, use the stable string `host-default` in the cache key and store the actual returned model separately in `generated_by_model`.

### `generated_five_w.rs`

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

### `generated_brief_content.rs`

This is the only shape accepted from model output. Do not trust model-returned metadata.

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

### `generated_brief.rs`

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

### `generated_brief_cache_entry.rs`

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

### `brief_llm_request.rs`

```rust
#[derive(Clone, Debug)]
pub struct BriefLlmRequest {
    pub model_hint: Option<String>,
    pub instructions: String,
    pub evidence_packet: serde_json::Value,
    pub max_tokens: u32,
}
```

### `brief_llm_client.rs`

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

The returned `String` is the actual model identifier reported by the MCP host.

## Phase 3: Add Typed MCP Errors

Extend `McpError` in `crates/rr-mcp/src/error.rs`.

Add variants:

```rust
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
```

Add constructors:

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

Update `message()` to include all three variants.

Update `crates/rr-mcp/src/error_conversion.rs`:

```rust
McpError::BriefSamplingUnavailable { message, .. } => {
    ProtocolError::invalid_request(message, None)
}
McpError::BriefGenerationRequestFailed {
    message, details, ..
}
| McpError::BriefGenerationResponseInvalid {
    message, details, ..
} => ProtocolError::internal_error(message, Some(json!({ "details": details }))),
```

## Phase 4: Add Sampling Client

### `sampling_brief_llm_client.rs`

Use MCP sampling. This is the default production client.

```rust
use crate::{BriefLlmClient, BriefLlmRequest, GeneratedBriefContent, McpError, McpResult};

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
        let user_prompt = format!(
            "Return only a JSON object matching this schema:\n\
             {{\"summary\":\"string\",\"five_w\":{{\"who\":\"string\",\"what\":\"string\",\"when\":\"string\",\"where\":\"string\",\"why\":\"string\",\"how\":\"string\"}},\"unknowns\":[\"string\"]}}\n\n\
             Use only this evidence packet. Unsupported facts must go in unknowns.\n\n\
             Evidence packet:\n{evidence_json}"
        );

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
        let output_text = assistant_text(&result)
            .ok_or_else(|| McpError::brief_generation_response_invalid("sampling response did not contain assistant text"))?;
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
```

If the `SamplingContent::iter()` API changes, use the equivalent local `rmcp` accessor. Keep this as a sampling adapter, not a provider adapter.

## Phase 5: Export New Types

Update `crates/rr-mcp/src/lib.rs`:

```rust
pub mod brief_cache_key;
pub mod brief_llm_client;
pub mod brief_llm_request;
pub mod generated_brief;
pub mod generated_brief_cache_entry;
pub mod generated_brief_content;
pub mod generated_five_w;
pub mod sampling_brief_llm_client;

pub use brief_cache_key::BriefCacheKey;
pub use brief_llm_client::BriefLlmClient;
pub use brief_llm_request::BriefLlmRequest;
pub use generated_brief::GeneratedBrief;
pub use generated_brief_cache_entry::GeneratedBriefCacheEntry;
pub use generated_brief_content::GeneratedBriefContent;
pub use generated_five_w::GeneratedFiveW;
pub use sampling_brief_llm_client::SamplingBriefLlmClient;
```

## Phase 6: Add Generated-Brief Cache

Update `crates/rr-mcp/src/project_cache.rs`:

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

Do not clear `generated_briefs` on project insert. The key includes evidence hash and prompt version, so rescans miss when evidence changes.

## Phase 7: Add the Sampling Helper

Update `McpServer` in `crates/rr-mcp/src/mcp_server.rs`:

```rust
use crate::{
    BriefCacheKey, BriefLlmClient, BriefLlmRequest, GeneratedBrief, GeneratedBriefCacheEntry,
    GeneratedBriefContent, SamplingBriefLlmClient,
    // keep existing imports
};

use futures::{stream, StreamExt};
use rmcp::{RoleServer, service::RequestContext};
```

Do not add a provider/client field to `McpServer`. The real sampling client needs the current request's `Peer<RoleServer>`, so construct it inside the tool handler from `RequestContext<RoleServer>`. Tests will call the internal helper with a fake client.

## Phase 8: Update `generate_llm_brief`

Update the tool annotation description so MCP clients see the new behavior:

```rust
#[tool(
      name = "generate_llm_brief",
      description = "Generate host-sampled per-element 5W summaries from cached RefactorRadar evidence",
      output_schema = rmcp::handler::server::tool::schema_for_type::<rmcp::model::JsonObject>()
)]
```

Change the tool signature to accept the MCP request context:

```rust
async fn generate_llm_brief(
    &self,
    context: RequestContext<RoleServer>,
    Parameters(params): Parameters<LlmBriefParams>,
) -> Result<Json<serde_json::Value>, ProtocolError> {
    let brief_client = Arc::new(SamplingBriefLlmClient::new(context.peer.clone()));
    self.generate_llm_brief_with_client(params, brief_client).await
}
```

Add an internal helper that contains the implementation and is directly testable:

```rust
async fn generate_llm_brief_with_client(
    &self,
    params: LlmBriefParams,
    brief_client: Arc<dyn BriefLlmClient>,
) -> Result<Json<serde_json::Value>, ProtocolError> {
```

Effective config:

```rust
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
let max_tokens = params
    .budget_tokens
    .unwrap_or(800)
    .min(u32::MAX as usize) as u32;
```

Build work items while holding the cache read lock, then drop it before sampling:

```rust
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
```

Element selection helper:

```rust
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
```

Generate with bounded concurrency. Use `buffered`, not `buffer_unordered`, to preserve order.

```rust
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
```

Helper:

```rust
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
```

Store generated results:

```rust
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
```

Return generated summaries, not evidence packets:

```rust
structured(serde_json::json!({
    "project_id": project_id,
    "brief_model": cache_model_key,
    "model_hint": model_hint,
    "prompt_version": LLM_BRIEF_PROMPT_VERSION,
    "max_concurrent_requests": max_concurrent_requests,
    "items": generated_results,
}))
.map_err(to_protocol_error)
```

Remove `summary_schema`, `instructions`, and `evidence_packet` from the default generated response. Use existing deterministic tools (`get_element_brief`, `get_5w_summary`) when raw evidence is needed.

## Phase 9: Tests

Use fake clients for unit tests. Do not call Codex, OpenAI, or the network.
Replace the existing `generate_llm_brief` tests that call the public tool method
directly; after the public method accepts `RequestContext<RoleServer>`, unit
tests should exercise generated-brief behavior through
`generate_llm_brief_with_client` unless they construct a valid MCP
`RequestContext`.

In `crates/rr-mcp/src/mcp_server/tests.rs`, add one fake type:

```rust
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

Create a server with normal constructors, then call the internal helper with the fake client:

```rust
fn server_with_fake_client(
    brief_model: Option<String>,
    max_concurrent_requests: Option<usize>,
) -> McpServer {
    McpServer::with_runtime_config(
        None,
        brief_model,
        max_concurrent_requests,
    )
}
```

The test calls should look like this after loading the fixture project:

```rust
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

assert_eq!("test-model-hint", result["model_hint"]);
```

Required `mcp_server/tests.rs` tests:

1. Request `brief_model` becomes the sampling model hint.
2. Server config `brief_model` is used as the hint when request hint is omitted.
3. Missing request/config model is allowed and uses `host-default` as the cache model key.
4. Request concurrency overrides server config.
5. Server config concurrency is used when request concurrency is omitted.
6. Zero concurrency is clamped to `1`.
7. Generated summary is returned in `items[0].summary`.
8. Generated 5W strings are returned.
9. Generated unknowns are returned.
10. `evidence_hash` is 64 hex characters.
11. `cache_key.evidence_hash` equals `evidence_hash`.
12. `cache_key.brief_model` equals the model hint or `host-default`.
13. `generated_by_model` equals the model returned by the fake client.
14. Cache hit avoids a second fake-client call.
15. Different model hint causes a cache miss.
16. Changed evidence hash causes a cache miss.
17. Concurrency cap is respected.

Add a small `#[cfg(test)]` module in `sampling_brief_llm_client.rs` for parser-specific tests:

```rust
#[test]
fn given_invalid_sampling_json_when_parsing_then_typed_error_is_returned() {
    let error = parse_generated_brief_content("not json").err();

    assert!(matches!(
        error,
        Some(McpError::BriefGenerationResponseInvalid { .. })
    ));
}
```

For changed evidence hash, vary `reference_limit` or `include_inferred` with a fixture that actually changes the evidence packet.

## Phase 10: Manual Smoke Test With Codex Host

After tests pass, build and run the MCP server normally. Do not set an API key.

```bash
cargo test -p rr-mcp
cargo build --release -p rr-mcp
target/release/rr-mcp --help
```

Expected help still includes:

```text
--brief-model
--max-concurrent-requests
```

Then, from Codex or another MCP client that advertises sampling, call:

```json
{
  "tool": "scan_project",
  "arguments": {
    "path": "crates/rr-core"
  }
}
```

Then call:

```json
{
  "tool": "generate_llm_brief",
  "arguments": {
    "project_id": "<returned project_id>",
    "limit": 1,
    "reference_limit": 5,
    "include_inferred": true
  }
}
```

Expected response shape:

```json
{
  "brief_model": "host-default",
  "model_hint": null,
  "prompt_version": "5w-summary-v1",
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
        "project_id": "<returned project_id>",
        "symbol_id": "...",
        "evidence_hash": "<same 64 hex chars>",
        "prompt_version": "5w-summary-v1",
        "brief_model": "host-default"
      },
      "generated_by_model": "<model reported by host>",
      "prompt_version": "5w-summary-v1"
    }
  ]
}
```

If the host does not advertise sampling, the tool must return the typed `BriefSamplingUnavailable` MCP error. That is a real unsupported-host condition, not a prompt/model failure.

## Verification Gate

Do not mark complete until all are true:

- `cargo fmt --check` passes.
- `cargo test -p rr-mcp` passes.
- `cargo check -p rr-mcp --all-targets` passes.
- `cargo test --workspace --all-targets` passes.
- `cargo build --release -p rr-mcp` passes.
- No `reqwest`, OpenAI HTTP client, `OPENAI_API_KEY`, or provider-specific code is added.
- `generate_llm_brief` returns generated summaries when the MCP host supports sampling.
- Unsupported sampling returns a typed MCP error.
- Cache key includes `project_id`, `symbol_id`, `evidence_hash`, `prompt_version`, and the model hint key.
- Repeated same-key generation avoids a second fake-client call in tests.
- Different model hint or changed evidence hash causes a cache miss in tests.
- No `anyhow`, implementation `unwrap`, implementation `expect`, or implementation `panic` is introduced.
- No file under `submodules/` is modified.
