# Plan: Rust SCIP Utility Library Inspired by the SCIP CLI

## Summary

Create a Rust library that ports the useful behavior of the upstream `scip`
CLI into reusable, testable APIs that RefactorRadar MCP and future CLI commands
can call directly.

This is not a direct one-for-one CLI rewrite. The goal is to move the CLI's
index inspection, stats, linting, and snapshot/test concepts into a Rust library
with structured return values, typed errors, and no stdout side effects.

Upstream SCIP CLI facts from `submodules/scip/cmd/scip`:

- `scip print` reads a SCIP index and prints debug or JSON output.
- `scip stats` counts documents, occurrences, definitions, document sizes, and
  optional lines of code.
- `scip lint` validates internal index consistency.
- `scip snapshot` renders source-like snapshot files for golden inspection.
- `scip test` validates a SCIP index against annotated test files.
- `scip expt-convert` converts a SCIP index to SQLite and should be deferred.

## Goals

- Provide a Rust library layer that MCP can call without shelling out to the Go
  `scip` CLI.
- Reuse the local `submodules/scip/bindings/rust` crate for SCIP types and
  symbol parsing/formatting.
- Support binary `.scip` and protobuf JSON index files.
- Return structured data for stats, lint diagnostics, snapshots, and test
  reports.
- Keep all output rendering outside the core library so MCP can return compact
  JSON and a future CLI can print text.
- Preserve enough upstream behavior that the Go `scip` CLI can be used as an
  oracle in fixture tests.

## Hard Constraints

- Follow `AGENTS.md` for Rust error handling.
- Do not use `anyhow` in RefactorRadar application or library crates.
- Define crate-local typed errors with `thiserror`, `ErrorLocation`, and a
  result alias.
- Public error constructors must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Library code must not write to stdout or stderr.
- Do not modify the SCIP submodule to add RefactorRadar-specific behavior.
- Keep generated SCIP protobuf types behind a small RefactorRadar API surface.
- Treat upstream `scip expt-convert` as deferred unless SQLite-backed querying
  becomes a concrete requirement.

## Target Crate

Add or use `crates/refactor-radar-scip` as the library crate.

Suggested module layout:

```text
lib.rs
format.rs
index_io.rs
json_print.rs
lint_diagnostic.rs
lint_index.rs
range_format.rs
scip_error.rs
scip_result.rs
snapshot_file.rs
snapshot_index.rs
stats_summary.rs
stats_index.rs
test_report.rs
test_index.rs
```

Keep one named type per file. Function-only modules may contain helper
functions without named types.

## Public API Shape

Core I/O:

```rust
read_index(path, format) -> ScipResult<scip::types::Index>
write_index(path, format, index) -> ScipResult<()>
index_to_json(index) -> ScipResult<String>
index_from_json(text) -> ScipResult<scip::types::Index>
```

Stats:

```rust
stats_index(index, options) -> ScipResult<StatsSummary>
```

Lint:

```rust
lint_index(index) -> Vec<LintDiagnostic>
```

Snapshot:

```rust
snapshot_index(index, source_root, options) -> ScipResult<Vec<SnapshotFile>>
```

Test:

```rust
test_index(index, source_root, options) -> ScipResult<TestReport>
```

The MCP server should call these APIs and map typed errors into MCP errors. A
future CLI can call the same APIs and handle printing.

## Phase 1: Index I/O and JSON

Implement format detection:

- `.scip` means binary protobuf.
- `.json` means protobuf JSON.
- unknown extensions require explicit format.

Read binary indexes with `protobuf::Message::parse_from_bytes`.
Write binary indexes with the existing `scip::write_message_to_file` helper or
equivalent buffered write.

Read/write JSON with `protobuf-json-mapping = "=3.7.2"`.

Acceptance:

- Binary `.scip` fixture round-trips.
- JSON fixture round-trips.
- Unknown extension without explicit format returns a typed error.
- JSON output is deterministic enough for snapshot tests.

## Phase 2: Stats Port

Port the useful parts of upstream `scip stats` into `StatsSummary`.

Minimum fields:

- document count
- occurrence count
- definition count
- external symbol count
- document symbol count
- document size distribution in bytes
- occurrence count distribution by document
- definition count distribution by document
- optional lines of code when project root and source files are available

Use an internal percentile helper instead of adding a broad statistics
dependency unless the dependency is clearly worth it.

Line counting should be non-fatal. If source files cannot be read, return stats
with a warning field rather than failing the whole operation.

Acceptance:

- Stats for `/tmp/Forksmith.scip` match the obvious counts from
  `scip print --json` or `protoc --decode`.
- Stats tests compare selected fields against upstream `scip stats` output for
  a tiny fixture.

## Phase 3: Lint Port

Port upstream `scip lint` as a pure function returning diagnostics rather than
joined errors.

Initial diagnostics:

- empty external symbol string
- duplicate external symbol info
- empty document path
- duplicate document path
- empty document symbol string
- duplicate symbol information within documents
- relationships pointing to missing symbols
- occurrences pointing to missing symbols
- duplicate occurrences at the same range and role
- invalid global symbol format when the Rust SCIP symbol parser rejects it
- definition and forward-definition role set on the same occurrence

Diagnostic shape:

- severity: warning or error
- code: stable diagnostic code
- message: stable plain message
- document path when relevant
- range when relevant
- SCIP symbol when relevant

Acceptance:

- Pure lint tests cover each diagnostic with tiny constructed indexes.
- Fixture diagnostics are stable and do not depend on display formatting.
- MCP can return lint diagnostics as compact JSON.

## Phase 4: Snapshot Port

Port the useful behavior of upstream `scip snapshot` for golden inspection.

Library behavior:

- Accept an index and source root.
- For each SCIP document, read the source file.
- Render a snapshot string with occurrence markers and symbol annotations.
- Return `Vec<SnapshotFile>` containing relative path and rendered contents.
- Do not write files in the library.

Options:

- project root override
- comment syntax, default `//`
- strict mode for missing source files
- optional path filter

Acceptance:

- Snapshot output for a tiny fixture matches an expected snapshot.
- Missing source files produce warnings in non-strict mode.
- Strict mode returns a typed error for missing source files.

## Phase 5: Test Port

Port upstream `scip test` as a library validator over annotated source files.

Library behavior:

- Parse expected annotations from source comments.
- Match expected definitions/references against SCIP occurrences.
- Support path filters.
- Optionally check that every source file has a SCIP document.
- Return a structured `TestReport` with pass/fail counts and diagnostics.

Keep the first version narrow. Support the annotation forms needed by local
fixtures before trying to match every upstream edge case.

Acceptance:

- A tiny annotated fixture passes.
- A fixture with one wrong reference returns a failing `TestReport`.
- Report diagnostics include file, line, expected symbol, and actual mismatch.

## Phase 6: Deferred SQLite Convert

Do not port `scip expt-convert` in v1.

Reasons:

- It is marked experimental upstream.
- MCP does not need SQLite to answer first-pass summary and 5W queries.
- RefactorRadar can build purpose-specific in-memory indexes first.

Revisit SQLite only if large index query performance requires it. If added,
use a separate optional feature and avoid making `rusqlite` part of the default
MCP dependency graph.

## Phase 7: MCP Integration

MCP should call this library rather than invoking the Go `scip` CLI.

Useful MCP tools backed by the library:

- `load_scip_project(path, format?)`
- `get_scip_stats(project_id)`
- `lint_scip_project(project_id)`
- `print_scip_json(project_id)` for debugging only, with size limits
- `snapshot_scip_project(project_id, source_root?, strict?)`
- `test_scip_project(project_id, source_root?, filters?)`

For normal RefactorRadar work, MCP should expose higher-level summaries and 5W
briefs from projected semantic models. Raw SCIP JSON should be explicitly
debug-oriented and size-limited.

## Phase 8: Verification Against Upstream CLI

Use the upstream Go CLI or pinned release binary as a test oracle in integration
tests where practical.

Suggested manual checks:

```bash
scip print --json /tmp/Forksmith.scip
scip stats --from /tmp/Forksmith.scip
scip lint /tmp/Forksmith.scip
```

Rust library checks should compare:

- selected JSON fields for print
- selected count fields for stats
- diagnostic codes/classes for lint
- snapshot output for tiny fixtures

Do not require the upstream CLI for ordinary unit tests.

## Test Plan

- I/O:
  - binary read/write round-trip
  - JSON read/write round-trip
  - format detection errors
- Stats:
  - empty index
  - one-document index
  - multi-document index with definitions and references
  - missing source root warning
- Lint:
  - duplicate documents
  - duplicate symbols
  - missing relationship target
  - missing occurrence target
  - invalid symbol format
  - conflicting definition roles
- Snapshot:
  - renders markers for definitions and references
  - supports comment syntax override
  - handles missing files in strict and non-strict modes
- Test:
  - passing annotated fixture
  - failing annotated fixture
  - filtered annotated fixture
- Workspace:
  - `cargo fmt --all -- --check`
  - `cargo test -p refactor-radar-scip`
  - `cargo clippy -p refactor-radar-scip --all-targets -- -D warnings`

## Acceptance Criteria

- RefactorRadar has a Rust SCIP utility library that can read, write, inspect,
  count, and lint SCIP indexes without invoking an external CLI.
- Library functions return structured values and typed errors.
- Library code has no stdout/stderr side effects.
- Upstream `scip` CLI remains only a development oracle or optional debugging
  tool.
- MCP can use the library directly for SCIP stats, lint diagnostics, and later
  5W semantic projection.
- SQLite conversion is explicitly deferred.
