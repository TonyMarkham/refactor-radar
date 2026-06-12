# VS-06: Summarizer Work Manifest

## Objective

Define and emit the work manifest consumed by Codex-native summarizer
subagents.

This slice gives Codex enough compact work items to spawn independent
summarizers without loading full source files or full artifact contents into
the orchestrator context.

## Depends On

- VS-01 `rr-data-model`
- VS-04 artifact DB and hash gates
- VS-05 MCP scan orchestration

## Deliverables

- Finalize `WorkManifest` and `WorkItem` shapes.
- Preassign run-scoped artifact paths.
- Persist `work-manifest.json`.
- Return compact work item summaries from `scan_target`.
- Add query tools summarizers need for evidence retrieval.
- Add receipt shape for subagent completion.

## Manifest Location

Each scan run should use:

```text
.refactor-radar/runs/{run_id}/scan/work-manifest.json
.refactor-radar/runs/{run_id}/artifacts/{work_item_id}.json
```

The manifest should be deterministic enough to resume a run after Codex context
changes.

## Work Item Fields

Recommended fields:

```json
{
  "work_item_id": "wi-...",
  "run_id": "rr-...",
  "project_id": "project-key",
  "document_path": "src/source_span.rs",
  "element_id": "rr-element-v2:...",
  "element_kind": "TypeAlias",
  "display_name": "impl SourceSpan",
  "aspect": "5w",
  "artifact_path": ".refactor-radar/runs/rr-.../artifacts/wi-....json",
  "content_hash": "sha256:...",
  "required_tools": [
    "get_element",
    "get_source_slice",
    "get_artifact_record"
  ]
}
```

The work item should not include full source text. Summarizers should request
evidence through MCP tools.

## Artifact Eligibility

Every element emitted by a language scanner can receive a summary work item
when its artifact is missing or stale.

Do not hard-code Rust or C# element allowlists in the manifest builder.

## Subagent Receipt

A summarizer should write exactly one artifact and return a compact receipt:

```json
{
  "work_item_id": "wi-...",
  "artifact_path": ".refactor-radar/runs/rr-.../artifacts/wi-....json",
  "artifact_sha256": "sha256:...",
  "status": "completed"
}
```

The orchestrator can collect receipts without reading artifact contents.

## Evidence Tools

MCP evidence tools should be compact and keyed by IDs from the manifest.

Initial tools:

- `get_element`
- `get_source_slice`
- `get_artifact_record`
- `get_scan_run`
- `get_work_manifest`

Later tools can expose references and related elements once the artifact DB
stores them.

## Error Handling

Errors should cover:

- work manifest write failure
- artifact path assignment failure
- duplicate work item ID
- unsupported aspect
- missing element for work item
- invalid receipt shape

Use the repo typed error policy in the crate that owns the behavior.

## Tests

Add tests for:

- missing artifact creates work item
- changed content hash creates work item
- unchanged artifact produces no work item
- work item path is under the run directory
- duplicate work item IDs fail
- manifest serializes and round trips
- compact MCP result does not include full source text

## Verification

Run:

```bash
cargo test -p rr-core
cargo test -p rr-mcp
```

## Non-Goals

- Do not implement summarizer agents.
- Do not validate summary prose quality.
- Do not publish artifacts into the artifact DB.
