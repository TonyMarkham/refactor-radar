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

- `refactor_radar_core::SemanticModel`
- `refactor_radar_scip::{generate_rust_scip, project_scip_index, Format}`
- `refactor_radar_report::{build_element_brief, build_project_summary,
  generate_llm_brief}`

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

- Import `ToolRouter` from `rmcp::handler::server::tool::ToolRouter`.
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
  of choosing the first match.
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
  portion with the relevant IDs and evidence.
- `generate_llm_brief` delegates to
  `refactor_radar_report::generate_llm_brief`.

## Error Mapping

Implement MCP error conversion:

- Missing project, element, symbol, or span maps to MCP resource-not-found.
- Unsupported explicit `format` values map to MCP invalid params with the
  rejected format.
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
- `find_related_symbols` returns function-like relationships.
- `get_source_span` returns span metadata only.
- `generate_llm_brief` returns Markdown in structured JSON.

Do not require a real rust-analyzer binary for these tests.

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
