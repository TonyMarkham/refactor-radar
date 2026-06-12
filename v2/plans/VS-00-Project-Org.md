# VS-00: Project Organization

## Objective

Define the v2 crate boundaries, process boundaries, and planning sequence for
RefactorRadar. This file is the architecture map. Implementation details live
in the vertical slice plans that follow it.

The immediate priority is to reach graphify-level feature parity in
RefactorRadar's own scanner, artifact, and summarizer flow. Editor integrations
are future work.

## Current Repo State

The v1 implementation is archived under `v1/`.

The root v2 workspace currently contains only:

```text
crates/wip
```

`crates/wip` is temporary and should be replaced by the first real v2 crate.

## Target Crates

Recommended v2 crates:

```text
crates/
  rr-data-model/
  rr-scip/
  rr-scip-rust/
  rr-scip-dotnet/
  rr-core/
  rr-mcp/
```

Optional later crates:

```text
crates/
  rr-cli/
  rr-report/
  rr-lsp/
```

Do not add optional crates until there is a concrete consumer.

## Dependency Direction

Arrows mean "depends on":

```text
rr-mcp -> rr-core
rr-mcp -> rr-data-model

rr-core -> rr-data-model

rr-scip-rust -> rr-scip
rr-scip-rust -> rr-data-model

rr-scip-dotnet -> rr-scip
rr-scip-dotnet -> rr-data-model
```

Important negative rules:

- `rr-data-model` has no RefactorRadar crate dependencies.
- `rr-scip` does not depend on `rr-data-model`.
- `rr-scip` does not depend on `rr-core`.
- `rr-core` does not depend on `rr-scip`.
- `rr-core` does not depend on language scanner crates.
- `rr-mcp` does not compile against language scanner crates.

## Crate Roles

### `rr-data-model`

Owns concrete RefactorRadar shapes and generic conversion traits.

It should define durable, serializable nouns such as:

- `Project`
- `Document`
- `Symbol`
- `Occurrence`
- `Range`
- `Signature`
- `Element`
- `ElementId`
- `ContentHash`
- `ArtifactDb`
- `WorkManifest`
- `WorkItem`
- `ScanRun`
- `Diagnostic`
- `TryIntoDataModel`

It must not know about rust-analyzer, scip-dotnet, MCP, source hashing, or
artifact scheduling.

### `rr-scip`

Owns the small curated SCIP helper library.

It should:

- own or re-export the upstream SCIP protobuf bindings used by Rust crates
- read binary `.scip` and SCIP JSON artifacts
- expose parsed `scip::types::Index` values
- provide range, enclosing-range, symbol, and occurrence helpers

It should not convert SCIP into RefactorRadar records and should not be a
binary, DLL, MCP server, or runtime boundary.

### `rr-scip-rust`

Owns the Rust scanner binary.

It should:

- run `rust-analyzer scip`
- parse the generated SCIP artifact through `rr-scip`
- implement `rr-data-model` conversion traits for Rust/rust-analyzer data
- emit concrete `rr-data-model` scan records

### `rr-scip-dotnet`

Owns the C# scanner binary.

It should:

- run `submodules/scip-dotnet`
- accept `.csproj` and later `.sln` targets
- parse the generated SCIP artifact through `rr-scip`
- implement `rr-data-model` conversion traits for C#/scip-dotnet data
- emit concrete `rr-data-model` scan records

### `rr-core`

Owns shared deterministic behavior after scanner binaries have produced
concrete `rr-data-model` records.

It should:

- read and validate the artifact DB
- apply the file hash gate
- apply element source-slice hash gates
- build deterministic element IDs
- compare artifact/aspect records
- emit work manifests
- prepare publish/register operations

It should not run language indexers or parse SCIP artifacts.

### `rr-mcp`

Owns MCP transport and RefactorRadar scanner process orchestration.

It should:

- read RefactorRadar scanner configuration
- invoke configured scanner binaries as subprocesses
- pass scanner output to `rr-core`
- expose query and scan tools to Codex

It should not contain language-specific scanner logic.

## Process Boundaries

Codex configuration only starts `rr-mcp`.

Example Codex MCP configuration:

```toml
[mcp_servers.refactor_radar]
command = "target/release/rr-mcp"
args = ["--config", ".refactor-radar/config.toml"]
enabled = true
required = true
startup_timeout_sec = 10
tool_timeout_sec = 60
```

`rr-mcp` then parses RefactorRadar's own config:

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

The scanner process contract is:

```text
scanner-binary scan --request {request_path} --out {response_path}
```

## Vertical Slice Plan Index

Implement in this order:

1. [VS-01: Data Model](VS-01-DataModel.md)
2. [VS-02: SCIP Helper](VS-02-ScipHelper.md)
3. [VS-03: Rust Scanner Binary](VS-03-RustScannerBinary.md)
4. [VS-04: Artifact DB And Hash Gates](VS-04-ArtifactDbHashGates.md)
5. [VS-05: MCP Scan Orchestration](VS-05-McpScanOrchestration.md)
6. [VS-06: Summarizer Work Manifest](VS-06-SummarizerWorkManifest.md)
7. [VS-07: Artifact Publish](VS-07-ArtifactPublish.md)

## Future Addendum: Editor Access

Someday, RefactorRadar should expose editor navigation over the artifact DB.

Likely first step:

```text
rr-lsp
  reads artifact DB
  reads summary artifacts
  maps source ranges to RR artifacts
  maps RR artifacts back to source ranges
  exposes navigation, hovers, code lenses, diagnostics, and search
```

Native IDE extensions can wrap or supplement `rr-lsp` where LSP integration is
weak. This is future work after RefactorRadar reaches graphify-level feature
parity.

The artifact DB should be the source of truth. Editor integrations should
consume it, not maintain documentation.
