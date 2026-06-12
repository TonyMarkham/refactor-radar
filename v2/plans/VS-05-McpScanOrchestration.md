# VS-05: MCP Scan Orchestration

## Objective

Add `crates/rr-mcp` as the MCP boundary that Codex talks to.

`rr-mcp` should read RefactorRadar configuration, invoke language scanner
binaries, pass concrete scan records into `rr-core`, and return compact scan
results and work manifests.

## Depends On

- VS-01 `rr-data-model`
- VS-04 `rr-core` artifact DB and hash gates

## Deliverables

- Add `crates/rr-mcp`.
- Add config parsing for scanner binaries.
- Add scanner process invocation.
- Add scan run directory creation.
- Add `scan_target` MCP tool.
- Add query tools needed by summarizer subagents.
- Update Codex MCP config guidance.
- Add integration tests with fake scanner binaries.

## Codex Configuration Boundary

Codex config should only launch `rr-mcp`.

Example:

```toml
[mcp_servers.refactor_radar]
command = "target/release/rr-mcp"
args = ["--config", ".refactor-radar/config.toml"]
enabled = true
required = true
startup_timeout_sec = 10
tool_timeout_sec = 60
```

Do not put RefactorRadar's scanner map directly in Codex config. Codex does not
own that schema.

## RefactorRadar Configuration

`rr-mcp` should parse `.refactor-radar/config.toml`.

Example:

```toml
[scan.languages.rust]
target_kinds = ["cargo_crate", "cargo_workspace"]
command = "target/release/rr-scip-rust"
args = ["scan", "--request", "{request}", "--out", "{output}"]

[scan.languages.csharp]
target_kinds = ["csproj", "sln"]
command = "target/release/rr-scip-dotnet"
args = ["scan", "--request", "{request}", "--out", "{output}"]
```

The initial implementation can support only Rust, but the config shape should
not hard-code Rust into the MCP protocol.

## Scanner Process Flow

`scan_target` should:

1. Resolve the requested target path.
2. Determine target kind or accept it explicitly.
3. Select a configured scanner binary.
4. Create `.refactor-radar/runs/{run_id}`.
5. Write scanner request JSON.
6. Invoke scanner process.
7. Read scanner response JSON.
8. Pass scan records to `rr-core`.
9. Persist compact scan result and work manifest.
10. Return compact MCP result to Codex.

`rr-mcp` should not inspect full source text unless serving a later evidence
query tool.

## Initial Tools

Recommended MCP tools:

- `scan_target`
- `list_scanners`
- `get_scan_run`
- `get_work_manifest`
- `get_element`
- `get_source_slice`
- `get_artifact_record`

Compatibility names such as `scan_rust_crate` can wrap `scan_target` later if
useful.

## Error Handling

Add `McpError` and `McpResult<T>`.

Errors should cover:

- config read failure
- config parse failure
- no scanner for target kind
- scanner request write failure
- scanner process launch failure
- scanner nonzero exit
- scanner response read failure
- scanner response parse failure
- core planning failure
- MCP serialization failure

Use the repo typed error policy.

## Tests

Add tests for:

- config parses multiple scanner entries
- missing scanner config returns typed error
- target kind selects expected scanner
- fake scanner process receives request path
- fake scanner response reaches `rr-core`
- scanner nonzero exit is mapped to a typed error
- `scan_target` returns compact result
- `rr-mcp` does not require compile-time dependency on scanner crates

## Verification

Run:

```bash
cargo test -p rr-mcp
cargo check -p rr-mcp
```

Manual verification:

```bash
target/release/rr-mcp --config .refactor-radar/config.toml
```

Then call `scan_target` through an MCP client.

## Non-Goals

- Do not run rust-analyzer directly from `rr-mcp`.
- Do not parse SCIP directly in `rr-mcp`.
- Do not spawn summarizer subagents.
- Do not publish summary artifacts.
