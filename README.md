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
