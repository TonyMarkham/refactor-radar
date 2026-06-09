# Plan: refactor-radar-report

## Objective

Create `crates/refactor-radar-report` as the reporting and summary derivation
crate. It consumes `refactor-radar-core::SemanticModel` values and produces
project summaries, element 5W briefs, and compact LLM-oriented Markdown briefs.

This crate must derive context from normalized core facts only. It should not
read SCIP files, run rust-analyzer, access source text, or expose MCP handlers.

## Constraints

- Follow `AGENTS.md` Rust error handling rules.
- Do not use `anyhow`.
- Define crate-local `ReportError` and `ReportResult<T>`.
- Every error variant must carry `location: ErrorLocation`.
- Public error constructors must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Provide stable plain messages through `ReportError::message()`.
- Use at most one named type definition per Rust source file.
- Separate confirmed facts, inference, and unknowns explicitly.
- Do not make broad refactor recommendations from 5W summaries alone.

## Prerequisites

Before starting this plan, verify the root workspace already contains
`crates/refactor-radar-core` and `crates/refactor-radar-scip`. The report crate
depends directly on `refactor-radar-core` only; `refactor-radar-scip` must
already exist as the upstream producer of core model data. Do not add a
`refactor-radar-scip` dependency or recreate model types in this crate.

Verify `refactor-radar-core` exports the model surface used here:

- `SemanticModel` with a project ID source, `files`, `elements`,
  `references`, `call_edges`, `element_by_id`, `incoming_call_edges`, and
  `outgoing_call_edges`. `element_by_id` must resolve both stable IDs and raw
  SCIP symbols.
- `ElementSummary`, `ProjectReportSummary`, `FiveWSummary`, `EvidenceRecord`,
  and `ElementBrief`.
- `ElementSummary` fields needed by the element brief facts below, including
  stable ID, raw SCIP symbol, package/crate, enclosing symbol, kind, display
  name, signature, parameters, return type, and definition span.
- Child symbols must be derivable from `model.elements` by matching child
  `enclosing_symbol` values to the parent element's raw SCIP symbol.
- `model.references` entries with element-compatible target IDs or raw SCIP
  symbols, document paths, and role flags needed for hotspots, per-element top
  reference files, returned evidence records, element ranking, and
  test-reference detection.
- `model.call_edges` or the `incoming_call_edges`/`outgoing_call_edges`
  helpers with SCIP-derived function-like relationship data needed for
  incoming/outgoing relationship summaries.

If any prerequisite is missing, stop and implement the prerequisite crate work
first.

## Workspace Setup

Create the crate with:

```bash
cargo new --lib crates/refactor-radar-report --name refactor-radar-report --vcs none
mkdir -p crates/refactor-radar-report/tests
```

After the prerequisites are present, add it to the root workspace member list
after `crates/refactor-radar-scip`.

Use this crate manifest shape:

```toml
[package]
name = "refactor-radar-report"
version.workspace = true
edition.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
error-location.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
thiserror.workspace = true
```

## Source Layout

Create these files under `crates/refactor-radar-report/src`:

```text
lib.rs
build_element_brief.rs
build_project_summary.rs
generate_llm_brief.rs
report_error.rs
report_result.rs
```

`lib.rs` should declare modules and re-export `build_element_brief`,
`build_project_summary`, `generate_llm_brief`, `ReportError`, and
`ReportResult`.

## Project Summary

Implement `build_project_summary(model)`:

- Return `ProjectReportSummary`.
- Count documents from `model.files`.
- Count elements from `model.elements`.
- Count references from `model.references`.
- Compute hotspot files from reference counts per document path.
- Sort hotspots by descending count, then ascending path.
- Render hotspot entries as `path (count)`.

## Element Briefs

Implement `build_element_brief(model, symbol_id, include_inferred,
reference_limit)`:

- Accept either a stable ID or raw SCIP symbol through
  `SemanticModel::element_by_id`.
- Return `ReportError::element_not_found(symbol_id)` for missing elements,
  backed by an `ElementNotFound { symbol_id, location }` variant.
- Include the original `ElementSummary`.
- Build a `FiveWSummary`.
- Include `EvidenceRecord` entries for returned references.
- Match element references by both the selected element's stable ID and raw SCIP
  symbol so references stored in either core-compatible identifier form are
  included.
- Limit reference evidence and related outputs by `reference_limit`.
- Compute per-element top reference files from the matched references and sort
  them by descending count, then ascending path.
- Build child-symbol facts by scanning `model.elements` for entries whose
  `enclosing_symbol` matches the selected element's raw SCIP symbol.
- Build incoming and outgoing function-like relationship facts from
  `SemanticModel::incoming_call_edges` and `SemanticModel::outgoing_call_edges`.

Confirmed facts should include:

- Element came from SCIP `SymbolInformation`.
- Package/crate when known.
- Enclosing symbol when known.
- Kind and display name.
- Signature text when present.
- Parameter names and parameter SCIP IDs when present.
- Return type text and return type SCIP ID when present.
- Child symbols such as enum variants and fields.
- Definition span and top reference files.
- SCIP-derived incoming and outgoing function-like references from call edges.

Inference should include only clearly labeled heuristic statements, such as:

- Many SCIP references may indicate API relevance.

Unknowns should include:

- Git recency is not available from SCIP.
- SCIP occurrence alone does not prove runtime call behavior.
- Return type SCIP symbol is unavailable when only text is confirmed.
- Intended public API status is unknown unless facts support it.

5W content:

- `who`: package and enclosing symbol facts.
- `what`: kind, name, signature, parameters, return type, and child symbols.
- `where`: definition span and top reference files.
- `when`: unknown recency unless future git metadata exists.
- `why`: inference only when `include_inferred` is true.
- `how`: child symbols, test references, and SCIP-derived function-like
  relationships.

Test references are detected when:

- SCIP role flags include the test role. Because this crate does not depend on
  `scip`, use a private local `SCIP_SYMBOL_ROLE_TEST: i32 = 0x20` constant and
  detect the bit with `(symbol_roles & SCIP_SYMBOL_ROLE_TEST) != 0`.
- Path is under a `tests/` directory, including paths that start with `tests/`
  or contain `/tests/`.
- Path ends with `_test.rs` or `_tests.rs`.

## LLM Brief

Implement `generate_llm_brief(model, budget_tokens)`:

- Start with project ID, document count, element count, reference count, and
  hotspots.
- Rank elements by descending matched reference count, display name, then symbol
  ID, using the same stable-ID-or-raw-SCIP-symbol reference matching as element
  briefs.
- Include up to 25 element sections.
- Each section must explicitly render `Confirmed:`, `Inferred:`, and
  `Unknown:`.
- Approximate token budget as `tokens * 4` characters.
- Stop adding lower-priority sections once the next section would exceed the
  budget.

## Tests

Create `crates/refactor-radar-report/tests/brief_tests.rs` with given/when/then
test names that verify:

- Project summary counts documents, elements, references, and hotspots.
- Enum briefs include variants and top reference files.
- Function briefs include signature facts.
- Function briefs include SCIP-derived function-like references.
- Function briefs include parameter SCIP IDs and return-type SCIP IDs when
  available.
- Missing return-type SCIP IDs are labeled as unknown when only text is
  available.
- Test references appear in the `how` section.
- Recency is unknown when git metadata is absent.
- LLM briefs include confirmed, inferred, and unknown sections.
- Token budget removes lower-signal sections first.

Use hand-built `SemanticModel` fixtures. Do not require SCIP parsing or MCP
setup in this crate's tests.

## Verification

Run:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-report
cargo clippy -p refactor-radar-report --all-targets -- -D warnings
```

Also verify:

```bash
for file in $(rg --files crates/refactor-radar-report/src crates/refactor-radar-report/tests -g '*.rs'); do
    count=$(rg -n '^\s*(pub(\([^)]*\))?\s+)?(struct|enum|trait|type)\s+' "$file" | wc -l)
    if [ "$count" -gt 1 ]; then
        echo "$file has $count named type definitions"
        exit 1
    fi
done

if rg -n '\b(anyhow|unwrap\(|expect\(|panic!\(|dbg!\()' crates/refactor-radar-report/src crates/refactor-radar-report/tests; then
    exit 1
fi
```

## Done Criteria

- `refactor-radar-report` derives project summaries, element briefs, 5W
  summaries, and LLM briefs from core models.
- Confirmed, inferred, and unknown content is always separated.
- SCIP-derived function relationships are not overstated as exact AST calls.
- Tests and clippy pass without `anyhow`, `unwrap()`, `expect()`, `panic!`, or
  `dbg!`.
