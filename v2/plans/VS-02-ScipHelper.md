# VS-02: SCIP Helper

## Objective

Add `crates/rr-scip` as RefactorRadar's small curated SCIP helper library.

This crate reads upstream SCIP artifacts and exposes low-level helper APIs for
Rust scanner binaries. It does not convert SCIP into RefactorRadar records.

## Depends On

- VS-01 for workspace cleanup and the new crate layout.
- Existing upstream SCIP submodule at `submodules/scip`.
- Root workspace dependency `scip = { path = "submodules/scip/bindings/rust" }`.

`rr-scip` does not depend on `rr-data-model`.

## Deliverables

- Add `crates/rr-scip`.
- Add binary `.scip` parsing.
- Add SCIP JSON parsing if still needed.
- Add format detection.
- Add range and enclosing-range helpers.
- Add symbol/occurrence traversal helpers.
- Add typed errors and tests.

## Crate Shape

Recommended files:

```text
crates/rr-scip/
  Cargo.toml
  src/
    lib.rs
    error.rs
    result.rs
    format.rs
    read_index.rs
    range.rs
    occurrence.rs
    symbol.rs
```

## Responsibilities

`rr-scip` should:

- read binary `.scip` files into `scip::types::Index`
- read SCIP JSON into `scip::types::Index`
- detect format from extension or explicit input
- prefer typed occurrence ranges when present
- fall back to legacy SCIP range fields
- expose single-line and multi-line range helpers
- expose enclosing-range helpers
- expose small iterators or functions for document symbols and occurrences

`rr-scip` should not:

- run `rust-analyzer`
- run `scip-dotnet`
- create `rr-data-model` records
- compute deterministic RefactorRadar IDs
- compute source hashes
- decide which symbols are artifact-eligible
- expose an MCP tool
- become a runtime DLL or binary boundary

## Range Semantics

Support:

- `typed_range`
- `range`
- `typed_enclosing_range`
- `enclosing_range`

Return a small helper type local to `rr-scip`, such as:

```rust
pub struct ScipRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}
```

Do not convert this into `rr-data-model::Range` in this crate. Language scanner
crates own that conversion.

## Evidence From v1

The v1 `rr-scip` crate already contains useful code for:

- `generate_rust_scip`
- `format`
- `project_scip_index`
- range helpers in `scip_to_core`

Do not copy `scip_to_core` as a projection layer. It returns v1
`rr_core::SemanticModel` and uses SCIP symbols as stable IDs. That behavior is
not valid for v2.

Reusable parts are:

- format detection
- protobuf/JSON parsing
- occurrence range extraction
- enclosing range extraction
- range containment helpers

## Error Handling

Add `ScipError` and `ScipResult<T>`.

Use the repo error pattern:

- `thiserror`
- `ErrorLocation`
- `#[track_caller]` constructors
- stable `message()` method

Initial errors should cover:

- unknown format
- file read failure
- binary parse failure
- JSON parse failure
- invalid range shape
- invalid negative range coordinate

## Tests

Add focused tests for:

- format detection for `.scip`
- format detection for `.json`
- unknown extension error
- binary SCIP fixture round trip
- JSON SCIP fixture parse
- typed range preferred over legacy range
- typed enclosing range preferred over legacy enclosing range
- malformed ranges rejected
- range containment helper

## Verification

Run:

```bash
cargo test -p rr-scip
cargo check -p rr-scip
```

## Non-Goals

- Do not build `rr-data-model` values.
- Do not implement Rust scanner behavior.
- Do not implement C# scanner behavior.
- Do not expose MCP tools.
