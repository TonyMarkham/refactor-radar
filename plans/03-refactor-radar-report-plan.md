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
- `StableId::as_str()` and `SymbolId::as_str()` accessors for comparing
  stable IDs, raw SCIP symbols, and reference targets without recreating ID
  wrapper types in this crate.
- The core constructors and fixture-facing model types used by the tests below:
  `StableId::from_scip_symbol`, `SymbolId::new`,
  `SourceSpan::from_scip_range`, `ProjectSummary`, `FileSummary`,
  `Language`, `ElementKind`, `SymbolReferenceSummary`, `CallEdgeSummary`,
  `FunctionSignatureSummary`, `FunctionParameterSummary`, and
  `TypeReferenceSummary`.
- `SemanticModel` must be directly constructible in tests with `project`,
  `files`, `elements`, `references`, `call_edges`, and `spans`.
- The fixture-facing structs above must expose the public fields accessed in
  the snippets below, including `SourceSpan.document_path`,
  `SourceSpan.start_line`, `SourceSpan.start_column`,
  `SymbolReferenceSummary.referenced_symbol_id`,
  `SymbolReferenceSummary.source_span`, `SymbolReferenceSummary.document_path`,
  `SymbolReferenceSummary.symbol_roles`, `CallEdgeSummary.enclosing_symbol_id`,
  `CallEdgeSummary.referenced_symbol_id`, `CallEdgeSummary.evidence_span`,
  `CallEdgeSummary.confidence`, and the signature-summary fields used in the
  test fixture.
- `ElementSummary` fields needed by the element brief facts below, including
  stable ID, raw SCIP symbol, package/crate, enclosing symbol, kind, display
  name, language, documentation, reference count, signature, parameters, return
  type, and definition span.
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
- Function briefs include outgoing and incoming SCIP-derived function-like
  references.
- Function briefs include parameter SCIP IDs and return-type SCIP IDs when
  available.
- Missing return-type SCIP IDs are labeled as unknown when only text is
  available.
- Missing element lookup returns `ReportError::ElementNotFound` and the stable
  message from `ReportError::message()`.
- Stable-ID lookup and stable-ID reference targets are matched the same way as
  raw SCIP symbols.
- Reference limits cap returned evidence and per-element reference-derived
  outputs, and `include_inferred = false` leaves both the element-level inferred
  list and 5W `why` empty.
- Test references from SCIP role flags, `tests/` paths, and Rust test-file
  suffixes appear in the `how` section.
- Recency is unknown when git metadata is absent.
- LLM briefs include confirmed, inferred, and unknown sections.
- Token budget removes lower-signal sections first.

Use hand-built `SemanticModel` fixtures. Do not require SCIP parsing or MCP
setup in this crate's tests.

## Concrete Implementation Snippets

Use these snippets as the concrete starting implementation for report
derivation. Keep the one-named-type-per-file rule when copying error/result
definitions into their target files.

```rust
// crates/refactor-radar-report/src/lib.rs
pub mod build_element_brief;
pub mod build_project_summary;
pub mod generate_llm_brief;
pub mod report_error;
pub mod report_result;

pub use build_element_brief::build_element_brief;
pub use build_project_summary::build_project_summary;
pub use generate_llm_brief::generate_llm_brief;
pub use report_error::ReportError;
pub use report_result::ReportResult;
```

```rust
// crates/refactor-radar-report/src/report_error.rs
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("{message} at {location}")]
    ElementNotFound {
        message: &'static str,
        symbol_id: String,
        location: ErrorLocation,
    },
}

impl ReportError {
    #[track_caller]
    pub fn element_not_found(symbol_id: impl Into<String>) -> Self {
        Self::ElementNotFound {
            message: "element was not found in the projected semantic model",
            symbol_id: symbol_id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::ElementNotFound { message, .. } => message,
        }
    }
}
```

```rust
// crates/refactor-radar-report/src/report_result.rs
use crate::ReportError;

pub type ReportResult<T> = std::result::Result<T, ReportError>;
```

```rust
// crates/refactor-radar-report/src/build_project_summary.rs
use refactor_radar_core::{ProjectReportSummary, SemanticModel};
use std::collections::BTreeMap;

pub fn build_project_summary(model: &SemanticModel) -> ProjectReportSummary {
    let mut reference_file_counts = BTreeMap::new();
    for reference in &model.references {
        let count = reference_file_counts
            .entry(reference.document_path.clone())
            .or_insert(0_usize);
        *count += 1;
    }
    let mut hotspot_files: Vec<_> = reference_file_counts.into_iter().collect();
    hotspot_files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ProjectReportSummary {
        project_id: model.project.project_id.clone(),
        document_count: model.files.len(),
        element_count: model.elements.len(),
        reference_count: model.references.len(),
        hotspot_files: hotspot_files
            .into_iter()
            .map(|(path, count)| format!("{path} ({count})"))
            .collect(),
    }
}
```

```rust
// crates/refactor-radar-report/src/build_element_brief.rs
use crate::{ReportError, ReportResult};
use refactor_radar_core::{
    ElementBrief, ElementSummary, EvidenceRecord, FiveWSummary, SemanticModel,
    SymbolReferenceSummary,
};
use std::collections::BTreeMap;

const SCIP_SYMBOL_ROLE_TEST: i32 = 0x20;

pub fn build_element_brief(
    model: &SemanticModel,
    symbol_id: &str,
    include_inferred: bool,
    reference_limit: usize,
) -> ReportResult<ElementBrief> {
    let element = model
        .element_by_id(symbol_id)
        .ok_or_else(|| ReportError::element_not_found(symbol_id))?
        .clone();
    let references: Vec<_> = model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, &element))
        .take(reference_limit)
        .cloned()
        .collect();
    let all_references: Vec<_> = model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, &element))
        .collect();
    let mut reference_file_counts = BTreeMap::new();
    for reference in &all_references {
        let count = reference_file_counts
            .entry(reference.document_path.clone())
            .or_insert(0_usize);
        *count += 1;
    }
    let mut top_reference_files: Vec<_> = reference_file_counts.into_iter().collect();
    top_reference_files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let child_symbols: Vec<_> = model
        .elements
        .iter()
        .filter(|child| child.enclosing_symbol.as_deref() == Some(element.scip_symbol.as_str()))
        .collect();
    let test_references: Vec<_> = all_references
        .iter()
        .copied()
        .filter(|reference| is_test_reference(reference))
        .take(reference_limit)
        .collect();
    let outgoing_call_edges = model.outgoing_call_edges(element.symbol_id.as_str());
    let incoming_call_edges = model.incoming_call_edges(element.symbol_id.as_str());
    let mut confirmed = vec![
        "element came from SCIP SymbolInformation".to_owned(),
        format!("kind and display name: {:?} {}", element.kind, element.display_name),
    ];
    let mut unknown = vec![
        "git recency is not available from SCIP".to_owned(),
        "SCIP occurrence alone does not prove runtime call behavior".to_owned(),
        "intended public API status is unknown unless facts support it".to_owned(),
    ];
    let mut what_summary = vec![format!("{:?} {}", element.kind, element.display_name)];
    if let Some(signature) = &element.signature {
        confirmed.extend(signature.confirmed.iter().cloned());
        unknown.extend(signature.unknown.iter().cloned());
        if let Some(signature_text) = &signature.signature_text {
            confirmed.push(format!("signature: {signature_text}"));
            what_summary.push(format!("signature: {signature_text}"));
        }
        if !signature.parameters.is_empty() {
            let parameter_names = signature
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            confirmed.push(format!("parameters: {parameter_names}"));
            what_summary.push(format!("parameters: {parameter_names}"));
            let parameter_symbols = signature
                .parameters
                .iter()
                .filter_map(|parameter| parameter.scip_symbol.as_deref())
                .collect::<Vec<_>>();
            if !parameter_symbols.is_empty() {
                confirmed.push(format!(
                    "parameter SCIP IDs: {}",
                    parameter_symbols.join(", ")
                ));
                what_summary.push(format!(
                    "parameter SCIP IDs: {}",
                    parameter_symbols.join(", ")
                ));
            }
        }
        if let Some(return_type) = &signature.return_type {
            confirmed.push(format!("return type: {}", return_type.display_text));
            what_summary.push(format!("returns: {}", return_type.display_text));
            if let Some(return_symbol) = &return_type.scip_symbol {
                confirmed.push(format!("return type SCIP ID: {return_symbol}"));
                what_summary.push(format!("return type SCIP ID: {return_symbol}"));
            } else {
                unknown.push(
                    "return type SCIP symbol is unavailable; only return type text is confirmed"
                        .to_owned(),
                );
            }
        }
    }
    if !child_symbols.is_empty() {
        let child_names = child_symbols
            .iter()
            .map(|child| child.display_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        confirmed.push(format!("child symbols: {child_names}"));
        what_summary.push(format!("child symbols: {child_names}"));
    }
    let mut where_summary = Vec::new();
    if let Some(span) = &element.definition_span {
        let definition = format!(
            "defined at {}:{}:{}",
            span.document_path, span.start_line, span.start_column
        );
        confirmed.push(definition.clone());
        where_summary.push(definition);
    }
    if !top_reference_files.is_empty() && reference_limit > 0 {
        let top_files = top_reference_files
            .iter()
            .take(reference_limit)
            .map(|(path, count)| format!("{path} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        confirmed.push(format!("top reference files: {top_files}"));
        where_summary.push(format!("top reference files: {top_files}"));
    }
    let inferred = if include_inferred && all_references.len() > 3 {
        vec!["many SCIP references may indicate API relevance".to_owned()]
    } else {
        Vec::new()
    };
    let mut who = Vec::new();
    if let Some(package) = &element.package {
        confirmed.push(format!("package/crate: {package}"));
        who.push(package.clone());
    }
    if let Some(enclosing_symbol) = &element.enclosing_symbol {
        confirmed.push(format!("enclosing symbol: {enclosing_symbol}"));
        who.push(enclosing_symbol.clone());
    }
    let outgoing_call_summaries = outgoing_call_edges
        .iter()
        .take(reference_limit)
        .map(|edge| {
            format!(
                "SCIP-derived outgoing function_like_reference to {} at {}:{}:{}",
                edge.referenced_symbol_id.as_str(),
                edge.evidence_span.document_path,
                edge.evidence_span.start_line,
                edge.evidence_span.start_column
            )
        })
        .collect::<Vec<_>>();
    let incoming_call_summaries = incoming_call_edges
        .iter()
        .take(reference_limit)
        .map(|edge| {
            format!(
                "SCIP-derived incoming function_like_reference from {} at {}:{}:{}",
                edge.enclosing_symbol_id.as_str(),
                edge.evidence_span.document_path,
                edge.evidence_span.start_line,
                edge.evidence_span.start_column
            )
        })
        .collect::<Vec<_>>();
    confirmed.extend(outgoing_call_summaries.iter().cloned());
    confirmed.extend(incoming_call_summaries.iter().cloned());
    Ok(ElementBrief {
        element: element.clone(),
        five_w: FiveWSummary {
            who,
            what: what_summary,
            where_: where_summary,
            when: vec!["unknown: not available from SCIP".to_owned()],
            why: inferred.clone(),
            how: child_symbols
                .iter()
                .map(|child| format!("child {:?} {}", child.kind, child.display_name))
                .chain(test_references.iter().map(|reference| {
                    format!(
                        "test reference at {}:{}:{}",
                        reference.source_span.document_path,
                        reference.source_span.start_line,
                        reference.source_span.start_column
                    )
                }))
                .chain(outgoing_call_summaries.iter().cloned())
                .chain(incoming_call_summaries.iter().cloned())
                .collect(),
        },
        evidence: references
            .iter()
            .map(|reference| EvidenceRecord {
                id: format!(
                    "evidence:{}:{}:{}",
                    reference.source_span.document_path,
                    reference.source_span.start_line,
                    reference.source_span.start_column
                ),
                kind: "SCIP occurrence".to_owned(),
                confidence_label: "confirmed".to_owned(),
                source_spans: vec![reference.source_span.clone()],
                confirmed: vec!["reference came from a SCIP occurrence".to_owned()],
                inferred: Vec::new(),
                unknown: vec!["SCIP occurrence alone does not prove runtime call behavior".to_owned()],
            })
            .collect(),
        confirmed,
        inferred,
        unknown,
    })
}

fn reference_matches_element(reference: &SymbolReferenceSummary, element: &ElementSummary) -> bool {
    let referenced_symbol_id = reference.referenced_symbol_id.as_str();
    referenced_symbol_id == element.symbol_id.as_str()
        || referenced_symbol_id == element.stable_id.as_str()
        || referenced_symbol_id == element.scip_symbol.as_str()
}

fn is_test_reference(reference: &SymbolReferenceSummary) -> bool {
    reference.symbol_roles & SCIP_SYMBOL_ROLE_TEST != 0
        || reference.document_path.starts_with("tests/")
        || reference.document_path.contains("/tests/")
        || reference.document_path.ends_with("_test.rs")
        || reference.document_path.ends_with("_tests.rs")
}
```

```rust
// crates/refactor-radar-report/src/generate_llm_brief.rs
use crate::{build_element_brief, build_project_summary, ReportResult};
use refactor_radar_core::{ElementSummary, SemanticModel, SymbolReferenceSummary};

pub fn generate_llm_brief(model: &SemanticModel, budget_tokens: Option<usize>) -> ReportResult<String> {
    let project_summary = build_project_summary(model);
    let max_chars = budget_tokens.map(|tokens| tokens.saturating_mul(4));
    let mut output = format!(
        "# Project {}\n\nDocuments: {}\nElements: {}\nReferences: {}\nHotspots: {}\n",
        project_summary.project_id,
        project_summary.document_count,
        project_summary.element_count,
        project_summary.reference_count,
        project_summary.hotspot_files.join(", ")
    );
    let mut ranked_elements: Vec<_> = model
        .elements
        .iter()
        .map(|element| (element, matched_reference_count(model, element)))
        .collect();
    ranked_elements.sort_by(|(left, left_count), (right, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.symbol_id.as_str().cmp(right.symbol_id.as_str()))
    });
    for (element, _) in ranked_elements.into_iter().take(25) {
        let brief = build_element_brief(model, element.symbol_id.as_str(), true, 5)?;
        let section = format!(
            "\n## {}\nConfirmed: {}\nInferred: {}\nUnknown: {}\n",
            brief.element.display_name,
            brief.confirmed.join("; "),
            brief.inferred.join("; "),
            brief.unknown.join("; ")
        );
        if !append_budgeted_section(&mut output, &section, max_chars) {
            break;
        }
    }
    Ok(output)
}

fn matched_reference_count(model: &SemanticModel, element: &ElementSummary) -> usize {
    model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, element))
        .count()
}

fn reference_matches_element(reference: &SymbolReferenceSummary, element: &ElementSummary) -> bool {
    let referenced_symbol_id = reference.referenced_symbol_id.as_str();
    referenced_symbol_id == element.symbol_id.as_str()
        || referenced_symbol_id == element.stable_id.as_str()
        || referenced_symbol_id == element.scip_symbol.as_str()
}

fn append_budgeted_section(output: &mut String, section: &str, max_chars: Option<usize>) -> bool {
    if let Some(max_chars) = max_chars {
        if output.len().saturating_add(section.len()) > max_chars {
            return false;
        }
    }
    output.push_str(section);
    true
}
```

Concrete starting test file:

```rust
// crates/refactor-radar-report/tests/brief_tests.rs
use refactor_radar_core::{
    CallEdgeSummary, ElementKind, ElementSummary, FunctionParameterSummary,
    FunctionSignatureSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary, TypeReferenceSummary,
};
use refactor_radar_report::{
    build_element_brief, build_project_summary, generate_llm_brief, ReportError,
};

const ENUM_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Status#";
const READY_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Status#Ready.";
const FUNCTION_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/parse_record().";
const HELPER_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
const PARAM_SYMBOL: &str = "local 1";
const RETURN_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Record#";

#[test]
fn given_project_model_when_building_summary_then_counts_and_hotspots_are_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let summary = build_project_summary(&model);

    assert_eq!("fixture", summary.project_id);
    assert_eq!(1, summary.document_count);
    assert_eq!(4, summary.element_count);
    assert_eq!(1, summary.reference_count);
    assert_eq!(vec!["src/lib.rs (1)"], summary.hotspot_files);
    Ok(())
}

#[test]
fn given_enum_element_when_building_brief_then_variants_and_reference_hotspots_are_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = build_element_brief(&model, ENUM_SYMBOL, true, 10)?;

    assert!(brief.five_w.what.iter().any(|entry| entry.contains("Ready")));
    assert!(brief
        .five_w
        .where_
        .iter()
        .any(|entry| entry.contains("src/lib.rs (1)")));
    assert!(!brief
        .unknown
        .iter()
        .any(|entry| entry.contains("return type SCIP symbol")));
    Ok(())
}

#[test]
fn given_stable_id_reference_when_building_brief_then_lookup_and_reference_matching_work(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    let stable_id = StableId::from_scip_symbol(FUNCTION_SYMBOL);
    let stable_span = SourceSpan::from_scip_range("fixture", "src/stable.rs", &[3, 4, 15])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(stable_id.as_str()),
        source_span: stable_span,
        document_path: "src/stable.rs".to_owned(),
        symbol_roles: 0,
    });

    let brief = build_element_brief(&model, stable_id.as_str(), true, 10)?;

    assert!(brief.evidence.iter().any(|evidence| {
        evidence
            .source_spans
            .iter()
            .any(|span| span.document_path == "src/stable.rs")
    }));
    assert!(brief
        .five_w
        .where_
        .iter()
        .any(|entry| entry.contains("src/stable.rs (1)")));
    Ok(())
}

#[test]
fn given_missing_element_when_building_brief_then_typed_error_and_message_are_returned(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let result = build_element_brief(&model, "missing-symbol", true, 10);

    assert!(result.is_err());
    let Some(error) = result.err() else {
        return Ok(());
    };
    assert_eq!(
        "element was not found in the projected semantic model",
        error.message()
    );
    match error {
        ReportError::ElementNotFound { symbol_id, .. } => {
            assert_eq!("missing-symbol", symbol_id.as_str());
        }
    }
    Ok(())
}

#[test]
fn given_reference_limit_and_disabled_inference_when_building_brief_then_outputs_are_capped(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    for (path, line) in [
        ("src/a.rs", 10),
        ("src/a.rs", 11),
        ("src/b.rs", 12),
        ("src/c.rs", 13),
    ] {
        let source_span = SourceSpan::from_scip_range("fixture", path, &[line, 0, 1])?;
        model.references.push(SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            source_span,
            document_path: path.to_owned(),
            symbol_roles: 0,
        });
    }

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, false, 1)?;

    assert_eq!(1, brief.evidence.len());
    assert!(brief.inferred.is_empty());
    assert!(brief.five_w.why.is_empty());
    assert!(brief
        .five_w
        .where_
        .iter()
        .any(|entry| entry == "top reference files: src/a.rs (2)"));
    assert!(!brief
        .five_w
        .where_
        .iter()
        .any(|entry| entry.contains("src/b.rs")));
    assert!(!brief
        .five_w
        .where_
        .iter()
        .any(|entry| entry.contains("src/c.rs")));
    Ok(())
}

#[test]
fn given_function_element_when_building_brief_then_signature_and_function_like_references_are_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;
    let helper_brief = build_element_brief(&model, HELPER_SYMBOL, true, 10)?;

    assert!(brief
        .five_w
        .what
        .iter()
        .any(|entry| entry.contains("signature: fn parse_record(input: &str) -> Result<Record, FixtureError>")));
    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("SCIP-derived outgoing function_like_reference")));
    assert!(brief
        .five_w
        .when
        .iter()
        .any(|entry| entry.contains("not available from SCIP")));
    assert!(brief
        .confirmed
        .iter()
        .any(|entry| entry.contains("signature text came from SCIP")));
    assert!(brief
        .confirmed
        .iter()
        .any(|entry| entry.contains("SCIP-derived outgoing function_like_reference")));
    assert!(helper_brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("SCIP-derived incoming function_like_reference")));
    assert!(helper_brief
        .confirmed
        .iter()
        .any(|entry| entry.contains("SCIP-derived incoming function_like_reference")));
    assert!(brief
        .unknown
        .iter()
        .any(|entry| entry.contains("SCIP occurrence alone does not prove runtime call behavior")));
    assert!(brief
        .unknown
        .iter()
        .any(|entry| entry.contains("intended public API status is unknown")));
    assert!(brief
        .unknown
        .iter()
        .any(|entry| entry.contains("return type SCIP symbol is unavailable")));
    assert!(brief.unknown.iter().any(|entry| entry.contains("git recency")));
    Ok(())
}

#[test]
fn given_test_reference_when_building_brief_then_how_labels_test_reference(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    let test_span = SourceSpan::from_scip_range("fixture", "tests/parse_record.rs", &[1, 2, 14])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: test_span,
        document_path: "tests/parse_record.rs".to_owned(),
        symbol_roles: 0,
    });
    let role_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[2, 3, 9])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: role_span,
        document_path: "src/lib.rs".to_owned(),
        symbol_roles: 0x20,
    });
    let singular_suffix_span =
        SourceSpan::from_scip_range("fixture", "src/parse_record_test.rs", &[3, 4, 10])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: singular_suffix_span,
        document_path: "src/parse_record_test.rs".to_owned(),
        symbol_roles: 0,
    });
    let plural_suffix_span =
        SourceSpan::from_scip_range("fixture", "src/parse_record_tests.rs", &[4, 5, 11])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: plural_suffix_span,
        document_path: "src/parse_record_tests.rs".to_owned(),
        symbol_roles: 0,
    });

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("test reference at tests/parse_record.rs")));
    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("test reference at src/lib.rs")));
    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("test reference at src/parse_record_test.rs")));
    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("test reference at src/parse_record_tests.rs")));
    Ok(())
}

#[test]
fn given_function_signature_with_scip_ids_when_building_brief_then_ids_are_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    if let Some(element) = model
        .elements
        .iter_mut()
        .find(|element| element.scip_symbol == FUNCTION_SYMBOL)
    {
        element.signature = Some(FunctionSignatureSummary {
            signature_text: Some("fn parse_record(input: &str) -> Record".to_owned()),
            parameters: vec![FunctionParameterSummary {
                name: "input".to_owned(),
                scip_symbol: Some(PARAM_SYMBOL.to_owned()),
                parameter_kind: "Parameter".to_owned(),
                source_span: None,
                type_reference: Some(TypeReferenceSummary {
                    display_text: "&str".to_owned(),
                    scip_symbol: None,
                    source_span: None,
                    signature_range: None,
                    confidence: "signature_text_only".to_owned(),
                }),
            }],
            return_type: Some(TypeReferenceSummary {
                display_text: "Record".to_owned(),
                scip_symbol: Some(RETURN_SYMBOL.to_owned()),
                source_span: None,
                signature_range: None,
                confidence: "scip_symbol".to_owned(),
            }),
            confirmed: vec!["signature text came from SCIP signature_documentation".to_owned()],
            unknown: Vec::new(),
        });
    }

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(brief
        .five_w
        .what
        .iter()
        .any(|entry| entry.contains(PARAM_SYMBOL)));
    assert!(brief
        .five_w
        .what
        .iter()
        .any(|entry| entry.contains(RETURN_SYMBOL)));
    assert!(!brief
        .unknown
        .iter()
        .any(|entry| entry.contains("return type SCIP symbol is unavailable")));
    Ok(())
}

#[test]
fn given_llm_brief_when_generated_then_confirmed_inferred_and_unknown_sections_are_rendered(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = generate_llm_brief(&model, None)?;

    assert!(brief.contains("Confirmed:"));
    assert!(brief.contains("Inferred:"));
    assert!(brief.contains("Unknown:"));
    Ok(())
}

#[test]
fn given_token_budget_when_generating_llm_brief_then_output_stays_within_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    for column in [1, 2, 3, 4] {
        let source_span =
            SourceSpan::from_scip_range("fixture", "src/lib.rs", &[9, column, column + 1])?;
        model.references.push(SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            source_span,
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 0,
        });
    }

    let brief = generate_llm_brief(&model, Some(250))?;

    assert!(brief.len() <= 1000);
    assert!(brief.contains("# Project fixture"));
    assert!(brief.contains("## parse_record"));
    assert!(!brief.contains("## Status"));
    assert!(!brief.contains("## Ready"));
    Ok(())
}

fn sample_model() -> Result<SemanticModel, Box<dyn std::error::Error>> {
    let enum_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 0, 6])?;
    let function_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[4, 0, 12])?;
    let reference_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[8, 10, 16])?;

    Ok(SemanticModel {
        project: ProjectSummary {
            project_id: "fixture".to_owned(),
            project_root: "fixture".to_owned(),
            producer_name: Some("rust-analyzer".to_owned()),
            producer_version: Some("test".to_owned()),
        },
        files: vec![refactor_radar_core::FileSummary {
            document_path: "src/lib.rs".to_owned(),
            language: "rust".to_owned(),
            symbol_count: 4,
            occurrence_count: 4,
        }],
        elements: vec![
            ElementSummary {
                stable_id: StableId::from_scip_symbol(ENUM_SYMBOL),
                symbol_id: SymbolId::new(ENUM_SYMBOL),
                scip_symbol: ENUM_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Enum,
                display_name: "Status".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: Some(enum_span.clone()),
                signature: None,
                documentation: Vec::new(),
                reference_count: 1,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(READY_SYMBOL),
                symbol_id: SymbolId::new(READY_SYMBOL),
                scip_symbol: READY_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::EnumMember,
                display_name: "Ready".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: Some(ENUM_SYMBOL.to_owned()),
                definition_span: None,
                signature: None,
                documentation: Vec::new(),
                reference_count: 0,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(FUNCTION_SYMBOL),
                symbol_id: SymbolId::new(FUNCTION_SYMBOL),
                scip_symbol: FUNCTION_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Function,
                display_name: "parse_record".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: Some(function_span.clone()),
                signature: Some(FunctionSignatureSummary {
                    signature_text: Some("fn parse_record(input: &str) -> Result<Record, FixtureError>".to_owned()),
                    parameters: Vec::new(),
                    return_type: Some(TypeReferenceSummary {
                        display_text: "Result<Record, FixtureError>".to_owned(),
                        scip_symbol: None,
                        source_span: None,
                        signature_range: None,
                        confidence: "signature_text_only".to_owned(),
                    }),
                    confirmed: vec!["signature text came from SCIP signature_documentation".to_owned()],
                    unknown: vec!["return type SCIP symbol is unavailable when SCIP emitted only text".to_owned()],
                }),
                documentation: Vec::new(),
                reference_count: 4,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(HELPER_SYMBOL),
                symbol_id: SymbolId::new(HELPER_SYMBOL),
                scip_symbol: HELPER_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Function,
                display_name: "private_offset".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: None,
                signature: None,
                documentation: Vec::new(),
                reference_count: 1,
            },
        ],
        references: vec![SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(ENUM_SYMBOL),
            source_span: reference_span.clone(),
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 0,
        }],
        call_edges: vec![CallEdgeSummary {
            enclosing_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            referenced_symbol_id: SymbolId::new(HELPER_SYMBOL),
            evidence_span: reference_span,
            confidence: "scip_reference_inside_function_enclosing_range".to_owned(),
        }],
        spans: vec![enum_span, function_span],
    })
}
```

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
