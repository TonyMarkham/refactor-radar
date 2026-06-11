# RefactorRadar

RefactorRadar builds SCIP-backed semantic indexes for Rust projects and exposes
them through local query tools, including a Codex MCP server.

## Codex MCP Setup

Build the MCP server:

```bash
cargo build -p rr-mcp
```

`scan_project` uses `rust-analyzer scip`. Do not point the MCP server at the
`/home/tony/.cargo/bin/rust-analyzer` shim or at anything under `submodules/`.
Use the rustup-installed toolchain binary.

Check whether the rust-analyzer component is installed:

```bash
rustup component list --installed | rg '^rust-analyzer'
rustup which rust-analyzer
```

If the component is missing, install it:

```bash
rustup component add rust-analyzer
```

After installation, `rustup which rust-analyzer` should print a toolchain path
similar to:

```text
/home/tony/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rust-analyzer
```

Configure Codex to pass that path to the MCP server in `.codex/config.toml`:

```toml
[mcp_servers.refactor_radar]
command = "target/debug/rr-mcp"
args = ["--rust-analyzer-path", "/home/tony/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rust-analyzer"]
enabled = true
required = true
startup_timeout_sec = 10
tool_timeout_sec = 60
```

Restart Codex after changing `.codex/config.toml`; MCP servers are started when
the Codex session starts.

## MCP Smoke Test

After restarting Codex, call the `refactor_radar` MCP server:

```text
refactor_radar.scan_project({ "path": "/home/tony/git/refactor-radar" })
```

The call should return a loaded project result. If it reports an unknown
`rust-analyzer` binary, re-check that `.codex/config.toml` points at the output
of `rustup which rust-analyzer`.

## Host-Sampled LLM Briefs And Codex Work Plans

The working Codex flow is:

```text
scan_project or load_scip_project
generate_brief_work_plan
Codex executes the returned get_element_brief tool_calls
Codex or Codex sub-agents generate the 5W brief JSON in the normal conversation
parent Codex merges task outputs by merge_key
```

`generate_brief_work_plan` is the primary Codex path. It is deterministic and
returns a Codex-ready task manifest with independent brief/summary tasks, exact
MCP tool-call recipes for each task, expected sub-agent output schemas, merge
keys, task budgets, acceptance criteria, and parent merge instructions. Codex can
use that manifest to spawn sub-agents, but `rr-mcp` does not spawn or manage
Codex sub-agents.

`get_brief_generation_capabilities` reports whether the connected MCP host
advertises sampling support. Call it before choosing a generated-brief path. If
it reports `sampling_supported: false`, use `generate_brief_work_plan`.

`generate_llm_brief` is only for MCP clients that advertise host-mediated
sampling. In that path, `rr-mcp` selects cached RefactorRadar evidence and asks
the connected MCP host to generate JSON through `sampling/createMessage`.

`--brief-model` and `.codex/config.toml` configure the server and model hint
only. They do not make an MCP host advertise sampling support.

Verification status should be reported with these terms:

- `implemented`: server code compiles, unit tests pass, work-plan generation works, and fake-client generation works.
- `registered`: the MCP host can see and call the tools.
- `Codex happy path verified`: Codex executes the work-plan flow and produces merged 5W brief output.
- `unsupported-host verified`: a host without sampling returns the typed unsupported-host error for `generate_llm_brief`.
- `sampling happy path verified`: a sampling-capable MCP client completes `sampling/createMessage`.

If Codex does not advertise sampling, `generate_llm_brief` is expected to return
the typed unsupported-host error. That does not block the Codex work-plan flow.

```text
MCP client does not advertise sampling support
```
