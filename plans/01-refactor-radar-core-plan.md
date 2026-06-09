# Plan: refactor-radar-core

## Objective

Create `crates/refactor-radar-core` as the crate that owns RefactorRadar's
serializable semantic model. It must contain no scanner, report, or MCP server
logic. Its job is to define stable IDs, source spans, elements, references,
call edges, project summaries, evidence records, and 5W brief data structures
used by the other crates.

## Constraints

- Follow `AGENTS.md` Rust error handling rules.
- Do not use `anyhow`.
- Define crate-local `CoreError` and `CoreResult<T>`.
- Every error variant must carry `location: ErrorLocation`.
- Public error constructors must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Provide stable plain messages through `CoreError::message()`.
- Use exactly one named type definition per Rust source file other than
  `lib.rs`.
- Keep `crates/wip` untouched.

## Workspace Setup

Create the crate with:

```bash
cargo new --lib crates/refactor-radar-core --name refactor-radar-core --vcs none
mkdir -p crates/refactor-radar-core/tests
```

Add it to the root workspace member list after `crates/wip`.

Ensure root workspace dependencies include `error-location`,
`serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`,
and `thiserror`.

Use this crate manifest shape:

```toml
[package]
name = "refactor-radar-core"
version.workspace = true
edition.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
error-location.workspace = true
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_json.workspace = true
```

## Source Layout

Create these files under `crates/refactor-radar-core/src`:

```text
lib.rs
call_edge_summary.rs
core_error.rs
core_result.rs
element_brief.rs
element_kind.rs
element_summary.rs
evidence_record.rs
file_summary.rs
five_w_summary.rs
function_parameter_summary.rs
function_signature_summary.rs
language.rs
project_report_summary.rs
project_summary.rs
semantic_model.rs
source_span.rs
span_id.rs
stable_id.rs
symbol_id.rs
symbol_reference_summary.rs
type_reference_summary.rs
```

`lib.rs` should declare and re-export every module/type, but must not define
named types. Keep all named type definitions in their own non-`lib.rs` files.

## Model Requirements

Implement these minimum types and fields:

- `StableId`, `SymbolId`, and `SpanId` as transparent string wrappers.
- All model structs, enums, and ID wrappers except `CoreError`/`CoreResult`
  must derive `Serialize`, `Deserialize`, `Debug`, `Clone`, and `PartialEq`;
  derive `Eq` where the fields allow it. ID wrappers must use
  `#[serde(transparent)]` so serialized IDs are plain strings.
- `Language` with `Rust` and `Unknown(String)`.
- `ElementKind` with Rust-relevant SCIP `SymbolInformation.Kind` variants:
  `AssociatedType`, `Constant`, `Constructor`, `Enum`, `EnumMember`, `Field`,
  `File`, `Function`, `Macro`, `Method`, `MethodReceiver`, `Module`,
  `Namespace`, `Package`, `Parameter`, `SelfParameter`, `StaticMethod`,
  `StaticVariable`, `Struct`, `Trait`, `TraitMethod`, `Type`, `TypeAlias`,
  `TypeParameter`, `Union`, `Variable`, and `Unknown(String)`.
- `SourceSpan` with span ID, document path, and one-based public `start_line`,
  `start_column`, `end_line`, and `end_column` fields, plus deterministic
  `SpanId` generation.
- `ProjectSummary` with project ID, root, producer name, and producer version.
- `FileSummary` with document path, language, symbol count, and occurrence
  count.
- `ElementSummary` with stable ID, SCIP symbol, language, kind, display name,
  package, enclosing symbol, definition span, signature, documentation, and
  reference count.
- `FunctionSignatureSummary` with optional full signature text, parameters,
  and optional return type.
- `FunctionParameterSummary` with parameter name, optional SCIP parameter
  symbol, and optional type reference.
- `TypeReferenceSummary` with display text and optional SCIP symbol.
- `SymbolReferenceSummary` with referenced symbol, source span, document path,
  and raw SCIP `symbol_roles` bitset as `i32` so Definition, Import,
  WriteAccess, ReadAccess, Generated, Test, and ForwardDefinition flags are
  preserved exactly.
- `CallEdgeSummary` with enclosing symbol, referenced symbol, evidence span,
  and confidence label.
- `SemanticModel` with project, files, elements, references, call edges, and
  spans.
- `ProjectReportSummary` for derived report counts and hotspot files, with
  project ID, document count, element count, reference count, and hotspot file
  labels.
- `EvidenceRecord` for report output, with ID, kind, confidence label,
  source spans, and separate `confirmed`, `inferred`, and `unknown` arrays.
- `FiveWSummary` for report output, with `who`, `what`, `where_`, `when`,
  `why`, and `how` arrays.
- `ElementBrief` for report output, with the original element summary, a
  `FiveWSummary`, and evidence records.

Use serde renames so `FiveWSummary` serializes as `who`, `what`, `where`,
`when`, `why`, and `how`; the Rust field for `where` must be named `where_`
and use `#[serde(rename = "where")]`.

## Behavior

- `StableId::from_scip_symbol(symbol)` must return `scip:{symbol}`.
- `SpanId::new(project_id, document_path, line, column)` must be deterministic
  and scoped by project, file, and one-based public start position.
- `SourceSpan::from_scip_range(project_id, document_path, range)` must accept
  SCIP `[line, start, end]` and
  `[start_line, start_col, end_line, end_col]` ranges.
- `SourceSpan::from_scip_range` must store `document_path` and generate the
  span ID with `SpanId::new(project_id, document_path, start_line,
  start_column)` after converting the start position to one-based public
  coordinates.
- SCIP positions are zero-based; all public `SourceSpan` fields must be
  one-based.
- `SourceSpan::from_scip_range` must reject ranges with any negative position,
  any element count other than 3 or 4, an end line before the start line, or an
  end column before the start column on the same line. An omitted deprecated
  SCIP range (`[]`) is invalid.
- Zero-width SCIP ranges are valid when encoded with 3 or 4 elements and
  `start == end`.
- Invalid ranges must return `CoreError::InvalidScipRange`.
- `SemanticModel` should provide helper lookup methods named `element_by_id`,
  `span_by_id`, `has_project_id`, `outgoing_call_edges`, and
  `incoming_call_edges`. `element_by_id` must accept either a stable ID string
  or the preserved raw SCIP symbol string.

## Tests

Create `crates/refactor-radar-core/tests/model_tests.rs` with given/when/then
test names that verify:

- Stable ID generation is deterministic.
- Original SCIP symbol strings are preserved in `ElementSummary`.
- SCIP source spans project to one-based line and column fields.
- `SourceSpan::from_scip_range` rejects omitted and malformed ranges while
  accepting zero-width ranges, and invalid ranges return
  `CoreError::InvalidScipRange` with a stable `CoreError::message()` value.
- `ElementBrief` serialization exposes `five_w.who`, `what`, `where`, `when`,
  `why`, and `how`.
- `SemanticModel` serializes and deserializes through JSON with ID wrappers as
  strings.
- `SemanticModel` helper methods resolve elements by stable ID and raw SCIP
  symbol, resolve spans by span ID, check the project ID, and return incoming
  and outgoing call edges.
- Evidence records preserve separate `confirmed`, `inferred`, and `unknown`
  arrays.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-core
cargo clippy -p refactor-radar-core --all-targets -- -D warnings
```

Also verify:

```bash
for file in crates/refactor-radar-core/src/*.rs; do
    if [ "$(basename "$file")" = "lib.rs" ]; then
        continue
    fi
    count=$(rg -n '^((pub|pub\([^)]*\)) )?(struct|enum|trait|type)\b' "$file" | wc -l)
    if [ "$count" -ne 1 ]; then
        echo "$file has $count named type definitions"
        exit 1
    fi
done

if rg -n 'anyhow|unwrap\(|expect\(|panic!|dbg!' crates/refactor-radar-core; then
    exit 1
fi
```

## Done Criteria

- `refactor-radar-core` compiles independently.
- It contains only model, ID, span, and typed error definitions.
- It exposes stable serialized shapes for downstream SCIP, report, and MCP
  crates.
- Tests and clippy pass without `anyhow`, `unwrap()`, `expect()`, `panic!`, or
  `dbg!`.
