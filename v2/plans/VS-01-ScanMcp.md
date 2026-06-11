# VS-01: Scanner MCP

## Objective

Build the v2 scanner MCP surface that turns a Rust crate path into a
deterministic summarization work manifest.

The scanner is the deterministic front half of the graphify-inspired
documentation system. It must run `rust-analyzer scip`, save the SCIP artifact,
compare the current crate against prior scan state, and return only the summary
work items that actually need model inference.

The scanner does not summarize. It decides what needs summarization.

## Context From Current Repo

The current repo already has these relevant crates:

- `crates/rr-core`
- `crates/rr-scip`
- `crates/rr-mcp`
- `crates/rr-report`

The current MCP server already exposes:

- `generate_rust_scip`
- `load_scip_project`
- `scan_project`
- `generate_brief_work_plan`
- query tools such as `list_elements`, `get_element`, `find_references`, and
  `find_related_symbols`

The v2 scanner should not discard that work, but it should add a more precise
crate scan tool that owns SCIP generation, deterministic source hashing,
delta planning, and artifact path assignment.

## Required Invariants

The scanner must enforce these rules:

- Raw SCIP symbol IDs are never durable.
- Raw SCIP symbol IDs are never artifact DB keys.
- Raw SCIP symbol IDs are never hash inputs.
- SCIP ranges are never durable identity.
- SCIP ranges are never content hash inputs.
- SCIP ranges may only be used as current-scan coordinates to locate source
  slices in the current source file.
- If a file hash is unchanged, everything rooted in that file is skipped.
- If a file hash changed, only changed child elements or missing artifacts are
  emitted as summarization work.
- The MCP scanner returns compact manifests and metadata, not full source text.

## Public MCP Shape

Add a new tool rather than overloading the existing `scan_project` semantics:

```text
scan_crate(crate_path, options) -> ScanCrateResult
```

`scan_project` can remain as the current simple "generate SCIP and cache the
model" tool. `scan_crate` is the v2 scanner workflow tool.

Recommended request fields:

```json
{
  "crate_path": "crates/rr-core",
  "output_root": ".refactor-radar",
  "artifact_db_path": ".refactor-radar/artifacts.json",
  "scip_artifact_path": null,
  "config_path": null,
  "exclude_vendored_libraries": true,
  "num_threads": null,
  "summary_aspects": ["5w", "behavior", "references"],
  "force": false
}
```

Recommended result fields:

```json
{
  "run_id": "rr-20260611-162000-abc123",
  "project_id": "file:///home/tony/git/refactor-radar/crates/rr-core",
  "crate_path": "crates/rr-core",
  "scip_artifact_path": ".refactor-radar/runs/rr-.../scan/index.scip",
  "artifact_db_path": ".refactor-radar/artifacts.json",
  "file_counts": {
    "total": 25,
    "unchanged": 22,
    "changed": 2,
    "new": 1,
    "deleted": 0
  },
  "element_counts": {
    "seen": 275,
    "skipped_by_file_hash": 240,
    "skipped_by_element_hash": 28,
    "scheduled": 7
  },
  "work_items": [],
  "diagnostics": []
}
```

The exact schema should be implemented as Rust types in `crates/rr-mcp/src`,
one named type per file, matching the local style.

## Tool Responsibilities

`scan_crate` must perform the following steps inside the MCP server:

1. Resolve and validate the crate path.
2. Create a run directory under `output_root`.
3. Run `rust-analyzer scip`.
4. Save the SCIP artifact to the run directory or requested output path.
5. Decode the SCIP artifact through `rr-scip`.
6. Load/cache the projected semantic model for existing query tools.
7. Hash current source files.
8. Load the prior artifact DB if present.
9. Apply the file hash gate.
10. For changed files only, derive deterministic child projections.
11. Hash child source slices.
12. Compare child/aspect hashes with the artifact DB.
13. Emit work items only for changed or missing aspects.
14. Preassign each work item a run-scoped artifact path.
15. Persist scan-run metadata needed by later artifact registration and publish
    steps.

The MCP server should do all deterministic scanning and comparison without
requiring Codex to inspect the SCIP artifact or source files.

## File Hash Gate

The first optimization gate is the normalized source file hash.

For each `Document.relative_path` in the SCIP artifact:

```text
current_file_hash = sha256(normalized_file_bytes)
previous_file_hash = artifact_db.files[relative_path].file_hash
```

If the hashes match:

```text
skip entire document
```

No child element in that document should enter the summarizer workflow. This
includes modules, structs, enums, impl blocks, fields, functions, methods,
tests, comments, references, and all summary aspects rooted in that file.

Only files that are new or changed proceed to child-level inspection.

Deleted files should be recorded in diagnostics or run state so the publish
step can expire stale artifacts, but deleted files do not create summarizer
work.

## Child Hash Gate

For changed files, the scanner must inspect child elements and hash their
content.

SCIP provides the current document, definition occurrences, symbol kind,
display name, signature text, references, and ranges. SCIP does not contain the
function body. Therefore the scanner must use SCIP bounds only to locate source
slices in the current source file.

For function-like items, prefer `typed_enclosing_range` or `enclosing_range`
because it covers the item body. For impl blocks, use the enclosing range for
the whole impl block. For fields, enum variants, constants, type aliases,
modules, and similar items, use the best available enclosing range or
definition range to locate the relevant source slice.

The normalized source slice selected by the SCIP bounds is the sole target of
the child content hash:

```text
element_content_hash = sha256(normalized_source_slice_from_scip_bounds)
```

For a method, the source slice should include the method attributes, signature,
body, and braces when the SCIP enclosing range includes them. For the observed
`rr-core` artifact, `SourceSpan::from_scip_range` had an identifier-only
`range` and a full method-item `enclosing_range`.

For an attributed function, the source slice should include attached
attributes. For the observed `rr-core` artifact, `CoreError::invalid_scip_range`
had an `enclosing_range` that included `#[track_caller]`.

For an impl block, the source slice should include the impl header, all child
items in that impl, and the block braces. Impl block hashes are intentionally
broader than method hashes. A sibling method change should change the impl
block hash, but it should not change the unchanged sibling method's hash.

The scanner may still attach non-hashed metadata to the work item and artifact
DB for routing, display, and debugging:

```json
{
  "schema_version": 1,
  "kind": "static_method",
  "display_name": "from_scip_range",
  "parent_locator": "impl SourceSpan",
  "signature_text": "pub fn from_scip_range(project_id: impl Into<String>, document_path: impl Into<String>, range: &[i32]) -> CoreResult<Self>",
  "content_hash": "sha256:..."
}
```

Do not include these values in `content_hash`:

- SCIP symbol ID
- raw `local N` symbol IDs
- definition range
- enclosing range
- line number
- column number
- occurrence order
- rust-analyzer internal IDs
- signature text as a separate hash input
- parent locator as a separate hash input
- display name as a separate hash input

## Element Identity

The element ID and the element content hash are different things.

Element identity answers:

```text
Which durable RefactorRadar artifact does this current item correspond to?
```

Element content hash answers:

```text
Did this item's summarizable content change?
```

Element IDs should be RefactorRadar-owned and deterministic. They may include:

- crate identity
- normalized relative file path
- element kind
- display name
- deterministic parent locator
- a deterministic disambiguator only when unavoidable

Element IDs must not include:

- raw SCIP symbol ID
- raw `local N`
- source range
- line number
- column number

Recommended conceptual shape:

```text
rr-element-v2:
  crate_key
  normalized_relative_file_path
  parent_locator
  kind
  display_name
  disambiguator
```

The initial implementation should avoid scheduling local variables as
summarizable elements. Locals can remain evidence for summaries, but they should
not receive durable summary artifacts in v2 unless a later plan explicitly adds
that feature.

## Parent Locator

The scanner needs a deterministic parent locator to distinguish fields,
methods, trait methods, and nested scopes without using SCIP IDs.

Impl blocks have two roles:

- They are parent scopes for methods and associated items.
- They are also first-class summarizable elements with their own source-slice
  hash and optional summary artifacts.

Preferred parent locators:

```text
module source_span
struct SourceSpan
enum ElementKind
impl SourceSpan
impl CoreError
impl TraitName for TypeName
trait SomeTrait
```

For an impl block element, the `parent_locator` should be the containing module
or type context, and the element display name should describe the impl itself,
for example:

```text
display_name = "impl SourceSpan"
parent_locator = "module source_span"
```

The parent locator can be derived from the current source slice and nearby SCIP
metadata. If the scanner cannot derive a trustworthy parent locator, it should:

1. Fall back to a conservative locator based on the source file and nearest
   top-level item.
2. Add a diagnostic.
3. Avoid reusing an existing artifact unless the identity is unambiguous.

## Source Slicing

The scanner must read the current source file and slice by SCIP ranges using
the artifact's declared position encoding.

The `rr-core` SCIP artifact observed during design used:

```text
position_encoding: UTF8CodeUnitOffsetFromLineStart
```

The implementation should:

- support SCIP typed ranges first
- fall back to legacy range fields
- handle single-line and multi-line ranges
- treat range values as current-scan locators only
- normalize line endings before hashing
- avoid trimming or formatting source slices in a way that erases meaningful
  code changes

A simple normalization policy for v2:

```text
normalize CRLF to LF
preserve all non-line-ending whitespace
preserve comments inside the item slice
hash UTF-8 bytes
```

## Artifact DB

Use a local artifact DB under `output_root`.

Initial format should be JSON because the workspace already depends on
`serde_json`. The code should keep the schema versioned so TOML/YAML can be
added later if needed.

Recommended path:

```text
.refactor-radar/artifacts.json
```

Recommended top-level shape:

```json
{
  "schema_version": 1,
  "crates": {
    "crate-key": {
      "crate_path": "crates/rr-core",
      "last_completed_run_id": "rr-...",
      "files": {
        "src/source_span.rs": {
          "file_hash": "sha256:...",
          "elements": {
            "rr-element-v2:...": {
              "kind": "static_method",
              "display_name": "from_scip_range",
              "parent_locator": "impl SourceSpan",
              "content_hash": "sha256:...",
              "aspects": {
                "5w": {
                  "aspect_hash": "sha256:...",
                  "artifact_path": ".refactor-radar/artifacts/...",
                  "artifact_sha256": "sha256:..."
                }
              }
            }
          }
        }
      }
    }
  }
}
```

The scanner reads this DB. It should not publish new accepted summary artifacts
directly. Publishing belongs to the later completion tool.

## Run Directory

Each scan should allocate a run directory:

```text
.refactor-radar/runs/{run_id}/
```

Recommended layout:

```text
.refactor-radar/runs/{run_id}/
  scan/
    index.scip
    scan-result.json
    work-manifest.json
  artifacts/
    {work_item_id}.json
```

The scanner preassigns artifact paths under the run's `artifacts/` directory.
Summarizer subagents write only to those paths.

## Work Items

A scanner work item should be small and sufficient for a Codex summarizer
subagent.

Recommended fields:

```json
{
  "work_item_id": "wi-...",
  "run_id": "rr-...",
  "project_id": "file:///...",
  "crate_key": "crate-key",
  "element_id": "rr-element-v2:...",
  "source_file": "src/source_span.rs",
  "kind": "impl_block",
  "display_name": "impl SourceSpan",
  "parent_locator": "module source_span",
  "aspect": "5w",
  "artifact_path": ".refactor-radar/runs/rr-.../artifacts/wi-....json",
  "required_tools": [
    "get_element",
    "find_references",
    "find_related_symbols",
    "get_source_span"
  ],
  "acceptance_criteria": [
    "write exactly one JSON artifact to artifact_path",
    "return work_item_id, artifact_path, sha256, and status"
  ]
}
```

The work item should not include full source text. The summarizer can call MCP
evidence tools when it needs context.

## Scanner Modules

Add focused modules to `crates/rr-mcp/src` or a new `crates/rr-scan` crate if
the implementation becomes too large. Start in `rr-mcp` only if the code stays
small and protocol-bound.

Recommended `rr-mcp` modules:

```text
scan_crate_params.rs
scan_crate_result.rs
scan_file_status.rs
scan_element_status.rs
scan_work_item.rs
scan_artifact_db.rs
scan_run_state.rs
scan_hash.rs
scan_source_slice.rs
```

If the deterministic scanner logic grows beyond protocol plumbing, move the
non-MCP code into:

```text
crates/rr-scan
```

and let `rr-mcp` only expose the MCP tool boundary.

## Error Handling

Follow the repo `AGENTS.md` rules.

- Do not use `anyhow`.
- Add typed errors with `thiserror`.
- Every error variant must carry `ErrorLocation`.
- Constructors must use `#[track_caller]`.
- Provide stable `message()` text for protocol mapping.

Scanner-specific errors should cover:

- invalid crate path
- failed run directory creation
- failed SCIP generation
- failed SCIP decoding
- unsupported SCIP position encoding
- source file missing for SCIP document
- source slice out of bounds
- artifact DB read failure
- artifact DB parse failure
- artifact DB schema version mismatch
- duplicate deterministic element ID
- ambiguous parent locator
- work manifest write failure

## Relationship To Summarizer Orchestration

The scanner does not spawn subagents. Codex does that after receiving the work
manifest.

The scanner's output is the input to a Codex-native skill flow:

```text
$refactor-radar-build {crate}
  -> scan_crate
  -> spawn refactor-radar-summarizer subagents for work_items
  -> register_summary_artifacts
  -> complete_summary_run
```

`register_summary_artifacts` and `complete_summary_run` should be planned
separately, but the scanner must preassign paths and persist enough run state
for those tools to validate receipts without reading summary contents into
Codex context.

## Implementation Phases

### Phase 1: Schema and Tool Skeleton

- Add `ScanCrateParams`.
- Add `ScanCrateResult`.
- Add `ScanWorkItem`.
- Add file and element count structs.
- Register `scan_crate` in `McpServer`.
- Return a minimal result after running existing SCIP generation and load logic.

### Phase 2: Run Directory and SCIP Artifact

- Create `.refactor-radar/runs/{run_id}/scan`.
- Save `index.scip` there by default.
- Reuse `rr_scip::generate_rust_scip`.
- Decode and cache the model.
- Persist `scan-result.json` with compact metadata.

### Phase 3: Artifact DB Read Model

- Add JSON artifact DB structs.
- Load missing DB as empty state.
- Enforce `schema_version`.
- Implement normalized relative path handling.
- Implement file hashing.

### Phase 4: File Hash Gate

- Compare current file hashes with artifact DB records.
- Mark unchanged files as skipped.
- Detect new, changed, and deleted files.
- Ensure unchanged files produce no work items.

### Phase 5: Child Projection and Hashing

- For changed files, walk SCIP document definitions.
- Filter to summarizable kinds, including impl blocks.
- Use ranges only to locate current source slices.
- Compute deterministic parent locators.
- Compute element IDs.
- Compute element content hashes.
- Compare prior element/aspect hashes.
- Emit work items only for changed or missing aspects.

### Phase 6: Manifest Persistence

- Write `work-manifest.json`.
- Preassign artifact paths under the run directory.
- Return compact work items to Codex.
- Ensure the manifest can be consumed later by artifact registration tools.

### Phase 7: Tests

Add focused tests for:

- unchanged file skips all children
- changed file schedules only changed child
- inserted line above a function does not change the function content hash
- function body change changes the function content hash
- field type change changes the field content hash
- impl block body change changes the impl block content hash
- sibling method change changes the impl block hash but not unchanged sibling
  method hashes
- SCIP `local N` symbols are never used as element IDs
- ranges are not content hash inputs
- duplicate deterministic element IDs are reported as typed errors
- missing artifact DB starts from empty state
- malformed artifact DB reports a typed parse error

## Verification

Run the narrow checks first:

```bash
cargo test -p rr-mcp
cargo test -p rr-scip
```

Then run the broader workspace check if the scanner touches shared types:

```bash
cargo test
cargo check
```

Manual verification with `crates/rr-core`:

1. Run `scan_crate` for `crates/rr-core`.
2. Confirm the first run schedules expected work items.
3. Run it again without changes.
4. Confirm zero work items.
5. Insert a blank line above a function.
6. Confirm the file is changed but unchanged children are skipped.
7. Change one function body.
8. Confirm only that function's relevant aspects are scheduled.

## Non-Goals

- Do not implement summarizer subagents in this plan.
- Do not implement artifact registration in this plan.
- Do not implement final publish semantics in this plan.
- Do not replace SCIP with tree-sitter.
- Do not persist raw SCIP symbols as durable IDs.
- Do not build a semantic quality auditor for generated summaries.

## Design Summary

The scanner MCP is the token-saving gatekeeper. It should spend CPU and local
I/O to avoid spending model tokens.

The key design is:

```text
SCIP document path + source file hash
  -> skip whole unchanged files

SCIP current-scan bounds + source slicing
  -> hash child content without hashing positions

RefactorRadar deterministic element IDs
  -> stable artifact DB keys without SCIP IDs

compact work manifest
  -> Codex subagents summarize only changed/missing aspects
```
