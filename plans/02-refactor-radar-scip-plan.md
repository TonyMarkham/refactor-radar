# Plan: refactor-radar-scip

## Objective

Create `crates/refactor-radar-scip` as the SCIP ingestion and projection crate.
It must run `rust-analyzer scip`, load binary or protobuf JSON SCIP indexes,
and project SCIP facts into `refactor-radar-core::SemanticModel`.

SCIP is the canonical Rust semantic fact source for v1. Do not add a
tree-sitter path in this crate.

## Constraints

- Follow `AGENTS.md` Rust error handling rules.
- Do not use `anyhow`.
- Define crate-local `ScipError` and `ScipResult<T>`.
- Every error variant must carry `location: ErrorLocation`.
- Public error constructors must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Provide stable plain messages through `ScipError::message()`.
- Use at most one named type definition per Rust source file; function-only
  modules may have zero named type definitions.
- Missing `rust-analyzer` must be reported clearly and must not trigger
  auto-install behavior.
- Subprocess stderr must be captured and returned or embedded in typed errors.
- Runtime query/projection must be local-first and not require network access.

## Workspace Setup

Before creating this crate, verify that `crates/refactor-radar-core` exists and
is already listed in the root workspace. If it is missing, stop and create the
core crate first; do not create a placeholder core crate in this plan.

Create the crate with:

```bash
cargo new --lib crates/refactor-radar-scip --name refactor-radar-scip --vcs none
mkdir -p crates/refactor-radar-scip/tests
```

Add it to the root workspace member list after `crates/refactor-radar-core`.

Use this crate manifest shape:

```toml
[package]
name = "refactor-radar-scip"
version.workspace = true
edition.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
error-location.workspace = true
protobuf.workspace = true
protobuf-json-mapping.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
scip.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

Ensure root workspace dependencies include:

```toml
protobuf = "=3.7.2"
protobuf-json-mapping = "=3.7.2"
scip = { path = "submodules/scip/bindings/rust" }
tempfile = "3"
tokio = { version = "1", features = ["macros", "process", "rt-multi-thread"] }
```

## Source Layout

Create these files under `crates/refactor-radar-scip/src`:

```text
lib.rs
format.rs
generate_rust_scip.rs
project_scip_index.rs
scip_error.rs
scip_result.rs
scip_to_core.rs
symbol_kind_projection.rs
```

`lib.rs` should declare modules and re-export `Format`,
`generate_rust_scip`, `project_scip_index`, `ScipError`, `ScipResult`, and
`scip_to_core`.

## Format Loading

Implement `Format` with:

- `Binary` for `.scip`.
- `Json` for `.json`.
- `Format::detect(path)` that errors on unknown extensions.

Implement `project_scip_index(path, format)`:

- Detect format when `format` is `None`.
- Parse binary `.scip` with `Index::parse_from_bytes`.
- Parse protobuf JSON with `protobuf_json_mapping::parse_from_str`.
- Return `ScipError::ReadFailed`, `ParseFailed`, or `UnknownFormat` as
  appropriate.
- Project the parsed index with `scip_to_core`.

## rust-analyzer Invocation

Implement `generate_rust_scip`:

```text
rust-analyzer scip <project_path> --output <output_path>
```

Inputs:

- `project_path`
- `output_path`
- optional `rust_analyzer_path`
- optional `config_path`
- `exclude_vendored_libraries`
- optional `num_threads`

Behavior:

- Use `tokio::process::Command`.
- Add `--config-path`, `--exclude-vendored-libraries`, and `--num-threads`
  only when requested.
- Map `ErrorKind::NotFound` to `RustAnalyzerMissing`.
- Map other launch failures to `RustAnalyzerLaunchFailed`.
- Map nonzero exit to `RustAnalyzerFailed` with status and stderr.
- Return stderr on success so callers can keep diagnostics out of stdout.

## SCIP Projection

Implement `scip_to_core(index)` with these rules:

- `Index.metadata.project_root` becomes `ProjectSummary.project_root` and the
  project ID for v1.
- `Index.metadata.tool_info` becomes producer name and version.
- Each `Document.relative_path` becomes a `FileSummary`.
- Each `Document.symbols` entry becomes an `ElementSummary`.
- Preserve original SCIP symbols in both `scip_symbol` and `SymbolId`.
- `StableId` comes from `StableId::from_scip_symbol`.
- Project symbol kinds through `symbol_kind_projection`.
- Derive package name with `scip::symbol::parse_symbol` when possible.
- Definition occurrences attach `definition_span`.
- Non-definition occurrences become `SymbolReferenceSummary`.
- Reference counts are counted across all documents.
- Function-like symbols are function, method, static method, and trait method.
- Derive `CallEdgeSummary` values from function-like references inside a
  function-like definition enclosing range.
- Label call-edge confidence as
  `scip_reference_inside_function_enclosing_range`.

Range handling:

- Prefer `Occurrence.typed_range`; fall back to deprecated `range`.
- Prefer `Occurrence.typed_enclosing_range`; fall back to deprecated
  `enclosing_range`.
- Convert `SingleLineRange` and `[line, start, end]` to one-line spans.
- Convert `MultiLineRange` and `[start_line, start_col, end_line, end_col]` to
  multi-line spans.
- Map range projection failures to `ScipError::CoreProjectionFailed`.

Signature handling:

- Preserve `SymbolInformation.signature_documentation.text`.
- The local `scip` binding exposes this as `Signature.text`; rust-analyzer's
  legacy Document-shaped payload still populates that field by protobuf number.
- Extract parameter text and return type text from signature text.
- Link parameter SCIP symbols when `Parameter` or `SelfParameter` symbols have
  enough enclosing-symbol or definition evidence.
- Project return-type SCIP IDs only from `Signature.occurrences`.
- If only signature text is available, mark type references as
  `signature_text_only`.
- Do not infer return-type SCIP IDs from normal document occurrences alone.

## Tests

Create `crates/refactor-radar-scip/tests/project_scip_index_tests.rs` with
given/when/then test names that verify:

- Binary `.scip` projection.
- Protobuf JSON projection.
- Format detection by extension.
- Unknown extension returns `ScipError::UnknownFormat`.
- Documents, symbols, occurrences, roles, and ranges map into the core model.
- Typed occurrence ranges take precedence over deprecated vectors.
- Typed enclosing ranges are used for call-edge derivation.
- Function-like references produce `CallEdgeSummary`.
- Legacy rust-analyzer signature text is recovered from `Signature.text`.
- Return-type SCIP IDs are preserved when `Signature.occurrences` provides
  them.
- Missing rust-analyzer maps to `RustAnalyzerMissing`.
- Non-NotFound launch failures map to `RustAnalyzerLaunchFailed`.
- Nonzero rust-analyzer exit captures stderr in `RustAnalyzerFailed`.
- Successful rust-analyzer stderr diagnostics are returned, not printed.

Use in-memory SCIP fixtures with `scip::types::Index` and fake unix shell
scripts for rust-analyzer subprocess behavior. Do not require a real
rust-analyzer binary for unit tests.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-scip
cargo clippy -p refactor-radar-scip --all-targets -- -D warnings
```

Also verify:

```bash
for file in crates/refactor-radar-scip/src/*.rs; do
    if [ "$(basename "$file")" = "lib.rs" ]; then
        continue
    fi
    count=$(rg -n '^((pub|pub\([^)]*\)) )?(struct|enum|trait|type)\b' "$file" | wc -l)
    if [ "$count" -gt 1 ]; then
        echo "$file has $count named type definitions"
        exit 1
    fi
done

if rg -n 'anyhow|unwrap\(|expect\(|panic!|dbg!' crates/refactor-radar-scip; then
    exit 1
fi
```

## Done Criteria

- `refactor-radar-scip` can generate SCIP with rust-analyzer when supplied.
- It can load binary and JSON SCIP inputs.
- It projects SCIP facts into the core semantic model without raw source
  extraction.
- Signature, range, reference, and call-edge limitations are labeled
  accurately.
- Tests and clippy pass without `anyhow`, `unwrap()`, `expect()`, `panic!`, or
  `dbg!`.
