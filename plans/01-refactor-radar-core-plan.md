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

## Concrete Implementation Snippets

Use these snippets as the concrete starting implementation for the core crate.
Keep the one-named-type-per-file rule when copying combined snippets into their
target files.

```rust
// crates/refactor-radar-core/src/lib.rs
pub mod core_error;
pub mod core_result;
pub mod call_edge_summary;
pub mod element_brief;
pub mod element_kind;
pub mod element_summary;
pub mod evidence_record;
pub mod file_summary;
pub mod five_w_summary;
pub mod function_parameter_summary;
pub mod function_signature_summary;
pub mod language;
pub mod project_report_summary;
pub mod project_summary;
pub mod semantic_model;
pub mod source_span;
pub mod span_id;
pub mod stable_id;
pub mod symbol_id;
pub mod symbol_reference_summary;
pub mod type_reference_summary;

pub use core_error::CoreError;
pub use core_result::CoreResult;
pub use call_edge_summary::CallEdgeSummary;
pub use element_brief::ElementBrief;
pub use element_kind::ElementKind;
pub use element_summary::ElementSummary;
pub use evidence_record::EvidenceRecord;
pub use file_summary::FileSummary;
pub use five_w_summary::FiveWSummary;
pub use function_parameter_summary::FunctionParameterSummary;
pub use function_signature_summary::FunctionSignatureSummary;
pub use language::Language;
pub use project_report_summary::ProjectReportSummary;
pub use project_summary::ProjectSummary;
pub use semantic_model::SemanticModel;
pub use source_span::SourceSpan;
pub use span_id::SpanId;
pub use stable_id::StableId;
pub use symbol_id::SymbolId;
pub use symbol_reference_summary::SymbolReferenceSummary;
pub use type_reference_summary::TypeReferenceSummary;
```

```rust
// crates/refactor-radar-core/src/core_error.rs
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{message} at {location}")]
    InvalidScipRange {
        message: &'static str,
        range: Vec<i32>,
        location: ErrorLocation,
    },
}

impl CoreError {
    #[track_caller]
    pub fn invalid_scip_range(range: Vec<i32>) -> Self {
        Self::InvalidScipRange {
            message: "SCIP range must contain valid zero-based positions",
            range,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidScipRange { message, .. } => message,
        }
    }
}
```

```rust
// crates/refactor-radar-core/src/core_result.rs
use crate::CoreError;

pub type CoreResult<T> = std::result::Result<T, CoreError>;
```

```rust
// crates/refactor-radar-core/src/stable_id.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableId(String);

impl StableId {
    pub fn from_scip_symbol(symbol: &str) -> Self {
        Self(format!("scip:{symbol}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

```rust
// crates/refactor-radar-core/src/symbol_id.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolId(String);

impl SymbolId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

```rust
// crates/refactor-radar-core/src/span_id.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpanId(String);

impl SpanId {
    pub fn new(project_id: &str, document_path: &str, line: u32, column: u32) -> Self {
        Self(format!("{project_id}::{document_path}:{line}:{column}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

```rust
// crates/refactor-radar-core/src/source_span.rs
use crate::{CoreError, CoreResult, SpanId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub id: SpanId,
    pub project_id: String,
    pub document_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl SourceSpan {
    pub fn from_scip_range(
        project_id: impl Into<String>,
        document_path: impl Into<String>,
        range: &[i32],
    ) -> CoreResult<Self> {
        let project_id = project_id.into();
        let document_path = document_path.into();
        let positions = match range {
            [line, start, end] => (*line, *start, *line, *end),
            [start_line, start_column, end_line, end_column] => {
                (*start_line, *start_column, *end_line, *end_column)
            }
            _ => return Err(CoreError::invalid_scip_range(range.to_vec())),
        };
        validate_scip_positions(positions, range)?;
        let start_line = one_based_position(positions.0, range)?;
        let start_column = one_based_position(positions.1, range)?;
        Ok(Self {
            id: SpanId::new(&project_id, &document_path, start_line, start_column),
            project_id,
            document_path,
            start_line,
            start_column,
            end_line: one_based_position(positions.2, range)?,
            end_column: one_based_position(positions.3, range)?,
        })
    }
}

fn validate_scip_positions(positions: (i32, i32, i32, i32), range: &[i32]) -> CoreResult<()> {
    let (start_line, start_column, end_line, end_column) = positions;
    if start_line < 0
        || start_column < 0
        || end_column < 0
        || end_line < start_line
        || (end_line == start_line && end_column < start_column)
    {
        return Err(CoreError::invalid_scip_range(range.to_vec()));
    }
    Ok(())
}

fn one_based_position(position: i32, range: &[i32]) -> CoreResult<u32> {
    let position = position
        .checked_add(1)
        .ok_or_else(|| CoreError::invalid_scip_range(range.to_vec()))?;
    u32::try_from(position).map_err(|_| CoreError::invalid_scip_range(range.to_vec()))
}
```

```rust
// crates/refactor-radar-core/src/language.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Unknown(String),
}
```

```rust
// crates/refactor-radar-core/src/element_kind.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    AssociatedType,
    Constant,
    Constructor,
    Enum,
    EnumMember,
    Field,
    File,
    Function,
    Macro,
    Method,
    MethodReceiver,
    Module,
    Namespace,
    Package,
    Parameter,
    SelfParameter,
    StaticMethod,
    StaticVariable,
    Struct,
    Trait,
    TraitMethod,
    Type,
    TypeAlias,
    TypeParameter,
    Union,
    Variable,
    Unknown(String),
}
```

```rust
// crates/refactor-radar-core/src/type_reference_summary.rs
use crate::SourceSpan;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeReferenceSummary {
    pub display_text: String,
    pub scip_symbol: Option<String>,
    pub source_span: Option<SourceSpan>,
    pub signature_range: Option<Vec<u32>>,
    pub confidence: String,
}
```

```rust
// crates/refactor-radar-core/src/function_parameter_summary.rs
use crate::{SourceSpan, TypeReferenceSummary};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionParameterSummary {
    pub name: String,
    pub scip_symbol: Option<String>,
    pub parameter_kind: String,
    pub source_span: Option<SourceSpan>,
    pub type_reference: Option<TypeReferenceSummary>,
}
```

```rust
// crates/refactor-radar-core/src/function_signature_summary.rs
use crate::{FunctionParameterSummary, TypeReferenceSummary};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionSignatureSummary {
    pub signature_text: Option<String>,
    pub parameters: Vec<FunctionParameterSummary>,
    pub return_type: Option<TypeReferenceSummary>,
    pub confirmed: Vec<String>,
    pub unknown: Vec<String>,
}
```

```rust
// crates/refactor-radar-core/src/project_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub project_root: String,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
}
```

```rust
// crates/refactor-radar-core/src/file_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileSummary {
    pub document_path: String,
    pub language: String,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

```rust
// crates/refactor-radar-core/src/project_report_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectReportSummary {
    pub project_id: String,
    pub document_count: usize,
    pub element_count: usize,
    pub reference_count: usize,
    pub hotspot_files: Vec<String>,
}
```

```rust
// crates/refactor-radar-core/src/element_summary.rs
use crate::{ElementKind, FunctionSignatureSummary, Language, SourceSpan, StableId, SymbolId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ElementSummary {
    pub stable_id: StableId,
    pub symbol_id: SymbolId,
    pub scip_symbol: String,
    pub language: Language,
    pub kind: ElementKind,
    pub display_name: String,
    pub package: Option<String>,
    pub enclosing_symbol: Option<String>,
    pub definition_span: Option<SourceSpan>,
    pub signature: Option<FunctionSignatureSummary>,
    pub documentation: Vec<String>,
    pub reference_count: usize,
}
```

```rust
// crates/refactor-radar-core/src/symbol_reference_summary.rs
use crate::{SourceSpan, SymbolId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolReferenceSummary {
    pub referenced_symbol_id: SymbolId,
    pub source_span: SourceSpan,
    pub document_path: String,
    pub symbol_roles: i32,
}
```

```rust
// crates/refactor-radar-core/src/call_edge_summary.rs
use crate::{SourceSpan, SymbolId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallEdgeSummary {
    pub enclosing_symbol_id: SymbolId,
    pub referenced_symbol_id: SymbolId,
    pub evidence_span: SourceSpan,
    pub confidence: String,
}
```

```rust
// crates/refactor-radar-core/src/semantic_model.rs
use crate::{
    CallEdgeSummary, ElementSummary, FileSummary, ProjectSummary, SourceSpan,
    SymbolReferenceSummary,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticModel {
    pub project: ProjectSummary,
    pub files: Vec<FileSummary>,
    pub elements: Vec<ElementSummary>,
    pub references: Vec<SymbolReferenceSummary>,
    pub call_edges: Vec<CallEdgeSummary>,
    pub spans: Vec<SourceSpan>,
}

impl SemanticModel {
    pub fn element_by_id(&self, id: &str) -> Option<&ElementSummary> {
        self.elements.iter().find(|element| {
            element.stable_id.as_str() == id
                || element.symbol_id.as_str() == id
                || element.scip_symbol.as_str() == id
        })
    }

    pub fn span_by_id(&self, id: &str) -> Option<&SourceSpan> {
        self.spans.iter().find(|span| span.id.as_str() == id)
    }

    pub fn has_project_id(&self, id: &str) -> bool {
        self.project.project_id == id
    }

    pub fn outgoing_call_edges(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.enclosing_symbol_id.as_str() == id)
            .collect()
    }

    pub fn incoming_call_edges(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.referenced_symbol_id.as_str() == id)
            .collect()
    }
}
```

```rust
// crates/refactor-radar-core/src/five_w_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FiveWSummary {
    pub who: Vec<String>,
    pub what: Vec<String>,
    #[serde(rename = "where")]
    pub where_: Vec<String>,
    pub when: Vec<String>,
    pub why: Vec<String>,
    pub how: Vec<String>,
}
```

```rust
// crates/refactor-radar-core/src/evidence_record.rs
use crate::SourceSpan;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub kind: String,
    pub confidence_label: String,
    pub source_spans: Vec<SourceSpan>,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}
```

```rust
// crates/refactor-radar-core/src/element_brief.rs
use crate::{ElementSummary, EvidenceRecord, FiveWSummary};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ElementBrief {
    pub element: ElementSummary,
    pub five_w: FiveWSummary,
    pub evidence: Vec<EvidenceRecord>,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}
```

## Tests

Create `crates/refactor-radar-core/tests/model_tests.rs` with given/when/then
test names that verify:

- Stable ID generation is deterministic.
- Original SCIP symbol strings are preserved in `ElementSummary`.
- SCIP 3-element and 4-element source spans project to one-based line and
  column fields.
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

Concrete starting test file:

```rust
// crates/refactor-radar-core/tests/model_tests.rs
use refactor_radar_core::{
    CallEdgeSummary, CoreError, ElementBrief, ElementKind, ElementSummary, EvidenceRecord,
    FileSummary, FiveWSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary,
};
use serde_json::Value;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
const CALLEE_SYMBOL: &str =
    "rust-analyzer cargo basic_crate 0.1.0 basic_crate/internal_sum().";

#[test]
fn given_scip_symbol_when_creating_stable_id_then_generation_is_deterministic() {
    let first = StableId::from_scip_symbol(SYMBOL);
    let second = StableId::from_scip_symbol(SYMBOL);

    assert_eq!(first, second);
    assert_eq!("scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().", first.as_str());
}

#[test]
fn given_element_summary_when_created_then_original_scip_symbol_is_preserved() {
    let element = sample_element();

    assert_eq!(SYMBOL, element.scip_symbol.as_str());
    assert_eq!(SYMBOL, element.symbol_id.as_str());
}

#[test]
fn given_zero_based_scip_range_when_projecting_span_then_public_fields_are_one_based(
) -> Result<(), Box<dyn std::error::Error>> {
    let span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 2, 5])?;

    assert_eq!("fixture", span.project_id);
    assert_eq!("fixture::src/lib.rs:1:3", span.id.as_str());
    assert_eq!("src/lib.rs", span.document_path);
    assert_eq!(1, span.start_line);
    assert_eq!(3, span.start_column);
    assert_eq!(1, span.end_line);
    assert_eq!(6, span.end_column);

    let multi_line_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[1, 3, 2, 7])?;

    assert_eq!("fixture::src/lib.rs:2:4", multi_line_span.id.as_str());
    assert_eq!(2, multi_line_span.start_line);
    assert_eq!(4, multi_line_span.start_column);
    assert_eq!(3, multi_line_span.end_line);
    assert_eq!(8, multi_line_span.end_column);
    Ok(())
}

#[test]
fn given_malformed_scip_ranges_when_projecting_span_then_invalid_ranges_are_rejected(
) -> Result<(), Box<dyn std::error::Error>> {
    let zero_width = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[2, 4, 4])?;

    assert_eq!(3, zero_width.start_line);
    assert_eq!(5, zero_width.start_column);
    assert_eq!(3, zero_width.end_line);
    assert_eq!(5, zero_width.end_column);

    let invalid_ranges: &[&[i32]] = &[
        &[],
        &[0, 1],
        &[-1, 0, 1],
        &[1, 3, 0, 4],
        &[1, 4, 1, 3],
    ];

    for range in invalid_ranges {
        let result = SourceSpan::from_scip_range("fixture", "src/lib.rs", *range);

        assert!(matches!(
            &result,
            Err(CoreError::InvalidScipRange { .. })
        ));
        if let Err(error) = result {
            assert_eq!("SCIP range must contain valid zero-based positions", error.message());
        }
    }

    Ok(())
}

#[test]
fn given_element_brief_when_serializing_then_five_w_and_evidence_labels_are_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let brief = ElementBrief {
        element: sample_element(),
        five_w: FiveWSummary {
            who: vec!["basic_crate".to_owned()],
            what: vec!["Function public_sum".to_owned()],
            where_: vec!["defined at src/lib.rs:1:1".to_owned()],
            when: vec!["not available from SCIP".to_owned()],
            why: vec!["many SCIP references may indicate API relevance".to_owned()],
            how: vec!["SCIP-derived outgoing function_like_reference".to_owned()],
        },
        evidence: vec![EvidenceRecord {
            id: "evidence:src/lib.rs:1:1".to_owned(),
            kind: "SCIP occurrence".to_owned(),
            confidence_label: "confirmed".to_owned(),
            source_spans: Vec::new(),
            confirmed: vec!["reference came from a SCIP occurrence".to_owned()],
            inferred: Vec::new(),
            unknown: vec!["SCIP occurrence alone does not prove runtime call behavior".to_owned()],
        }],
        confirmed: vec!["element came from SCIP SymbolInformation".to_owned()],
        inferred: vec!["many SCIP references may indicate API relevance".to_owned()],
        unknown: vec!["git recency is not available from SCIP".to_owned()],
    };

    let value = serde_json::to_value(brief)?;
    assert!(value.get("five_w").and_then(|five_w| five_w.get("who")).is_some());
    assert!(value.get("five_w").and_then(|five_w| five_w.get("what")).is_some());
    assert!(value.get("five_w").and_then(|five_w| five_w.get("where")).is_some());
    assert!(value.get("five_w").and_then(|five_w| five_w.get("when")).is_some());
    assert!(value.get("five_w").and_then(|five_w| five_w.get("why")).is_some());
    assert!(value.get("five_w").and_then(|five_w| five_w.get("how")).is_some());
    assert!(matches!(value.get("evidence"), Some(Value::Array(_))));
    assert!(matches!(value.get("confirmed"), Some(Value::Array(_))));
    assert!(matches!(value.get("inferred"), Some(Value::Array(_))));
    assert!(matches!(value.get("unknown"), Some(Value::Array(_))));

    let evidence = value
        .get("evidence")
        .and_then(Value::as_array)
        .and_then(|records| records.first());
    assert!(evidence.and_then(|record| record.get("confirmed")).is_some());
    assert!(evidence.and_then(|record| record.get("inferred")).is_some());
    assert!(evidence.and_then(|record| record.get("unknown")).is_some());
    Ok(())
}

#[test]
fn given_semantic_model_when_serializing_then_ids_are_strings_and_helpers_resolve(
) -> Result<(), Box<dyn std::error::Error>> {
    let span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 0, 10])?;
    let model = SemanticModel {
        project: ProjectSummary {
            project_id: "fixture".to_owned(),
            project_root: "/tmp/fixture".to_owned(),
            producer_name: Some("test-producer".to_owned()),
            producer_version: Some("0.1.0".to_owned()),
        },
        files: vec![FileSummary {
            document_path: "src/lib.rs".to_owned(),
            language: "Rust".to_owned(),
            symbol_count: 1,
            occurrence_count: 1,
        }],
        elements: vec![sample_element()],
        references: vec![SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(SYMBOL),
            source_span: span.clone(),
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 1 | 8,
        }],
        call_edges: vec![CallEdgeSummary {
            enclosing_symbol_id: SymbolId::new(SYMBOL),
            referenced_symbol_id: SymbolId::new(CALLEE_SYMBOL),
            evidence_span: span.clone(),
            confidence: "inferred".to_owned(),
        }],
        spans: vec![span.clone()],
    };

    assert!(model.has_project_id("fixture"));
    assert!(model.element_by_id("scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().").is_some());
    assert!(model.element_by_id(SYMBOL).is_some());
    assert!(model.span_by_id(span.id.as_str()).is_some());
    assert_eq!(1, model.outgoing_call_edges(SYMBOL).len());
    assert_eq!(1, model.incoming_call_edges(CALLEE_SYMBOL).len());

    let value = serde_json::to_value(&model)?;
    let element = value
        .get("elements")
        .and_then(Value::as_array)
        .and_then(|elements| elements.first());
    assert_eq!(
        Some(Value::String(SYMBOL.to_owned())),
        element.and_then(|element| element.get("symbol_id")).cloned()
    );
    assert_eq!(
        Some(Value::String(
            "scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().".to_owned()
        )),
        element.and_then(|element| element.get("stable_id")).cloned()
    );

    let serialized = serde_json::to_string(&model)?;
    let round_trip: SemanticModel = serde_json::from_str(&serialized)?;

    assert_eq!(model, round_trip);
    Ok(())
}

fn sample_element() -> ElementSummary {
    ElementSummary {
        stable_id: StableId::from_scip_symbol(SYMBOL),
        symbol_id: SymbolId::new(SYMBOL),
        scip_symbol: SYMBOL.to_owned(),
        language: Language::Rust,
        kind: ElementKind::Function,
        display_name: "public_sum".to_owned(),
        package: Some("basic_crate".to_owned()),
        enclosing_symbol: None,
        definition_span: None,
        signature: None,
        documentation: Vec::new(),
        reference_count: 1,
    }
}
```

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
