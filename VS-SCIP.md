# Plan: MCP SCIP CRUD Using rust-analyzer SCIP Output

## Summary

Use SCIP as RefactorRadar's canonical semantic model for Rust. Do not build a
custom tree-sitter-to-SCIP Rust indexer in v1. Instead, MCP invokes
`rust-analyzer scip` to generate canonical `.scip` files, then RefactorRadar
provides typed file I/O and CRUD tools over those SCIP indexes.

## Key Changes

- Add `refactor-radar-scip` for reading/writing SCIP binary and protobuf JSON
  files, plus CRUD helpers over `scip::types::Index`.
- Add `refactor-radar-mcp` for stdio MCP tools that generate and modify SCIP
  files.
- Use `submodules/scip/bindings/rust` as the `scip` path dependency.
- Use `submodules/rust-analyzer` as the local reference source for
  rust-analyzer's `scip` CLI behavior and tests.
- Add `protobuf-json-mapping = "=3.7.2"` for protobuf JSON support.
- Add no tree-sitter Rust scanner in v1; rust-analyzer is the Rust SCIP
  producer.
- Keep typed crate-local errors with `ErrorLocation`; no `anyhow` in
  RefactorRadar crates.

## MCP Interface

- `generate_rust_scip(path, output_path, rust_analyzer_path?, config_path?)`
  - Runs `rust-analyzer scip <path> --output <output_path>`.
  - Adds `--config-path <config_path>` when supplied.
  - Overwrites the output file according to rust-analyzer behavior.
  - Returns typed MCP error if rust-analyzer is missing or exits nonzero.
- `read_scip_index(path, format?)`
- `update_scip_index_metadata(path, format?, metadata)`
- `delete_scip_index(path)`
- `create_scip_document(path, format?, document)`
- `get_scip_document(path, format?, relative_path)`
- `update_scip_document(path, format?, document)`
- `delete_scip_document(path, format?, relative_path)`
- `create_scip_symbol(path, format?, relative_path, symbol)`
- `get_scip_symbol(path, format?, symbol)`
- `update_scip_symbol(path, format?, relative_path, symbol)`
- `delete_scip_symbol(path, format?, symbol)`

## Behavior

- Format detection:
  - `.scip` means binary protobuf.
  - `.json` means protobuf JSON.
  - Unknown extensions require explicit `format`.
- CRUD operates directly on the generated file.
- Updates use replace-by-ID semantics:
  - documents by `relative_path`
  - symbols by `SymbolInformation.symbol`
- Create fails on duplicate ID; update/delete fail on missing ID.
- MCP never writes protocol-irrelevant output to stdout; subprocess stderr is
  captured and returned in typed errors when relevant.

## Test Plan

- `refactor-radar-scip`:
  - round-trips binary `.scip`
  - round-trips protobuf JSON
  - CRUDs metadata, documents, and symbols
  - rejects duplicate create and missing update/delete
- `refactor-radar-mcp`:
  - generates SCIP through a fake rust-analyzer executable in tests
  - maps missing rust-analyzer to a typed MCP error
  - reads/modifies generated SCIP files
  - returns not-found errors for unknown documents and symbols
- Workspace checks:
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`

## Assumptions

- rust-analyzer is the authoritative Rust indexer for v1.
- Missing rust-analyzer is reported clearly; RefactorRadar does not auto-install
  it.
- Local verification found `rust-analyzer` listed in
  `submodules/scip/README.md` as a Rust SCIP emitter.
- rust-analyzer CLI behavior should be verified from
  `submodules/rust-analyzer` before implementation, especially the local
  `scip` command flags and output semantics.
- Sources: local `submodules/scip/README.md`, local
  `submodules/rust-analyzer`, rust-analyzer CLI source at
  <https://rust-lang.github.io/rust-analyzer/src/rust_analyzer/cli/flags.rs.html>,
  rust-analyzer SCIP implementation at
  <https://rust-lang.github.io/rust-analyzer/src/rust_analyzer/cli/scip.rs.html>,
  and protobuf JSON mapping docs at
  <https://docs.rs/protobuf-json-mapping/latest/protobuf_json_mapping/>.
