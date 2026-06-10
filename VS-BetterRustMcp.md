# Plan: Better Rust MCP With SCIP-Backed 5W Summaries

## Objective

Implement a local stdio MCP server for RefactorRadar that uses
`rust-analyzer scip` as the canonical Rust semantic fact source, then derives
compact, evidence-preserving summaries that are useful to humans and LLM agents.

This replaces the tree-sitter-first scanner shape from `VS-RustMcp.md` for the
Rust v1 path. SCIP provides the Rust symbol/reference/index facts. RefactorRadar
normalizes those facts into summary artifacts and MCP responses, including
5W-style element briefs:

- Who owns or encloses this element?
- What kind of element is it and what role does it play?
- Where is it defined and where is it referenced?
- When was it produced or, later, changed?
- Why might it matter, with inference clearly labeled?
- How does it relate to callers, callees, variants, fields, tests, and hotspots?

The product value is not SCIP CRUD. The product value is MCP-accessible semantic
interrogation over SCIP-backed code intelligence.

## Hard Constraints

- Follow `AGENTS.md` for every Rust crate.
- Do not use `anyhow` for RefactorRadar application or library errors.
- Every crate must define a crate-local typed error enum and result alias.
- Error variants must carry `location: ErrorLocation`.
- Public error constructor functions must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Stable plain messages must be available through `message()` when MCP or
  report output needs text without source-location detail.
- Use exactly one named type definition per Rust source file.
- Keep `crates/wip` untouched unless a later plan explicitly removes it.
- The MCP stdio server must never write logs or diagnostics to stdout. Stdout is
  reserved for MCP JSON-RPC traffic.
- Runtime summary/query operation must be local-first and must not require
  network access.
- Missing `rust-analyzer` must be reported clearly; RefactorRadar must not
  auto-install it.

## Repo Facts To Respect

- Root `Cargo.toml` currently has one workspace member: `crates/wip`.
- Root lints deny `unwrap_used`, `expect_used`, `panic`, and unused must-use
  values.
- Root workspace dependencies already include `error-location`, `thiserror`,
  and `soul-attributes`.
- `submodules/scip/bindings/rust` provides the local `scip` crate.
- `submodules/rust-analyzer` provides local source for verifying
  `rust-analyzer scip` flags and output behavior.
- The local `submodules/scip/bindings/rust` crate is `scip` 0.8.1 and exposes
  `SymbolInformation.signature_documentation` as `Signature`. The vendored
  `submodules/rust-analyzer` source currently generates that field with the
  `scip` 0.7 Document-shaped payload. The `scip` 0.7 `Document.text` field and
  `scip` 0.8 `Signature.text` field both use protobuf field number 5, so
  parsing current rust-analyzer output through the local 0.8 binding populates
  `Signature.text`.
- The local `scip` 0.8.1 `Occurrence` type exposes `typed_range` and
  `typed_enclosing_range` oneofs. Projection must prefer those typed ranges
  when present and fall back to the deprecated `range` and `enclosing_range`
  vectors for rust-analyzer's current output.
- The vendored rust-analyzer CLI supports:
  `rust-analyzer scip <path> --output <path>`.
- SCIP positions are zero-based ranges; RefactorRadar public spans should expose
  one-based line and column fields.

## Target Architecture

```text
MCP client / coding agent
    |
    v
crates/refactor-radar-mcp
    stdio MCP tools, cached project indexes, query handlers
    |
    v
crates/refactor-radar-report
    5W element briefs, project summaries, LLM brief rendering
    |
    v
crates/refactor-radar-core
    normalized semantic model, evidence model, source spans, stable IDs
    ^
    |
crates/refactor-radar-scip
    rust-analyzer subprocess runner, SCIP read/write, SCIP-to-core projection
    ^
    |
rust-analyzer scip + submodules/scip/bindings/rust
```

Tree-sitter Rust is not part of the first implementation. Add a later syntax
supplement only for facts SCIP does not encode directly, such as exact call
expressions, branch counts, match counts, and nesting depth.

## Phase 0: Bootstrap Commands

Run these from the repository root:

```bash
git status --short
cargo metadata --no-deps --format-version=1

cargo new --lib crates/refactor-radar-core --name refactor-radar-core --vcs none
cargo new --lib crates/refactor-radar-scip --name refactor-radar-scip --vcs none
cargo new --lib crates/refactor-radar-report --name refactor-radar-report --vcs none
cargo new --lib crates/refactor-radar-mcp --name refactor-radar-mcp --vcs none

mkdir -p crates/refactor-radar-mcp/src/bin
mkdir -p crates/refactor-radar-core/tests
mkdir -p crates/refactor-radar-scip/tests
mkdir -p crates/refactor-radar-report/tests
mkdir -p testdata/rust/basic_crate/src
```

After scaffold creation, explicitly edit the root workspace manifest. Do not
rely on `cargo new` to keep the workspace member list in the desired order.

## Phase 1: Workspace Manifests

Root `Cargo.toml` target shape:

```toml
[workspace]
members = [
    "crates/wip",
    "crates/refactor-radar-core",
    "crates/refactor-radar-scip",
    "crates/refactor-radar-report",
    "crates/refactor-radar-mcp"
]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
repository = "https://github.com/TonyMarkham/refactor-radar"

[workspace.dependencies]
error-location                  = { version = "0.1.0" }
protobuf                        = { version = "=3.7.2" }
protobuf-json-mapping           = { version = "=3.7.2" }
rmcp                            = { version = "1.7.0", default-features = false, features = ["server", "macros", "schemars", "transport-io"] }
schemars                        = { version = "1.1", features = ["derive"] }
scip                            = { path = "submodules/scip/bindings/rust" }
serde                           = { version = "1.0", features = ["derive"] }
serde_json                      = { version = "1.0" }
soul-attributes                 = { version = "0.1.1" }
tempfile                        = { version = "3.23.0" }
thiserror                       = { version = "2.0.18" }
tokio                           = { version = "1", features = ["macros", "rt-multi-thread", "io-std", "io-util", "process", "sync"] }

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"

[workspace.lints.rust]
unused_must_use = "deny"
```

`crates/refactor-radar-core` owns only serializable RefactorRadar models and
depends on `serde`, `thiserror`, and `error-location`.

`crates/refactor-radar-scip` depends on `scip`, `protobuf`,
`protobuf-json-mapping`, `refactor-radar-core`, `tokio`, `thiserror`, and
`error-location`.

`crates/refactor-radar-report` depends on `refactor-radar-core`,
`thiserror`, and `error-location`.

`crates/refactor-radar-mcp` depends on `refactor-radar-core`,
`refactor-radar-scip`, `refactor-radar-report`, `rmcp`, `schemars`, `serde`,
`serde_json`, `tempfile`, `tokio`, `thiserror`, and `error-location`.

Concrete crate manifest snippets:

```toml
# crates/refactor-radar-core/Cargo.toml
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

```toml
# crates/refactor-radar-scip/Cargo.toml
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

```toml
# crates/refactor-radar-report/Cargo.toml
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

```toml
# crates/refactor-radar-mcp/Cargo.toml
[package]
name = "refactor-radar-mcp"
version.workspace = true
edition.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
error-location.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
refactor-radar-report = { path = "../refactor-radar-report" }
refactor-radar-scip = { path = "../refactor-radar-scip" }
rmcp.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
tempfile.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
protobuf.workspace = true
scip.workspace = true
```

## Phase 2: Core Model

Create one file per named type. Recommended
`crates/refactor-radar-core/src` layout:

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

Minimum model fields:

- `SemanticModel`
  - `project: ProjectSummary`
  - `files: Vec<FileSummary>`
  - `elements: Vec<ElementSummary>`
  - `references: Vec<SymbolReferenceSummary>`
  - `call_edges: Vec<CallEdgeSummary>`
  - `spans: Vec<SourceSpan>`
  - helper lookups by project ID, symbol ID, and span ID
- `ProjectReportSummary`
  - project ID
  - document count
  - element count
  - reference count
  - hotspot files derived from reference counts
- `ElementSummary`
  - RefactorRadar stable ID
  - original SCIP symbol string
  - language
  - kind
  - display name
  - package or crate name when derivable
  - enclosing symbol when present
  - definition span
  - signature summary when SCIP provides it
  - documentation strings when SCIP provides them
  - reference count
- `FunctionSignatureSummary`
  - signature text from `SymbolInformation.signature_documentation`
  - `parameters: Vec<FunctionParameterSummary>`
  - `return_type: Option<TypeReferenceSummary>`
  - `confirmed: Vec<String>`
  - `unknown: Vec<String>`
- `FunctionParameterSummary`
  - display name
  - parameter SCIP symbol when SCIP emits one
  - parameter kind, such as `Parameter` or `SelfParameter`
  - source span when the parameter occurrence is available
  - type reference when a type SCIP symbol is available
- `TypeReferenceSummary`
  - display text
  - SCIP symbol when the type is represented by a SCIP occurrence
  - source span or signature-text range when available
  - confidence, such as `scip_symbol`, `signature_text_only`, or `unknown`
- `SymbolReferenceSummary`
  - referenced symbol ID
  - source span
  - document path
  - role flags projected from SCIP `SymbolRole`
- `SourceSpan`
  - project ID
  - document path
  - stable span ID scoped by project ID, document path, and start position
  - one-based start and end line/column fields
- `CallEdgeSummary`
  - enclosing function-like symbol ID
  - referenced function-like symbol ID
  - source span of the reference occurrence
  - confidence label such as `scip_reference_inside_function_enclosing_range`
- `ElementBrief`
  - `element: ElementSummary`
  - `five_w: FiveWSummary`
  - `evidence: Vec<EvidenceRecord>`
  - `confirmed: Vec<String>`
  - `inferred: Vec<String>`
  - `unknown: Vec<String>`
- `FiveWSummary`
  - `who: Vec<String>`
  - `what_: Vec<String>` or `what_summary: Vec<String>`
  - `where_: Vec<String>` or `where_summary: Vec<String>`
  - `when_: Vec<String>` or `when_summary: Vec<String>`
  - `why_: Vec<String>` or `why_summary: Vec<String>`
  - `how: Vec<String>`

Use field names that serialize cleanly as `who`, `what`, `where`, `when`,
`why`, and `how`. If Rust keywords require different internal names, use serde
renames.

Implementation snippets for the core model:

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

// crates/refactor-radar-core/src/element_kind.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    AssociatedType,
    Attribute,
    Constant,
    Enum,
    EnumMember,
    Field,
    Function,
    Macro,
    Method,
    Module,
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
    Unknown,
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

// crates/refactor-radar-core/src/function_parameter_summary.rs
use crate::{SourceSpan, TypeReferenceSummary};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionParameterSummary {
    pub display_name: String,
    pub scip_symbol: Option<String>,
    pub parameter_kind: String,
    pub source_span: Option<SourceSpan>,
    pub type_reference: Option<TypeReferenceSummary>,
}

// crates/refactor-radar-core/src/function_signature_summary.rs
use crate::{FunctionParameterSummary, TypeReferenceSummary};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionSignatureSummary {
    pub signature_text: String,
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

// crates/refactor-radar-core/src/file_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileSummary {
    pub document_path: String,
    pub language: String,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}

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

// crates/refactor-radar-core/src/symbol_reference_summary.rs
use crate::{SourceSpan, SymbolId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolReferenceSummary {
    pub referenced_symbol_id: SymbolId,
    pub source_span: SourceSpan,
    pub document_path: String,
    pub role_flags: i32,
}

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
        self.elements
            .iter()
            .find(|element| element.stable_id.as_str() == id || element.symbol_id.as_str() == id)
    }

    pub fn span_by_id(&self, id: &str) -> Option<&SourceSpan> {
        self.spans.iter().find(|span| span.id.as_str() == id)
    }

    pub fn call_edges_for_enclosing(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.enclosing_symbol_id.as_str() == id)
            .collect()
    }

    pub fn call_edges_for_referenced(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.referenced_symbol_id.as_str() == id)
            .collect()
    }

    pub fn is_project_id(&self, id: &str) -> bool {
        self.project.project_id == id
    }
}
```

```rust
// crates/refactor-radar-core/src/five_w_summary.rs
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FiveWSummary {
    pub who: Vec<String>,
    #[serde(rename = "what")]
    pub what_summary: Vec<String>,
    #[serde(rename = "where")]
    pub where_summary: Vec<String>,
    #[serde(rename = "when")]
    pub when_summary: Vec<String>,
    #[serde(rename = "why")]
    pub why_summary: Vec<String>,
    pub how: Vec<String>,
}

// crates/refactor-radar-core/src/evidence_record.rs
use crate::SourceSpan;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub label: String,
    pub source: String,
    pub span: Option<SourceSpan>,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}

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

## Phase 3: SCIP Crate

Recommended `crates/refactor-radar-scip/src` layout:

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

Responsibilities:

- Run `rust-analyzer scip <path> --output <output_path>`.
- Accept optional `rust_analyzer_path`, `config_path`,
  `exclude_vendored_libraries`, and `num_threads`.
- Capture subprocess stderr and include it in typed errors on nonzero exit.
- Read binary `.scip` files into `scip::types::Index`.
- Read protobuf JSON `.json` files into `scip::types::Index`.
- Detect format by extension:
  - `.scip` means binary protobuf.
  - `.json` means protobuf JSON.
  - unknown extensions require explicit format.
- Project SCIP documents, symbols, occurrences, references, roles, and source
  ranges into the RefactorRadar core model.
- Preserve original SCIP symbol strings so MCP clients can ask follow-up
  questions using either RefactorRadar stable IDs or SCIP IDs.

Projection rules:

- `Index.metadata.project_root` becomes the project root URI.
- `Index.metadata.tool_info` becomes project scan producer metadata.
- Each `Document.relative_path` becomes a `FileSummary`.
- Each `Document.symbols` entry becomes an `ElementSummary`.
- Each occurrence with `Definition` role links an element to a definition span.
- Each non-definition occurrence becomes a `SymbolReferenceSummary`.
- Function, method, static method, and trait method symbols should preserve
  `SymbolInformation.signature_documentation.text` as signature text.
- Signature occurrences inside `SymbolInformation.signature_documentation` must
  be projected into parameter and return-type `TypeReferenceSummary` values
  when SCIP provides them.
- Parameter symbols emitted as `Parameter` or `SelfParameter` should be linked
  to their enclosing function when their `enclosing_symbol`, definition
  occurrence, or enclosing range provides enough evidence.
- Return-type SCIP symbols should be captured when they appear in
  `Signature.occurrences`. The vendored rust-analyzer currently emits legacy
  signature payloads with text but without signature occurrences, and its
  `Document.text` is empty, so v1 must not infer return-type SCIP IDs from
  normal document occurrences alone. If only text is available, keep the text
  and mark the type reference as `signature_text_only`.
- For `Occurrence` source ranges, prefer `typed_range` when present and fall
  back to the deprecated `range` vector.
- For `Occurrence` enclosing ranges, prefer `typed_enclosing_range` when
  present and fall back to the deprecated `enclosing_range` vector.
- SCIP range `[line, start, end]` or a `SingleLineRange` becomes a one-line
  span.
- SCIP range `[start_line, start_col, end_line, end_col]` or a
  `MultiLineRange` becomes a multi-line span.
- Public/private visibility is `unknown` in v1 unless the SCIP symbol or docs
  provide a reliable fact.
- Function call summaries are derived from function/method symbol references
  inside a function's enclosing range and must be labeled as SCIP-derived
  references, not exact AST call expressions.

Implementation snippets for the SCIP crate:

```rust
// crates/refactor-radar-scip/src/lib.rs
pub mod format;
pub mod generate_rust_scip;
pub mod project_scip_index;
pub mod scip_error;
pub mod scip_result;
pub mod scip_to_core;
pub mod symbol_kind_projection;

pub use format::Format;
pub use generate_rust_scip::generate_rust_scip;
pub use project_scip_index::project_scip_index;
pub use scip_error::ScipError;
pub use scip_result::ScipResult;
pub use scip_to_core::scip_to_core;
```

```rust
// crates/refactor-radar-scip/src/scip_error.rs
use error_location::ErrorLocation;
use std::path::PathBuf;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScipError {
    #[error("{message} at {location}")]
    UnknownFormat {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerMissing {
        message: &'static str,
        executable: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerLaunchFailed {
        message: &'static str,
        executable: PathBuf,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    RustAnalyzerFailed {
        message: &'static str,
        status: Option<i32>,
        stderr: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ReadFailed {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ParseFailed {
        message: &'static str,
        path: PathBuf,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    CoreProjectionFailed {
        message: &'static str,
        location: ErrorLocation,
    },
}

impl ScipError {
    #[track_caller]
    pub fn unknown_format(path: PathBuf) -> Self {
        Self::UnknownFormat {
            message: "SCIP input format could not be detected from extension",
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_missing(executable: PathBuf) -> Self {
        Self::RustAnalyzerMissing {
            message: "rust-analyzer executable was not found",
            executable,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_launch_failed(executable: PathBuf, details: impl Into<String>) -> Self {
        Self::RustAnalyzerLaunchFailed {
            message: "rust-analyzer executable could not be launched",
            executable,
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn rust_analyzer_failed(status: Option<i32>, stderr: String) -> Self {
        Self::RustAnalyzerFailed {
            message: "rust-analyzer scip exited unsuccessfully",
            status,
            stderr,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn read_failed(path: PathBuf) -> Self {
        Self::ReadFailed {
            message: "failed to read SCIP input file",
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn parse_failed(path: PathBuf, message: &'static str) -> Self {
        Self::ParseFailed {
            message,
            path,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn core_projection_failed() -> Self {
        Self::CoreProjectionFailed {
            message: "failed to project SCIP source range",
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::UnknownFormat { message, .. }
            | Self::RustAnalyzerMissing { message, .. }
            | Self::RustAnalyzerLaunchFailed { message, .. }
            | Self::RustAnalyzerFailed { message, .. }
            | Self::ReadFailed { message, .. }
            | Self::ParseFailed { message, .. }
            | Self::CoreProjectionFailed { message, .. } => message,
        }
    }
}

// crates/refactor-radar-scip/src/scip_result.rs
use crate::ScipError;

pub type ScipResult<T> = std::result::Result<T, ScipError>;
```

```rust
// crates/refactor-radar-scip/src/format.rs
use crate::{ScipError, ScipResult};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Binary,
    Json,
}

impl Format {
    pub fn detect(path: &Path) -> ScipResult<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("scip") => Ok(Self::Binary),
            Some("json") => Ok(Self::Json),
            _ => Err(ScipError::unknown_format(PathBuf::from(path))),
        }
    }
}
```

```rust
// crates/refactor-radar-scip/src/generate_rust_scip.rs
use crate::{ScipError, ScipResult};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub async fn generate_rust_scip(
    project_path: &Path,
    output_path: &Path,
    rust_analyzer_path: Option<&Path>,
    config_path: Option<&Path>,
    exclude_vendored_libraries: bool,
    num_threads: Option<usize>,
) -> ScipResult<String> {
    let executable = rust_analyzer_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rust-analyzer"));
    let mut command = Command::new(&executable);
    command.arg("scip").arg(project_path).arg("--output").arg(output_path);
    if let Some(config_path) = config_path {
        command.arg("--config-path").arg(config_path);
    }
    if exclude_vendored_libraries {
        command.arg("--exclude-vendored-libraries");
    }
    if let Some(num_threads) = num_threads {
        command.arg("--num-threads").arg(num_threads.to_string());
    }
    let output = command
        .output()
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => ScipError::rust_analyzer_missing(executable.clone()),
            _ => ScipError::rust_analyzer_launch_failed(executable.clone(), error.to_string()),
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(ScipError::rust_analyzer_failed(output.status.code(), stderr));
    }
    Ok(stderr)
}
```

```rust
// crates/refactor-radar-scip/src/project_scip_index.rs
use crate::{scip_to_core, Format, ScipError, ScipResult};
use protobuf::Message;
use refactor_radar_core::SemanticModel;
use scip::types::Index;
use std::path::{Path, PathBuf};

pub fn project_scip_index(path: &Path, format: Option<Format>) -> ScipResult<SemanticModel> {
    let format = match format {
        Some(format) => format,
        None => Format::detect(path)?,
    };
    let path_buf = PathBuf::from(path);
    let bytes = std::fs::read(path).map_err(|_| ScipError::read_failed(path_buf.clone()))?;
    let index = match format {
        Format::Binary => Index::parse_from_bytes(&bytes).map_err(|_| {
            ScipError::parse_failed(path_buf.clone(), "failed to parse binary SCIP protobuf")
        })?,
        Format::Json => {
            let json = String::from_utf8_lossy(&bytes);
            protobuf_json_mapping::parse_from_str::<Index>(json.as_ref()).map_err(|_| {
                ScipError::parse_failed(path_buf.clone(), "failed to parse SCIP protobuf JSON")
            })?
        }
    };
    scip_to_core(&index)
}
```

```rust
// crates/refactor-radar-scip/src/symbol_kind_projection.rs
use refactor_radar_core::ElementKind;
use scip::types::symbol_information::Kind as ScipKind;

pub fn project_symbol_kind(kind: ScipKind) -> ElementKind {
    match kind {
        ScipKind::AssociatedType => ElementKind::AssociatedType,
        ScipKind::Attribute => ElementKind::Attribute,
        ScipKind::Constant => ElementKind::Constant,
        ScipKind::Enum => ElementKind::Enum,
        ScipKind::EnumMember => ElementKind::EnumMember,
        ScipKind::Field => ElementKind::Field,
        ScipKind::Function => ElementKind::Function,
        ScipKind::Macro => ElementKind::Macro,
        ScipKind::Method => ElementKind::Method,
        ScipKind::Module => ElementKind::Module,
        ScipKind::Parameter => ElementKind::Parameter,
        ScipKind::SelfParameter => ElementKind::SelfParameter,
        ScipKind::StaticMethod => ElementKind::StaticMethod,
        ScipKind::StaticVariable => ElementKind::StaticVariable,
        ScipKind::Struct => ElementKind::Struct,
        ScipKind::Trait => ElementKind::Trait,
        ScipKind::TraitMethod => ElementKind::TraitMethod,
        ScipKind::Type => ElementKind::Type,
        ScipKind::TypeAlias => ElementKind::TypeAlias,
        ScipKind::TypeParameter => ElementKind::TypeParameter,
        ScipKind::Union => ElementKind::Union,
        ScipKind::Variable => ElementKind::Variable,
        _ => ElementKind::Unknown,
    }
}
```

```rust
// crates/refactor-radar-scip/src/scip_to_core.rs
use crate::{symbol_kind_projection::project_symbol_kind, ScipError, ScipResult};
use refactor_radar_core::{
    CallEdgeSummary, ElementKind, ElementSummary, FileSummary, FunctionParameterSummary,
    FunctionSignatureSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary, TypeReferenceSummary,
};
use scip::{
    symbol::parse_symbol,
    types::{occurrence, Document, Index, Occurrence, Signature, SymbolInformation, SymbolRole},
};

pub fn scip_to_core(index: &Index) -> ScipResult<SemanticModel> {
    let metadata = index.metadata.as_ref();
    let project_root = metadata
        .map(|metadata| metadata.project_root.clone())
        .unwrap_or_default();
    let project_id = project_root.clone();
    let tool_info = metadata.and_then(|metadata| metadata.tool_info.as_ref());
    let mut files = Vec::new();
    let mut elements = Vec::new();
    let mut references = Vec::new();
    let mut call_edges = Vec::new();
    let mut spans = Vec::new();

    for document in &index.documents {
        files.push(FileSummary {
            document_path: document.relative_path.clone(),
            language: document.language.clone(),
            symbol_count: document.symbols.len(),
            occurrence_count: document.occurrences.len(),
        });

        for symbol in &document.symbols {
            let definition_span = document
                .occurrences
                .iter()
                .find(|occurrence| {
                    occurrence.symbol == symbol.symbol
                        && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                })
                .map(|occurrence| {
                    let range = occurrence_range(occurrence)
                        .ok_or_else(ScipError::core_projection_failed)?;
                    SourceSpan::from_scip_range(&project_id, &document.relative_path, &range)
                        .map_err(|_| ScipError::core_projection_failed())
                })
                .transpose()?;
            if let Some(span) = &definition_span {
                spans.push(span.clone());
            }
            let reference_count = reference_count(index, &symbol.symbol);
            let signature = symbol
                .signature_documentation
                .as_ref()
                .map(|signature| {
                    project_signature(
                        signature,
                        &symbol.symbol,
                        &document.symbols,
                        &document.occurrences,
                        &project_id,
                        &document.relative_path,
                    )
                })
                .transpose()?;
            elements.push(ElementSummary {
                stable_id: StableId::from_scip_symbol(&symbol.symbol),
                symbol_id: SymbolId::new(symbol.symbol.clone()),
                scip_symbol: symbol.symbol.clone(),
                language: Language::Rust,
                kind: symbol
                    .kind
                    .enum_value()
                    .ok()
                    .map(project_symbol_kind)
                    .unwrap_or(ElementKind::Unknown),
                display_name: symbol.display_name.clone(),
                package: package_name(&symbol.symbol),
                enclosing_symbol: (!symbol.enclosing_symbol.is_empty())
                    .then(|| symbol.enclosing_symbol.clone()),
                definition_span,
                signature,
                documentation: symbol.documentation.clone(),
                reference_count,
            });
        }

        for occurrence in &document.occurrences {
            if occurrence.symbol.is_empty()
                || occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
            {
                continue;
            }
            let range = occurrence_range(occurrence)
                .ok_or_else(ScipError::core_projection_failed)?;
            let source_span = SourceSpan::from_scip_range(
                &project_id,
                &document.relative_path,
                &range,
            )
                .map_err(|_| ScipError::core_projection_failed())?;
            spans.push(source_span.clone());
            references.push(SymbolReferenceSummary {
                referenced_symbol_id: SymbolId::new(occurrence.symbol.clone()),
                source_span,
                document_path: document.relative_path.clone(),
                role_flags: occurrence.symbol_roles,
            });
        }
        call_edges.extend(project_call_edges(index, document, &project_id)?);
    }

    Ok(SemanticModel {
        project: ProjectSummary {
            project_id: project_id.clone(),
            project_root,
            producer_name: tool_info.map(|tool_info| tool_info.name.clone()),
            producer_version: tool_info.map(|tool_info| tool_info.version.clone()),
        },
        files,
        elements,
        references,
        call_edges,
        spans,
    })
}

fn project_call_edges(
    index: &Index,
    document: &Document,
    project_id: &str,
) -> ScipResult<Vec<CallEdgeSummary>> {
    let mut call_edges = Vec::new();
    for symbol in document.symbols.iter().filter(|symbol| is_function_like_symbol(symbol)) {
        let Some(definition) = document.occurrences.iter().find(|occurrence| {
            occurrence.symbol == symbol.symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                && occurrence_enclosing_range(occurrence).is_some()
        }) else {
            continue;
        };
        let Some(definition_range) = occurrence_enclosing_range(definition) else {
            continue;
        };
        for occurrence in &document.occurrences {
            let Some(reference_range) = occurrence_range(occurrence) else {
                continue;
            };
            if occurrence.symbol.is_empty()
                || occurrence.symbol == symbol.symbol
                || occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                || !range_contains(&definition_range, &reference_range)
                || !is_function_like_symbol_id(index, &occurrence.symbol)
            {
                continue;
            }
            let evidence_span =
                SourceSpan::from_scip_range(project_id, &document.relative_path, &reference_range)
                    .map_err(|_| ScipError::core_projection_failed())?;
            call_edges.push(CallEdgeSummary {
                enclosing_symbol_id: SymbolId::new(symbol.symbol.clone()),
                referenced_symbol_id: SymbolId::new(occurrence.symbol.clone()),
                evidence_span,
                confidence: "scip_reference_inside_function_enclosing_range".to_owned(),
            });
        }
    }
    Ok(call_edges)
}

fn is_function_like_symbol_id(index: &Index, symbol_id: &str) -> bool {
    index
        .documents
        .iter()
        .flat_map(|document| document.symbols.iter())
        .chain(index.external_symbols.iter())
        .find(|symbol| symbol.symbol == symbol_id)
        .map(is_function_like_symbol)
        .unwrap_or(false)
}

fn is_function_like_symbol(symbol: &SymbolInformation) -> bool {
    symbol
        .kind
        .enum_value()
        .ok()
        .map(project_symbol_kind)
        .map(|kind| is_function_like_kind(&kind))
        .unwrap_or(false)
}

fn is_function_like_kind(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Function
            | ElementKind::Method
            | ElementKind::StaticMethod
            | ElementKind::TraitMethod
    )
}

fn range_contains(outer: &[i32], inner: &[i32]) -> bool {
    let Some((outer_start_line, outer_start_column, outer_end_line, outer_end_column)) =
        range_bounds(outer)
    else {
        return false;
    };
    let Some((inner_start_line, inner_start_column, inner_end_line, inner_end_column)) =
        range_bounds(inner)
    else {
        return false;
    };
    position_lte(
        (outer_start_line, outer_start_column),
        (inner_start_line, inner_start_column),
    ) && position_lte(
        (inner_end_line, inner_end_column),
        (outer_end_line, outer_end_column),
    )
}

fn range_bounds(range: &[i32]) -> Option<(i32, i32, i32, i32)> {
    let bounds = match range {
        [line, start, end] => (*line, *start, *line, *end),
        [start_line, start_column, end_line, end_column] => {
            (*start_line, *start_column, *end_line, *end_column)
        }
        _ => return None,
    };
    let (start_line, start_column, end_line, end_column) = bounds;
    if start_line < 0
        || start_column < 0
        || end_column < 0
        || end_line < start_line
        || (end_line == start_line && end_column < start_column)
    {
        return None;
    }
    Some(bounds)
}

fn position_lte(left: (i32, i32), right: (i32, i32)) -> bool {
    left.0 < right.0 || (left.0 == right.0 && left.1 <= right.1)
}

fn occurrence_range(occurrence: &Occurrence) -> Option<Vec<i32>> {
    match &occurrence.typed_range {
        Some(occurrence::Typed_range::SingleLineRange(range)) => Some(vec![
            range.line,
            range.start_character,
            range.end_character,
        ]),
        Some(occurrence::Typed_range::MultiLineRange(range)) => Some(vec![
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        ]),
        _ if !occurrence.range.is_empty() => Some(occurrence.range.clone()),
        _ => None,
    }
}

fn occurrence_enclosing_range(occurrence: &Occurrence) -> Option<Vec<i32>> {
    match &occurrence.typed_enclosing_range {
        Some(occurrence::Typed_enclosing_range::SingleLineEnclosingRange(range)) => Some(vec![
            range.line,
            range.start_character,
            range.end_character,
        ]),
        Some(occurrence::Typed_enclosing_range::MultiLineEnclosingRange(range)) => Some(vec![
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        ]),
        _ if !occurrence.enclosing_range.is_empty() => Some(occurrence.enclosing_range.clone()),
        _ => None,
    }
}

fn reference_count(index: &Index, symbol_id: &str) -> usize {
    index
        .documents
        .iter()
        .flat_map(|document| document.occurrences.iter())
        .filter(|occurrence| {
            occurrence.symbol == symbol_id
                && occurrence.symbol_roles & SymbolRole::Definition as i32 == 0
        })
        .count()
}

fn package_name(symbol_id: &str) -> Option<String> {
    parse_symbol(symbol_id)
        .ok()
        .and_then(|symbol| symbol.package.as_ref().map(|package| package.name.clone()))
        .filter(|name| !name.is_empty())
}

fn project_signature(
    signature: &Signature,
    enclosing_symbol: &str,
    symbols: &[SymbolInformation],
    occurrences: &[Occurrence],
    project_id: &str,
    document_path: &str,
) -> ScipResult<FunctionSignatureSummary> {
    let signature_text = signature_text_from_scip(signature);
    let parameters = parameter_texts(&signature_text)
        .into_iter()
        .map(|parameter_text| {
            let display_name = parameter_display_name(parameter_text);
            let parameter_symbol = parameter_symbol_for(enclosing_symbol, &display_name, symbols);
            let source_span = parameter_symbol
                .map(|symbol| {
                    parameter_definition_span(
                        project_id,
                        document_path,
                        &symbol.symbol,
                        occurrences,
                    )
                })
                .transpose()?
                .flatten();
            let parameter_kind = parameter_symbol
                .and_then(|symbol| symbol.kind.enum_value().ok())
                .map(project_symbol_kind)
                .unwrap_or_else(|| {
                    if is_self_parameter(&display_name) {
                        ElementKind::SelfParameter
                    } else {
                        ElementKind::Parameter
                    }
                });
            let type_reference = parameter_text
                .split_once(':')
                .map(|(_, type_text)| type_reference_from_text(type_text.trim(), signature));
            FunctionParameterSummary {
                parameter_kind: format!("{parameter_kind:?}"),
                display_name,
                scip_symbol: parameter_symbol.map(|symbol| symbol.symbol.clone()),
                source_span,
                type_reference,
            }
        })
        .collect::<ScipResult<Vec<_>>>()?;
    let return_type = return_type_text(&signature_text)
        .map(|type_text| type_reference_from_text(type_text, signature));
    let mut unknown = Vec::new();
    if signature.occurrences.is_empty() {
        unknown.push(
            "SCIP signature occurrences were unavailable; parameter and return types are text-only"
                .to_owned(),
        );
    }
    Ok(FunctionSignatureSummary {
        signature_text,
        parameters,
        return_type,
        confirmed: vec!["signature text came from SCIP signature_documentation".to_owned()],
        unknown,
    })
}

fn parameter_definition_span(
    project_id: &str,
    document_path: &str,
    parameter_symbol: &str,
    occurrences: &[Occurrence],
) -> ScipResult<Option<SourceSpan>> {
    occurrences
        .iter()
        .find(|occurrence| {
            occurrence.symbol == parameter_symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
        })
        .map(|occurrence| {
            let range = occurrence_range(occurrence)
                .ok_or_else(ScipError::core_projection_failed)?;
            SourceSpan::from_scip_range(project_id, document_path, &range)
                .map_err(|_| ScipError::core_projection_failed())
        })
        .transpose()
}

fn signature_text_from_scip(signature: &Signature) -> String {
    signature.text.clone()
}

fn parameter_display_name(parameter_text: &str) -> String {
    let raw_name = parameter_text
        .split_once(':')
        .map(|(name, _)| name.trim())
        .unwrap_or(parameter_text.trim());
    raw_name
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim()
        .to_owned()
}

fn parameter_symbol_for<'a>(
    enclosing_symbol: &str,
    display_name: &str,
    symbols: &'a [SymbolInformation],
) -> Option<&'a SymbolInformation> {
    symbols.iter().find(|symbol| {
        if symbol.enclosing_symbol != enclosing_symbol {
            return false;
        }
        let Some(kind) = symbol.kind.enum_value().ok().map(project_symbol_kind) else {
            return false;
        };
        match kind {
            ElementKind::SelfParameter => {
                symbol.display_name == display_name || is_self_parameter(display_name)
            }
            ElementKind::Parameter => symbol.display_name == display_name,
            _ => false,
        }
    })
}

fn is_self_parameter(display_name: &str) -> bool {
    display_name == "self"
}

fn parameter_texts(signature_text: &str) -> Vec<&str> {
    let Some(open_index) = signature_text.find('(') else {
        return Vec::new();
    };
    let Some(close_index) = matching_close_paren(signature_text, open_index) else {
        return Vec::new();
    };
    split_top_level_commas(&signature_text[open_index + 1..close_index])
        .into_iter()
        .map(str::trim)
        .filter(|parameter| !parameter.is_empty())
        .collect()
}

fn return_type_text(signature_text: &str) -> Option<&str> {
    let open_index = signature_text.find('(')?;
    let close_index = matching_close_paren(signature_text, open_index)?;
    let after_params = signature_text.get(close_index + 1..)?.trim();
    let return_text = after_params.strip_prefix("->")?.trim();
    (!return_text.is_empty()).then_some(return_text)
}

fn type_reference_from_text(type_text: &str, signature: &Signature) -> TypeReferenceSummary {
    let occurrence = occurrence_for_text(signature, type_text);
    let scip_symbol = occurrence.map(|occurrence| occurrence.symbol.clone());
    let confidence = if scip_symbol.is_some() {
        "scip_symbol"
    } else {
        "signature_text_only"
    };
    TypeReferenceSummary {
        display_text: type_text.to_owned(),
        scip_symbol,
        source_span: None,
        signature_range: occurrence
            .and_then(occurrence_range)
            .and_then(|range| signature_range_from_scip(&range)),
        confidence: confidence.to_owned(),
    }
}

fn occurrence_for_text<'a>(
    signature: &'a Signature,
    type_text: &str,
) -> Option<&'a Occurrence> {
    let signature_text = signature_text_from_scip(signature);
    signature.occurrences.iter().find(|occurrence| {
        if occurrence.symbol.is_empty() {
            return false;
        }
        let Some(range) = occurrence_range(occurrence) else {
            return false;
        };
        signature_occurrence_text(&signature_text, &range)
            .map(|occurrence_text| type_text.contains(occurrence_text.trim()))
            .unwrap_or(false)
    })
}

fn signature_occurrence_text<'a>(signature_text: &'a str, range: &[i32]) -> Option<&'a str> {
    let (line, start, end) = match range {
        [line, start, end] => (*line, *start, *end),
        [start_line, start_column, end_line, end_column] if start_line == end_line => {
            (*start_line, *start_column, *end_column)
        }
        _ => return None,
    };
    if line != 0 || start < 0 || end < start {
        return None;
    }
    signature_text.get(usize::try_from(start).ok()?..usize::try_from(end).ok()?)
}

fn signature_range_from_scip(range: &[i32]) -> Option<Vec<u32>> {
    let (start_line, start_column, end_line, end_column) = match range {
        [line, start, end] => (*line, *start, *line, *end),
        [start_line, start_column, end_line, end_column] => {
            (*start_line, *start_column, *end_line, *end_column)
        }
        _ => return None,
    };
    if start_line < 0
        || start_column < 0
        || end_column < 0
        || end_line < start_line
        || (end_line == start_line && end_column < start_column)
    {
        return None;
    }
    Some(vec![
        one_based_signature_position(start_line)?,
        one_based_signature_position(start_column)?,
        one_based_signature_position(end_line)?,
        one_based_signature_position(end_column)?,
    ])
}

fn one_based_signature_position(position: i32) -> Option<u32> {
    u32::try_from(position.checked_add(1)?).ok()
}

fn matching_close_paren(text: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, character) in text.char_indices().skip_while(|(index, _)| *index < open_index) {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                if let Some(part) = text.get(start..index) {
                    parts.push(part);
                }
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if let Some(part) = text.get(start..) {
        parts.push(part);
    }
    parts
}
```

## Phase 4: 5W Summary Derivation

Implement 5W summaries in `refactor-radar-report` from normalized core facts.
The derivation must separate confirmed facts from inference and unknowns.

Confirmed examples:

- Who:
  - defining crate/package
  - enclosing module, impl, trait, or parent symbol when known
- What:
  - symbol kind
  - display name
  - signature documentation when present
  - for functions, parameter names, parameter SCIP IDs, return type text, and
    return type SCIP IDs when available
  - enum variants or fields when represented as child symbols
- Where:
  - definition span
  - top reference files
  - hotspot files ranked by reference count
- When:
  - SCIP generation timestamp if RefactorRadar records one
  - otherwise `"not available from SCIP"` as an unknown
- How:
  - callers and referenced function-like symbols by SCIP occurrence analysis
  - child symbols such as enum variants and fields
  - test references when test paths or test roles are detected

Inferred examples:

- Why:
  - likely domain role from name, module, references, and documentation
  - likely architectural role from reference hotspots
  - possible public API relevance when many files reference the symbol

Unknown examples:

- Whether a low-level symbol is part of intended public API.
- Whether references are semantically equivalent across workflows.
- Whether a SCIP-derived function reference is a runtime call expression.
- Whether a function return type can be resolved to a SCIP symbol when
  rust-analyzer only emitted signature text.
- Whether git recency or ownership matters when git metadata is not provided.

Do not make broad refactor recommendations from 5W summaries alone. They are
review context, not judgments.

Implementation snippets for report derivation:

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
    ElementBrief, EvidenceRecord, FiveWSummary, SemanticModel, SymbolReferenceSummary,
};
use std::collections::BTreeMap;

const SCIP_SYMBOL_ROLE_TEST: i32 = 32;

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
        .filter(|reference| reference.referenced_symbol_id.as_str() == element.symbol_id.as_str())
        .take(reference_limit)
        .cloned()
        .collect();
    let all_references: Vec<_> = model
        .references
        .iter()
        .filter(|reference| reference.referenced_symbol_id.as_str() == element.symbol_id.as_str())
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
    let outgoing_call_edges = model.call_edges_for_enclosing(element.symbol_id.as_str());
    let incoming_call_edges = model.call_edges_for_referenced(element.symbol_id.as_str());
    let mut confirmed = vec!["element came from SCIP SymbolInformation".to_owned()];
    let mut unknown = vec!["git recency is not available from SCIP".to_owned()];
    let mut what_summary = vec![format!("{:?} {}", element.kind, element.display_name)];
    if let Some(signature) = &element.signature {
        confirmed.extend(signature.confirmed.iter().cloned());
        unknown.extend(signature.unknown.iter().cloned());
        what_summary.push(format!("signature: {}", signature.signature_text));
        if !signature.parameters.is_empty() {
            let parameter_names = signature
                .parameters
                .iter()
                .map(|parameter| parameter.display_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            what_summary.push(format!("parameters: {parameter_names}"));
            let parameter_symbols = signature
                .parameters
                .iter()
                .filter_map(|parameter| parameter.scip_symbol.as_deref())
                .collect::<Vec<_>>();
            if !parameter_symbols.is_empty() {
                what_summary.push(format!(
                    "parameter SCIP IDs: {}",
                    parameter_symbols.join(", ")
                ));
            }
        }
        if let Some(return_type) = &signature.return_type {
            what_summary.push(format!("returns: {}", return_type.display_text));
            if let Some(return_symbol) = &return_type.scip_symbol {
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
        what_summary.push(format!("child symbols: {child_names}"));
    }
    let mut where_summary: Vec<String> = element
        .definition_span
        .as_ref()
        .map(|span| {
            format!(
                "defined at {}:{}:{}",
                span.document_path, span.start_line, span.start_column
            )
        })
        .into_iter()
        .collect();
    if !top_reference_files.is_empty() {
        let top_files = top_reference_files
            .iter()
            .take(reference_limit)
            .map(|(path, count)| format!("{path} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        where_summary.push(format!("top reference files: {top_files}"));
    }
    let inferred = if include_inferred && element.reference_count > 3 {
        vec!["many SCIP references may indicate API relevance".to_owned()]
    } else {
        Vec::new()
    };
    let mut who = Vec::new();
    if let Some(package) = &element.package {
        who.push(package.clone());
    }
    if let Some(enclosing_symbol) = &element.enclosing_symbol {
        who.push(enclosing_symbol.clone());
    }
    Ok(ElementBrief {
        element: element.clone(),
        five_w: FiveWSummary {
            who,
            what_summary,
            where_summary,
            when_summary: vec!["unknown: not available from SCIP".to_owned()],
            why_summary: inferred.clone(),
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
                .chain(outgoing_call_edges.iter().take(reference_limit).map(|edge| {
                    format!(
                        "SCIP-derived outgoing function_like_reference to {} at {}:{}:{}",
                        edge.referenced_symbol_id.as_str(),
                        edge.evidence_span.document_path,
                        edge.evidence_span.start_line,
                        edge.evidence_span.start_column
                    )
                }))
                .chain(incoming_call_edges.iter().take(reference_limit).map(|edge| {
                    format!(
                        "SCIP-derived incoming function_like_reference from {} at {}:{}:{}",
                        edge.enclosing_symbol_id.as_str(),
                        edge.evidence_span.document_path,
                        edge.evidence_span.start_line,
                        edge.evidence_span.start_column
                    )
                }))
                .collect(),
        },
        evidence: references
            .iter()
            .map(|reference| EvidenceRecord {
                label: "SCIP occurrence".to_owned(),
                source: reference.document_path.clone(),
                span: Some(reference.source_span.clone()),
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

fn is_test_reference(reference: &SymbolReferenceSummary) -> bool {
    reference.role_flags & SCIP_SYMBOL_ROLE_TEST != 0
        || reference.document_path.contains("/tests/")
        || reference.document_path.ends_with("_test.rs")
        || reference.document_path.ends_with("_tests.rs")
}
```

```rust
// crates/refactor-radar-report/src/generate_llm_brief.rs
use crate::{build_element_brief, build_project_summary, ReportResult};
use refactor_radar_core::SemanticModel;

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
    let mut ranked_elements: Vec<_> = model.elements.iter().collect();
    ranked_elements.sort_by(|left, right| {
        right
            .reference_count
            .cmp(&left.reference_count)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.symbol_id.as_str().cmp(right.symbol_id.as_str()))
    });
    for element in ranked_elements.into_iter().take(25) {
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

## Phase 5: MCP Interface

Initial MCP tools:

- `generate_rust_scip(path, output_path, rust_analyzer_path?, config_path?, exclude_vendored_libraries?, num_threads?)`
  - Generates a `.scip` file through rust-analyzer.
  - Returns path, stderr digest, producer metadata when available, and file size.
- `load_scip_project(path, format?)`
  - Reads an existing `.scip` or protobuf JSON file.
  - Projects it into the in-memory RefactorRadar model cache.
  - Returns `project_id`, document count, symbol count, and occurrence count.
- `scan_project(path, output_path?, rust_analyzer_path?, config_path?)`
  - Convenience tool that generates SCIP into a temp or supplied path, then
    loads it into the cache.
- `get_project_summary(project_id)`
- `list_elements(project_id, kind?, path?, limit?)`
- `get_element(symbol_id)`
- `get_function_signature(symbol_id)`
- `get_element_brief(symbol_id, include_inferred?, reference_limit?)`
- `get_5w_summary(symbol_id, reference_limit?)`
- `find_references(symbol_id, limit?)`
- `find_related_symbols(symbol_id, limit?)`
- `get_source_span(span_id)`
- `generate_llm_brief(project_id, budget_tokens?)`

Response rules:

- Return compact structured JSON by default.
- Never return raw source by default.
- Always include stable IDs and source span IDs for follow-up queries.
- Preserve SCIP IDs in responses for traceability.
- Label `confirmed`, `inferred`, and `unknown` explicitly.
- For SCIP-derived call data, use language such as `function_like_references`
  unless an AST supplement has confirmed exact call expressions.

## Phase 6: MCP Server Crate

Recommended `crates/refactor-radar-mcp/src` layout:

```text
lib.rs
element_query_params.rs
find_references_params.rs
find_related_symbols_params.rs
generate_llm_brief_params.rs
generate_rust_scip_params.rs
generate_rust_scip_result.rs
get_5w_summary_params.rs
get_element_brief_params.rs
get_element_params.rs
get_function_signature_params.rs
get_project_summary_params.rs
get_source_span_params.rs
load_scip_project_params.rs
load_scip_project_result.rs
mcp_error_conversion.rs
mcp_server_error.rs
mcp_server_result.rs
refactor_radar_mcp_server.rs
scan_cache.rs
scan_project_params.rs
scan_project_result.rs
```

The server cache should store:

- projected `SemanticModel` values by `project_id`
- source SCIP paths by `project_id`
- producer metadata by `project_id`
- optional generation diagnostics by `project_id`

Error mapping:

- missing project, element, symbol, or span maps to MCP resource-not-found
- rust-analyzer missing maps to a typed internal MCP error with a stable message
- rust-analyzer nonzero exit maps to a typed internal MCP error with captured
  stderr in structured error data
- malformed SCIP/protobuf JSON maps to a typed internal MCP error

`src/bin/refactor-radar-mcp.rs` may write startup failure messages to stderr.
Library code and MCP handlers must not write to stdout.

Implementation snippets for the MCP server crate:

```rust
// crates/refactor-radar-mcp/src/lib.rs
pub mod element_query_params;
pub mod find_references_params;
pub mod find_related_symbols_params;
pub mod generate_llm_brief_params;
pub mod generate_rust_scip_params;
pub mod generate_rust_scip_result;
pub mod get_5w_summary_params;
pub mod get_element_brief_params;
pub mod get_element_params;
pub mod get_function_signature_params;
pub mod get_project_summary_params;
pub mod get_source_span_params;
pub mod load_scip_project_params;
pub mod load_scip_project_result;
pub mod mcp_error_conversion;
pub mod mcp_server_error;
pub mod mcp_server_result;
pub mod refactor_radar_mcp_server;
pub mod scan_cache;
pub mod scan_project_params;
pub mod scan_project_result;

pub use mcp_server_error::McpServerError;
pub use mcp_server_result::McpServerResult;
pub use refactor_radar_mcp_server::RefactorRadarMcpServer;
```

```rust
// crates/refactor-radar-mcp/src/load_scip_project_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LoadScipProjectParams {
    pub path: String,
    pub format: Option<String>,
}

// crates/refactor-radar-mcp/src/load_scip_project_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct LoadScipProjectResult {
    pub project_id: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}

// crates/refactor-radar-mcp/src/generate_rust_scip_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GenerateRustScipParams {
    pub path: String,
    pub output_path: String,
    pub rust_analyzer_path: Option<String>,
    pub config_path: Option<String>,
    pub exclude_vendored_libraries: Option<bool>,
    pub num_threads: Option<usize>,
}

// crates/refactor-radar-mcp/src/generate_rust_scip_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct GenerateRustScipResult {
    pub path: String,
    pub stderr_digest: String,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
    pub file_size: u64,
}
```

```rust
// crates/refactor-radar-mcp/src/scan_project_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ScanProjectParams {
    pub path: String,
    pub output_path: Option<String>,
    pub rust_analyzer_path: Option<String>,
    pub config_path: Option<String>,
}

// crates/refactor-radar-mcp/src/scan_project_result.rs
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ScanProjectResult {
    pub project_id: String,
    pub output_path: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
```

```rust
// crates/refactor-radar-mcp/src/element_query_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ElementQueryParams {
    pub project_id: String,
    pub kind: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
}

// crates/refactor-radar-mcp/src/find_references_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FindReferencesParams {
    pub symbol_id: String,
    pub limit: Option<usize>,
}

// crates/refactor-radar-mcp/src/find_related_symbols_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FindRelatedSymbolsParams {
    pub symbol_id: String,
    pub limit: Option<usize>,
}

// crates/refactor-radar-mcp/src/generate_llm_brief_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GenerateLlmBriefParams {
    pub project_id: String,
    pub budget_tokens: Option<usize>,
}
```

```rust
// crates/refactor-radar-mcp/src/get_5w_summary_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct Get5wSummaryParams {
    pub symbol_id: String,
    pub reference_limit: Option<usize>,
}

// crates/refactor-radar-mcp/src/get_element_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetElementParams {
    pub symbol_id: String,
}

// crates/refactor-radar-mcp/src/get_element_brief_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetElementBriefParams {
    pub symbol_id: String,
    pub include_inferred: Option<bool>,
    pub reference_limit: Option<usize>,
}

// crates/refactor-radar-mcp/src/get_function_signature_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetFunctionSignatureParams {
    pub symbol_id: String,
}

// crates/refactor-radar-mcp/src/get_project_summary_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetProjectSummaryParams {
    pub project_id: String,
}

// crates/refactor-radar-mcp/src/get_source_span_params.rs
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanParams {
    pub span_id: String,
}
```

```rust
// crates/refactor-radar-mcp/src/scan_cache.rs
use refactor_radar_core::SemanticModel;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct ScanCache {
    pub models: HashMap<String, SemanticModel>,
    pub scip_paths: HashMap<String, PathBuf>,
    pub producer_metadata: HashMap<String, String>,
    pub generation_diagnostics: HashMap<String, String>,
}

impl ScanCache {
    pub fn insert(&mut self, model: SemanticModel, scip_path: PathBuf, diagnostics: Option<String>) {
        let project_id = model.project.project_id.clone();
        if let Some(producer_name) = &model.project.producer_name {
            self.producer_metadata
                .insert(project_id.clone(), producer_name.clone());
        }
        if let Some(diagnostics) = diagnostics {
            self.generation_diagnostics
                .insert(project_id.clone(), diagnostics);
        }
        self.scip_paths.insert(project_id.clone(), scip_path);
        self.models.insert(project_id, model);
    }
}
```

```rust
// crates/refactor-radar-mcp/src/mcp_server_error.rs
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("{message} at {location}")]
    NotFound {
        message: &'static str,
        id: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    Internal {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    ServerRuntime {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
}

impl McpServerError {
    #[track_caller]
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound {
            message: "requested RefactorRadar resource was not found",
            id: id.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal {
            message: "RefactorRadar MCP tool failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn server_runtime(details: impl Into<String>) -> Self {
        Self::ServerRuntime {
            message: "RefactorRadar MCP server failed",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::NotFound { message, .. }
            | Self::Internal { message, .. }
            | Self::ServerRuntime { message, .. } => message,
        }
    }
}

// crates/refactor-radar-mcp/src/mcp_server_result.rs
use crate::mcp_server_error::McpServerError;

pub type McpServerResult<T> = std::result::Result<T, McpServerError>;
```

```rust
// crates/refactor-radar-mcp/src/mcp_error_conversion.rs
use crate::McpServerError;
use refactor_radar_scip::ScipError;
use rmcp::ErrorData as McpError;
use serde_json::json;

pub fn to_mcp_error(error: McpServerError) -> McpError {
    match error {
        McpServerError::NotFound { message, id, .. } => {
            McpError::resource_not_found(message, Some(json!({ "id": id })))
        }
        McpServerError::Internal { message, details, .. } => {
            McpError::internal_error(message, Some(json!({ "details": details })))
        }
        McpServerError::ServerRuntime { message, details, .. } => {
            McpError::internal_error(message, Some(json!({ "details": details })))
        }
    }
}

impl From<McpServerError> for McpError {
    fn from(error: McpServerError) -> Self {
        to_mcp_error(error)
    }
}

pub fn scip_to_mcp_error(error: ScipError) -> McpError {
    match error {
        ScipError::UnknownFormat { message, path, .. } => {
            McpError::internal_error(message, Some(json!({ "path": path })))
        }
        ScipError::RustAnalyzerMissing {
            message,
            executable,
            ..
        } => McpError::internal_error(message, Some(json!({ "executable": executable }))),
        ScipError::RustAnalyzerLaunchFailed {
            message,
            executable,
            details,
            ..
        } => McpError::internal_error(
            message,
            Some(json!({ "executable": executable, "details": details })),
        ),
        ScipError::RustAnalyzerFailed {
            message,
            status,
            stderr,
            ..
        } => McpError::internal_error(
            message,
            Some(json!({ "status": status, "stderr": stderr })),
        ),
        ScipError::ReadFailed { message, path, .. }
        | ScipError::ParseFailed { message, path, .. } => {
            McpError::internal_error(message, Some(json!({ "path": path })))
        }
        ScipError::CoreProjectionFailed { message, .. } => {
            McpError::internal_error(message, None)
        }
    }
}
```

```rust
// crates/refactor-radar-mcp/src/refactor_radar_mcp_server.rs
use crate::{
    element_query_params::ElementQueryParams,
    find_references_params::FindReferencesParams,
    find_related_symbols_params::FindRelatedSymbolsParams,
    generate_llm_brief_params::GenerateLlmBriefParams,
    generate_rust_scip_params::GenerateRustScipParams,
    generate_rust_scip_result::GenerateRustScipResult,
    get_5w_summary_params::Get5wSummaryParams,
    get_element_brief_params::GetElementBriefParams,
    get_element_params::GetElementParams,
    get_function_signature_params::GetFunctionSignatureParams,
    get_project_summary_params::GetProjectSummaryParams,
    get_source_span_params::GetSourceSpanParams,
    load_scip_project_params::LoadScipProjectParams,
    load_scip_project_result::LoadScipProjectResult,
    mcp_error_conversion::scip_to_mcp_error,
    mcp_server_error::McpServerError,
    scan_cache::ScanCache,
    scan_project_params::ScanProjectParams,
    scan_project_result::ScanProjectResult,
};
use refactor_radar_report::{
    build_element_brief, build_project_summary, generate_llm_brief as render_llm_brief,
};
use refactor_radar_scip::{
    generate_rust_scip as run_rust_analyzer_scip, project_scip_index, Format,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::{Json, Parameters}},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RefactorRadarMcpServer {
    cache: Arc<RwLock<ScanCache>>,
    tool_router: ToolRouter<Self>,
}

impl RefactorRadarMcpServer {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(ScanCache::default())),
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for RefactorRadarMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RefactorRadarMcpServer {
    #[tool(name = "load_scip_project", description = "Load a SCIP index into the RefactorRadar cache")]
    async fn load_scip_project(
        &self,
        Parameters(params): Parameters<LoadScipProjectParams>,
    ) -> Result<Json<LoadScipProjectResult>, McpError> {
        let path = PathBuf::from(&params.path);
        let format = match params.format.as_deref() {
            Some("scip") => Some(Format::Binary),
            Some("json") => Some(Format::Json),
            Some(other) => {
                return Err(McpError::invalid_params(
                    "unsupported SCIP format",
                    Some(serde_json::json!({ "format": other })),
                ));
            }
            None => None,
        };
        let model = project_scip_index(&path, format).map_err(scip_to_mcp_error)?;
        let result = LoadScipProjectResult {
            project_id: model.project.project_id.clone(),
            document_count: model.files.len(),
            symbol_count: model.elements.len(),
            occurrence_count: occurrence_count(&model),
        };
        self.cache.write().await.insert(model, path, None);
        Ok(Json(result))
    }

    #[tool(name = "generate_rust_scip", description = "Generate a rust-analyzer SCIP file")]
    async fn generate_rust_scip(
        &self,
        Parameters(params): Parameters<GenerateRustScipParams>,
    ) -> Result<Json<GenerateRustScipResult>, McpError> {
        let output_path = PathBuf::from(&params.output_path);
        let rust_analyzer_path = params.rust_analyzer_path.as_deref().map(PathBuf::from);
        let config_path = params.config_path.as_deref().map(PathBuf::from);
        let stderr = run_rust_analyzer_scip(
            &PathBuf::from(&params.path),
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            params.exclude_vendored_libraries.unwrap_or(false),
            params.num_threads,
        )
        .await
        .map_err(scip_to_mcp_error)?;
        let file_size = std::fs::metadata(&output_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let (producer_name, producer_version) =
            project_scip_index(&output_path, Some(Format::Binary))
                .ok()
                .map(|model| (model.project.producer_name, model.project.producer_version))
                .unwrap_or((None, None));
        Ok(Json(GenerateRustScipResult {
            path: output_path.display().to_string(),
            stderr_digest: stderr.chars().take(500).collect(),
            producer_name,
            producer_version,
            file_size,
        }))
    }

    #[tool(name = "scan_project", description = "Generate SCIP and load it into the cache")]
    async fn scan_project(
        &self,
        Parameters(params): Parameters<ScanProjectParams>,
    ) -> Result<Json<ScanProjectResult>, McpError> {
        let output_path = match &params.output_path {
            Some(output_path) => PathBuf::from(output_path),
            None => {
                let file = tempfile::Builder::new()
                    .suffix(".scip")
                    .tempfile()
                    .map_err(|error| {
                        McpError::internal_error(
                            "failed to create temporary SCIP output file",
                            Some(serde_json::json!({ "details": error.to_string() })),
                        )
                    })?;
                file.into_temp_path().keep().map_err(|error| {
                    McpError::internal_error(
                        "failed to keep temporary SCIP output path",
                        Some(serde_json::json!({ "details": error.to_string() })),
                    )
                })?
            }
        };
        let rust_analyzer_path = params.rust_analyzer_path.as_deref().map(PathBuf::from);
        let config_path = params.config_path.as_deref().map(PathBuf::from);
        let stderr = run_rust_analyzer_scip(
            &PathBuf::from(&params.path),
            &output_path,
            rust_analyzer_path.as_deref(),
            config_path.as_deref(),
            false,
            None,
        )
        .await
        .map_err(scip_to_mcp_error)?;
        let model =
            project_scip_index(&output_path, Some(Format::Binary)).map_err(scip_to_mcp_error)?;
        let result = ScanProjectResult {
            project_id: model.project.project_id.clone(),
            output_path: output_path.display().to_string(),
            document_count: model.files.len(),
            symbol_count: model.elements.len(),
            occurrence_count: occurrence_count(&model),
        };
        self.cache.write().await.insert(model, output_path, Some(stderr));
        Ok(Json(result))
    }

    #[tool(name = "get_project_summary", description = "Return one cached project summary")]
    async fn get_project_summary(
        &self,
        Parameters(params): Parameters<GetProjectSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::not_found(params.project_id.clone()))?;
        structured(build_project_summary(model))
    }

    #[tool(name = "list_elements", description = "List cached semantic elements")]
    async fn list_elements(
        &self,
        Parameters(params): Parameters<ElementQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::not_found(params.project_id.clone()))?;
        let limit = params.limit.unwrap_or(100);
        let elements: Vec<_> = model
            .elements
            .iter()
            .filter(|element| {
                params
                    .kind
                    .as_ref()
                    .map(|kind| normalize_kind(&format!("{:?}", element.kind)) == normalize_kind(kind))
                    .unwrap_or(true)
            })
            .filter(|element| {
                params.path.as_ref().map(|path| {
                    element
                        .definition_span
                        .as_ref()
                        .map(|span| span.document_path == *path)
                        .unwrap_or(false)
                }).unwrap_or(true)
            })
            .take(limit)
            .collect();
        structured(elements)
    }

    #[tool(name = "get_element", description = "Return one element by stable ID or SCIP symbol")]
    async fn get_element(
        &self,
        Parameters(params): Parameters<GetElementParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let element = cache
            .models
            .values()
            .find_map(|model| model.element_by_id(&params.symbol_id))
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        structured(element)
    }

    #[tool(name = "get_function_signature", description = "Return function signature facts")]
    async fn get_function_signature(
        &self,
        Parameters(params): Parameters<GetFunctionSignatureParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let element = cache
            .models
            .values()
            .find_map(|model| model.element_by_id(&params.symbol_id))
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        structured(serde_json::json!({
            "stable_id": element.stable_id.as_str(),
            "symbol_id": element.symbol_id.as_str(),
            "definition_span_id": element.definition_span.as_ref().map(|span| span.id.as_str()),
            "signature": &element.signature,
        }))
    }

    #[tool(name = "get_element_brief", description = "Return a 5W element brief")]
    async fn get_element_brief(
        &self,
        Parameters(params): Parameters<GetElementBriefParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .values()
            .find(|model| model.element_by_id(&params.symbol_id).is_some())
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        let brief = build_element_brief(
            model,
            &params.symbol_id,
            params.include_inferred.unwrap_or(true),
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpError::internal_error(error.message(), None))?;
        structured(brief)
    }

    #[tool(name = "get_5w_summary", description = "Return the 5W summary and follow-up IDs for an element")]
    async fn get_5w_summary(
        &self,
        Parameters(params): Parameters<Get5wSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .values()
            .find(|model| model.element_by_id(&params.symbol_id).is_some())
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        let brief = build_element_brief(
            model,
            &params.symbol_id,
            true,
            params.reference_limit.unwrap_or(20),
        )
        .map_err(|error| McpError::internal_error(error.message(), None))?;
        let stable_id = brief.element.stable_id.as_str().to_owned();
        let symbol_id = brief.element.symbol_id.as_str().to_owned();
        let definition_span_id = brief
            .element
            .definition_span
            .as_ref()
            .map(|span| span.id.as_str().to_owned());
        structured(serde_json::json!({
            "stable_id": stable_id,
            "symbol_id": symbol_id,
            "definition_span_id": definition_span_id,
            "five_w": brief.five_w,
        }))
    }

    #[tool(name = "find_references", description = "Return span metadata for symbol references")]
    async fn find_references(
        &self,
        Parameters(params): Parameters<FindReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let symbol_id = canonical_symbol_id(&cache, &params.symbol_id)
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        let references: Vec<_> = cache
            .models
            .values()
            .flat_map(|model| model.references.iter())
            .filter(|reference| reference.referenced_symbol_id.as_str() == symbol_id.as_str())
            .take(limit)
            .collect();
        structured(references)
    }

    #[tool(name = "find_related_symbols", description = "Return child symbols and SCIP-derived function-like relations")]
    async fn find_related_symbols(
        &self,
        Parameters(params): Parameters<FindRelatedSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params.limit.unwrap_or(100);
        let cache = self.cache.read().await;
        let symbol_id = canonical_symbol_id(&cache, &params.symbol_id)
            .ok_or_else(|| McpServerError::not_found(params.symbol_id.clone()))?;
        let child_symbols: Vec<_> = cache
            .models
            .values()
            .flat_map(|model| model.elements.iter())
            .filter(|element| element.enclosing_symbol.as_deref() == Some(symbol_id.as_str()))
            .take(limit)
            .collect();
        let outgoing_function_like_references: Vec<_> = cache
            .models
            .values()
            .flat_map(|model| model.call_edges_for_enclosing(&symbol_id))
            .take(limit)
            .collect();
        let incoming_function_like_references: Vec<_> = cache
            .models
            .values()
            .flat_map(|model| model.call_edges_for_referenced(&symbol_id))
            .take(limit)
            .collect();
        let related = serde_json::json!({
            "child_symbols": child_symbols,
            "outgoing_function_like_references": outgoing_function_like_references,
            "incoming_function_like_references": incoming_function_like_references,
        });
        structured(related)
    }

    #[tool(name = "get_source_span", description = "Return source span metadata without raw source")]
    async fn get_source_span(
        &self,
        Parameters(params): Parameters<GetSourceSpanParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let span = cache
            .models
            .values()
            .flat_map(|model| model.spans.iter())
            .find(|span| span.id.as_str() == params.span_id.as_str())
            .ok_or_else(|| McpServerError::not_found(params.span_id.clone()))?;
        structured(span)
    }

    #[tool(name = "generate_llm_brief", description = "Return a compact Markdown project brief")]
    async fn generate_llm_brief(
        &self,
        Parameters(params): Parameters<GenerateLlmBriefParams>,
    ) -> Result<CallToolResult, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .models
            .get(&params.project_id)
            .ok_or_else(|| McpServerError::not_found(params.project_id.clone()))?;
        let brief = render_llm_brief(model, params.budget_tokens)
            .map_err(|error| McpError::internal_error(error.message(), None))?;
        structured(serde_json::json!({ "markdown": brief }))
    }
}

fn structured(value: impl Serialize) -> Result<CallToolResult, McpError> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(|error| McpError::internal_error("failed to serialize MCP tool result", Some(serde_json::json!({ "details": error.to_string() }))))
}

fn occurrence_count(model: &refactor_radar_core::SemanticModel) -> usize {
    model.files.iter().map(|file| file.occurrence_count).sum()
}

fn canonical_symbol_id(cache: &ScanCache, requested_id: &str) -> Option<String> {
    if let Some(symbol_id) = cache
        .models
        .values()
        .find_map(|model| model.element_by_id(requested_id))
        .map(|element| element.symbol_id.as_str().to_owned())
    {
        return Some(symbol_id);
    }
    let scip_id = requested_id.strip_prefix("scip:").unwrap_or(requested_id);
    cache
        .models
        .values()
        .any(|model| {
            model
                .references
                .iter()
                .any(|reference| reference.referenced_symbol_id.as_str() == scip_id)
                || model
                    .elements
                    .iter()
                    .any(|element| element.enclosing_symbol.as_deref() == Some(scip_id))
                || model.call_edges.iter().any(|edge| {
                    edge.enclosing_symbol_id.as_str() == scip_id
                        || edge.referenced_symbol_id.as_str() == scip_id
                })
        })
        .then(|| scip_id.to_owned())
}

fn normalize_kind(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RefactorRadarMcpServer {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo` aliases rmcp's non-exhaustive `InitializeResult`; use its
        // public constructor instead of a struct literal.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("SCIP-backed RefactorRadar semantic query server")
    }
}
```

```rust
// crates/refactor-radar-mcp/src/bin/refactor-radar-mcp.rs
use refactor_radar_mcp::{McpServerError, McpServerResult, RefactorRadarMcpServer};
use rmcp::{transport::stdio, ServiceExt};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run refactor-radar MCP server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> McpServerResult<()> {
    let service = RefactorRadarMcpServer::new()
        .serve(stdio())
        .await
        .map_err(|error| McpServerError::server_runtime(error.to_string()))?;
    service
        .waiting()
        .await
        .map_err(|error| McpServerError::server_runtime(error.to_string()))?;
    Ok(())
}
```

## Phase 7: Fixture Coverage

Create small Rust fixture crates under `testdata/rust`.

Minimum fixture cases:

- basic crate with public functions, private helpers, an enum, and tests
- crate with enum variants and fields used across multiple files
- crate with methods in impl blocks
- crate with typed function parameters and a `Result<T, E>` return type
- crate with a function parameter of type `impl FnOnce()` to document the
  SCIP-only limitation around callable locals
- crate with repeated references to one central error enum

The tests may use a fake rust-analyzer executable for subprocess behavior and a
checked-in tiny SCIP fixture for projection behavior. Do not require the real
rust-analyzer binary for every unit test.

Concrete fixture snippets:

```toml
# testdata/rust/basic_crate/Cargo.toml
[package]
name = "basic-crate"
version = "0.1.0"
edition = "2024"
```

```rust
// testdata/rust/basic_crate/src/lib.rs
pub mod fixture_error;
pub mod processor;
pub mod record;
pub mod status;
pub mod usage;

pub use fixture_error::FixtureError;
pub use processor::Processor;
pub use record::Record;
pub use status::Status;

pub fn public_sum(left: i32, right: i32) -> i32 {
    private_offset(left) + right
}

pub fn parse_record(input: &str) -> Result<Record, FixtureError> {
    let id = input
        .parse::<u32>()
        .map_err(|_| FixtureError::InvalidId)?;
    Ok(Record {
        id,
        status: Status::Ready,
    })
}

pub fn run_callback(callback: impl FnOnce() -> usize) -> usize {
    callback()
}

fn private_offset(value: i32) -> i32 {
    value + 1
}

#[cfg(test)]
mod tests {
    use super::{parse_record, public_sum, run_callback, FixtureError, Status};
    use crate::usage::{describe, repeated_error_one, repeated_error_two};

    #[test]
    fn parses_record_and_reports_variants() {
        let parsed = parse_record("7");
        assert!(parsed.is_ok());
        let Ok(record) = parsed else {
            return;
        };
        assert_eq!(Status::Ready, record.status);
        assert_eq!("ready", describe(&record));
    }

    #[test]
    fn exercises_private_helper_and_callback_shape() {
        assert_eq!(4, public_sum(1, 2));
        assert_eq!(3, run_callback(|| 3));
    }

    #[test]
    fn repeats_central_error_references() {
        assert_eq!(FixtureError::InvalidId, repeated_error_one());
        assert_eq!(FixtureError::InvalidId, repeated_error_two());
    }
}
```

```rust
// testdata/rust/basic_crate/src/fixture_error.rs
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureError {
    InvalidId,
}
```

```rust
// testdata/rust/basic_crate/src/status.rs
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Ready,
    Blocked,
}
```

```rust
// testdata/rust/basic_crate/src/record.rs
use crate::Status;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub id: u32,
    pub status: Status,
}
```

```rust
// testdata/rust/basic_crate/src/processor.rs
use crate::{Record, Status};

#[derive(Clone, Debug, Default)]
pub struct Processor;

impl Processor {
    pub fn mark_blocked(&self, record: &mut Record) {
        record.status = Status::Blocked;
    }
}
```

```rust
// testdata/rust/basic_crate/src/usage.rs
use crate::{FixtureError, Record, Status};

pub fn describe(record: &Record) -> &'static str {
    match record.status {
        Status::Ready => "ready",
        Status::Blocked => "blocked",
    }
}

pub fn repeated_error_one() -> FixtureError {
    FixtureError::InvalidId
}

pub fn repeated_error_two() -> FixtureError {
    FixtureError::InvalidId
}
```

## Phase 8: Tests

Core tests:

- stable ID generation is deterministic
- SCIP symbol strings are preserved in `ElementSummary`
- source span conversion returns one-based line and column fields
- `ElementBrief` serializes with `who`, `what`, `where`, `when`, `why`, and
  `how` fields
- evidence records separate confirmed, inferred, and unknown fields

SCIP crate tests:

- round-trips binary `.scip`
- round-trips protobuf JSON
- detects format by extension
- rejects unknown extensions without explicit format
- maps SCIP documents, symbols, occurrences, roles, and ranges into core model
- projects return-type SCIP IDs from `Signature.occurrences` when present
- prefers typed SCIP occurrence and enclosing ranges over deprecated range
  vectors when present
- derives `CallEdgeSummary` values from function-like definition enclosing
  ranges and function-like reference occurrences
- maps missing rust-analyzer to a typed error
- maps non-`NotFound` rust-analyzer launch failures to a typed launch error
- maps rust-analyzer nonzero exit to a typed error with stderr
- preserves rust-analyzer diagnostics without writing them to stdout

Report tests:

- project summary counts documents, elements, references, and hotspots
- element brief for an enum includes variants and top reference files
- element brief for a function includes function-like references when present
- element brief for a function includes parameter SCIP IDs and return-type SCIP
  IDs when available
- 5W summary labels unavailable recency as unknown when git metadata is absent
- LLM brief separates confirmed facts from inference and unknowns
- token budget reduction removes lower-signal sections first

MCP tests:

- `load_scip_project` caches a projected model
- `generate_rust_scip` works with a fake rust-analyzer executable and returns
  output path, stderr digest, producer metadata, and file size
- `scan_project` works with a fake rust-analyzer executable
- `scan_project` maps fake rust-analyzer nonzero stderr into structured MCP
  error data
- `get_project_summary` returns compact structured JSON
- `list_elements` supports kind/path/limit filters
- `get_element` returns one element by stable ID or SCIP ID
- `get_function_signature` returns parameter and return-type references,
  including SCIP IDs where available and explicit unknowns where unavailable
- `get_element_brief` returns 5W fields and evidence labels
- `find_references` accepts stable IDs and SCIP IDs and returns span metadata
  only
- `find_related_symbols` accepts stable IDs and SCIP IDs and returns child
  symbols plus incoming and outgoing SCIP-derived function-like references
- `get_source_span` returns span metadata only
- `generate_llm_brief` returns Markdown text for a cached project
- unknown IDs map to MCP resource-not-found errors

Use given/when/then test names and create concrete tests in these files:

```rust
// crates/refactor-radar-core/tests/model_tests.rs
use refactor_radar_core::{
    ElementBrief, ElementKind, ElementSummary, EvidenceRecord, FiveWSummary, Language, SourceSpan,
    StableId, SymbolId,
};
use serde_json::Value;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";

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
    Ok(())
}

#[test]
fn given_element_brief_when_serializing_then_five_w_and_evidence_labels_are_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let brief = ElementBrief {
        element: sample_element(),
        five_w: FiveWSummary {
            who: vec!["basic_crate".to_owned()],
            what_summary: vec!["Function public_sum".to_owned()],
            where_summary: vec!["defined at src/lib.rs:1:1".to_owned()],
            when_summary: vec!["not available from SCIP".to_owned()],
            why_summary: vec!["many SCIP references may indicate API relevance".to_owned()],
            how: vec!["SCIP-derived outgoing function_like_reference".to_owned()],
        },
        evidence: vec![EvidenceRecord {
            label: "SCIP occurrence".to_owned(),
            source: "src/lib.rs".to_owned(),
            span: None,
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

```rust
// crates/refactor-radar-scip/tests/project_scip_index_tests.rs
use protobuf::{Message, MessageField};
use refactor_radar_scip::{generate_rust_scip, project_scip_index, Format, ScipError};
use scip::types::{
    symbol_information, Document, Index, Metadata, MultiLineRange, Occurrence, Signature,
    SingleLineRange, SymbolInformation, SymbolRole, ToolInfo,
};
use tempfile::tempdir;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
const PRIVATE_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
const RETURN_SYMBOL: &str = "rust-analyzer cargo core 0.0.0 primitive/i32#";

#[test]
fn given_binary_scip_when_projecting_then_symbols_and_ranges_are_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!("fixture", model.project.project_id);
    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    assert_eq!(Some(SYMBOL), model.elements.first().map(|element| element.scip_symbol.as_str()));
    assert_eq!(Some(1), model.spans.first().map(|span| span.start_line));
    Ok(())
}

#[test]
fn given_typed_scip_range_when_projecting_then_typed_range_takes_precedence(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    let mut index = sample_index();
    if let Some(document) = index.documents.first_mut() {
        if let Some(occurrence) = document.occurrences.first_mut() {
            occurrence.range = vec![9, 9, 12];
            occurrence.set_single_line_range(SingleLineRange {
                line: 2,
                start_character: 4,
                end_character: 9,
                ..Default::default()
            });
        }
    }
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(Some(3), model.spans.first().map(|span| span.start_line));
    assert_eq!(Some(5), model.spans.first().map(|span| span.start_column));
    Ok(())
}

#[test]
fn given_json_scip_when_projecting_then_explicit_format_reads_protobuf_json(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.json");
    let json = protobuf_json_mapping::print_to_string(&sample_index())?;
    std::fs::write(&path, json)?;

    let model = project_scip_index(&path, Some(Format::Json))?;

    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    Ok(())
}

#[test]
fn given_json_extension_when_format_is_not_explicit_then_format_is_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.json");
    let json = protobuf_json_mapping::print_to_string(&sample_index())?;
    std::fs::write(&path, json)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    Ok(())
}

#[test]
fn given_scip_reference_inside_function_when_projecting_then_reference_and_call_edge_are_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index_with_call_edge().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.references.len());
    assert_eq!(PRIVATE_SYMBOL, model.references[0].referenced_symbol_id.as_str());
    assert_eq!(0, model.references[0].role_flags);
    assert_eq!(1, model.call_edges.len());
    assert_eq!(SYMBOL, model.call_edges[0].enclosing_symbol_id.as_str());
    assert_eq!(PRIVATE_SYMBOL, model.call_edges[0].referenced_symbol_id.as_str());
    Ok(())
}

#[test]
fn given_typed_enclosing_range_when_projecting_then_call_edge_is_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    let mut index = sample_index_with_call_edge();
    if let Some(document) = index.documents.first_mut() {
        if let Some(definition) = document.occurrences.first_mut() {
            definition.enclosing_range.clear();
            definition.set_multi_line_enclosing_range(MultiLineRange {
                start_line: 0,
                start_character: 0,
                end_line: 3,
                end_character: 1,
                ..Default::default()
            });
        }
        if let Some(reference) = document.occurrences.get_mut(1) {
            reference.range.clear();
            reference.set_single_line_range(SingleLineRange {
                line: 1,
                start_character: 4,
                end_character: 18,
                ..Default::default()
            });
        }
    }
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.call_edges.len());
    assert_eq!(SYMBOL, model.call_edges[0].enclosing_symbol_id.as_str());
    assert_eq!(PRIVATE_SYMBOL, model.call_edges[0].referenced_symbol_id.as_str());
    Ok(())
}

#[test]
fn given_legacy_rust_analyzer_signature_when_projecting_then_signature_text_is_recovered(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index_with_legacy_document_signature_text().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    let signature_text = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .map(|signature| signature.signature_text.as_str());
    assert_eq!(Some("fn public_sum(left: i32, right: i32) -> i32"), signature_text);
    Ok(())
}

#[test]
fn given_signature_occurrence_when_projecting_then_return_type_scip_id_is_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index_with_signature_occurrence().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    let return_type = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.return_type.as_ref());
    assert_eq!(Some(RETURN_SYMBOL), return_type.and_then(|return_type| return_type.scip_symbol.as_deref()));
    assert_eq!(Some("scip_symbol"), return_type.map(|return_type| return_type.confidence.as_str()));
    Ok(())
}

#[test]
fn given_unknown_extension_when_format_is_not_explicit_then_typed_error_is_returned(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.data");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;

    let error = project_scip_index(&path, None).err();

    assert!(matches!(error, Some(ScipError::UnknownFormat { .. })));
    Ok(())
}

#[tokio::test]
async fn given_missing_rust_analyzer_when_generating_then_typed_error_is_returned(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let output_path = directory.path().join("missing.scip");
    let missing_path = directory.path().join("missing-rust-analyzer");

    let error = generate_rust_scip(
        directory.path(),
        &output_path,
        Some(&missing_path),
        None,
        false,
        None,
    )
    .await
    .err();

    assert!(matches!(error, Some(ScipError::RustAnalyzerMissing { .. })));
    Ok(())
}

#[tokio::test]
async fn given_non_not_found_launch_failure_when_generating_then_typed_launch_error_is_returned(
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let directory = tempdir()?;
        let output_path = directory.path().join("permission.scip");
        let script_path = directory.path().join("ra-not-executable");
        std::fs::write(&script_path, "#!/bin/sh\nexit 0\n")?;

        let error = generate_rust_scip(
            directory.path(),
            &output_path,
            Some(&script_path),
            None,
            false,
            None,
        )
        .await
        .err();

        assert!(matches!(error, Some(ScipError::RustAnalyzerLaunchFailed { .. })));
    }
    Ok(())
}

#[tokio::test]
async fn given_nonzero_rust_analyzer_when_generating_then_stderr_is_captured(
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir()?;
        let script_path = directory.path().join("ra-fails");
        std::fs::write(&script_path, "#!/bin/sh\necho failed >&2\nexit 7\n")?;
        let mut permissions = std::fs::metadata(&script_path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions)?;
        let output_path = directory.path().join("failed.scip");

        let error = generate_rust_scip(
            directory.path(),
            &output_path,
            Some(&script_path),
            None,
            false,
            None,
        )
        .await
        .err();

        assert!(matches!(
            error,
            Some(ScipError::RustAnalyzerFailed { stderr, .. }) if stderr.contains("failed")
        ));
    }
    Ok(())
}

#[tokio::test]
async fn given_successful_rust_analyzer_when_writing_stderr_then_diagnostics_are_returned(
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir()?;
        let script_path = directory.path().join("ra-diagnostics");
        std::fs::write(
            &script_path,
            "#!/bin/sh\necho indexed >&2\nprintf '' > \"$4\"\nexit 0\n",
        )?;
        let mut permissions = std::fs::metadata(&script_path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions)?;
        let output_path = directory.path().join("generated.scip");

        let stderr = generate_rust_scip(
            directory.path(),
            &output_path,
            Some(&script_path),
            None,
            false,
            None,
        )
        .await?;

        assert!(stderr.contains("indexed"));
        assert!(output_path.exists());
    }
    Ok(())
}

fn sample_index() -> Index {
    Index {
        metadata: MessageField::some(Metadata {
            project_root: "fixture".to_owned(),
            tool_info: MessageField::some(ToolInfo {
                name: "rust-analyzer".to_owned(),
                version: "test".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        documents: vec![Document {
            relative_path: "src/lib.rs".to_owned(),
            language: "rust".to_owned(),
            symbols: vec![SymbolInformation {
                symbol: SYMBOL.to_owned(),
                kind: symbol_information::Kind::Function.into(),
                display_name: "public_sum".to_owned(),
                ..Default::default()
            }],
            occurrences: vec![Occurrence {
                range: vec![0, 0, 10],
                symbol: SYMBOL.to_owned(),
                symbol_roles: SymbolRole::Definition as i32,
                enclosing_range: vec![0, 0, 3, 1],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn sample_index_with_call_edge() -> Index {
    let mut index = sample_index();
    if let Some(document) = index.documents.first_mut() {
        document.symbols.push(SymbolInformation {
            symbol: PRIVATE_SYMBOL.to_owned(),
            kind: symbol_information::Kind::Function.into(),
            display_name: "private_offset".to_owned(),
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![1, 4, 18],
            symbol: PRIVATE_SYMBOL.to_owned(),
            symbol_roles: 0,
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![4, 0, 14],
            symbol: PRIVATE_SYMBOL.to_owned(),
            symbol_roles: SymbolRole::Definition as i32,
            enclosing_range: vec![4, 0, 6, 1],
            ..Default::default()
        });
    }
    index
}

fn sample_index_with_legacy_document_signature_text() -> Index {
    let mut index = sample_index();
    let mut signature = Signature::default();
    signature.text = "fn public_sum(left: i32, right: i32) -> i32".to_owned();
    if let Some(document) = index.documents.first_mut() {
        if let Some(symbol) = document.symbols.first_mut() {
            symbol.signature_documentation = MessageField::some(signature);
        }
    }
    index
}

fn sample_index_with_signature_occurrence() -> Index {
    let mut index = sample_index();
    let mut signature = Signature::default();
    signature.language = "rust".to_owned();
    signature.text = "fn public_sum(left: i32, right: i32) -> i32".to_owned();
    signature.occurrences.push(Occurrence {
        range: vec![0, 40, 43],
        symbol: RETURN_SYMBOL.to_owned(),
        ..Default::default()
    });
    if let Some(document) = index.documents.first_mut() {
        if let Some(symbol) = document.symbols.first_mut() {
            symbol.signature_documentation = MessageField::some(signature);
        }
    }
    index
}
```

```rust
// crates/refactor-radar-report/tests/brief_tests.rs
use refactor_radar_core::{
    CallEdgeSummary, ElementKind, ElementSummary, FunctionParameterSummary,
    FunctionSignatureSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary, TypeReferenceSummary,
};
use refactor_radar_report::{build_element_brief, build_project_summary, generate_llm_brief};

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

    assert!(brief.five_w.what_summary.iter().any(|entry| entry.contains("Ready")));
    assert!(brief
        .five_w
        .where_summary
        .iter()
        .any(|entry| entry.contains("src/lib.rs (1)")));
    assert!(!brief
        .unknown
        .iter()
        .any(|entry| entry.contains("return type SCIP symbol")));
    Ok(())
}

#[test]
fn given_function_element_when_building_brief_then_signature_and_function_like_references_are_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(brief
        .five_w
        .what_summary
        .iter()
        .any(|entry| entry.contains("signature: fn parse_record(input: &str) -> Result<Record, FixtureError>")));
    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("SCIP-derived outgoing function_like_reference")));
    assert!(brief
        .five_w
        .when_summary
        .iter()
        .any(|entry| entry.contains("not available from SCIP")));
    assert!(brief
        .confirmed
        .iter()
        .any(|entry| entry.contains("signature text came from SCIP")));
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
        role_flags: 32,
    });

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(brief
        .five_w
        .how
        .iter()
        .any(|entry| entry.contains("test reference at tests/parse_record.rs")));
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
            signature_text: "fn parse_record(input: &str) -> Record".to_owned(),
            parameters: vec![FunctionParameterSummary {
                display_name: "input".to_owned(),
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
        .what_summary
        .iter()
        .any(|entry| entry.contains(PARAM_SYMBOL)));
    assert!(brief
        .five_w
        .what_summary
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
    let model = sample_model()?;

    let brief = generate_llm_brief(&model, Some(125))?;

    assert!(brief.len() <= 500);
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
                    signature_text: "fn parse_record(input: &str) -> Result<Record, FixtureError>".to_owned(),
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
            role_flags: 0,
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

```rust
// crates/refactor-radar-mcp/src/refactor_radar_mcp_server.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        element_query_params::ElementQueryParams,
        find_references_params::FindReferencesParams,
        find_related_symbols_params::FindRelatedSymbolsParams,
        generate_llm_brief_params::GenerateLlmBriefParams,
        generate_rust_scip_params::GenerateRustScipParams,
        get_5w_summary_params::Get5wSummaryParams,
        get_element_brief_params::GetElementBriefParams,
        get_element_params::GetElementParams,
        get_function_signature_params::GetFunctionSignatureParams,
        get_project_summary_params::GetProjectSummaryParams,
        get_source_span_params::GetSourceSpanParams,
        load_scip_project_params::LoadScipProjectParams,
        scan_project_params::ScanProjectParams,
    };
    use protobuf::{Message, MessageField};
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use rmcp::model::CallToolResult;
    use scip::types::{
        symbol_information, Document, Index, Metadata, Occurrence, Signature, SymbolInformation,
        SymbolRole, ToolInfo,
    };
    use tempfile::tempdir;

    const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
    const PRIVATE_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
    const EXTERNAL_SYMBOL: &str = "rust-analyzer cargo std 0.0.0 std/option/Option#";

    #[tokio::test]
    async fn given_scip_file_when_loading_project_then_cache_contains_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let (_directory, path) = write_fixture_scip()?;

        let Json(result) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;

        assert_eq!("fixture", result.project_id);
        assert_eq!(1, result.document_count);
        assert!(server.cache.read().await.models.contains_key("fixture"));
        Ok(())
    }

    #[tokio::test]
    async fn given_missing_project_when_getting_summary_then_mcp_error_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();

        let error = server
            .get_project_summary(Parameters(GetProjectSummaryParams {
                project_id: "missing".to_owned(),
            }))
            .await
            .err();

        assert!(error.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn given_fake_rust_analyzer_when_generating_scip_then_result_metadata_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let fixture_path = directory.path().join("fixture.scip");
            std::fs::write(&fixture_path, sample_index().write_to_bytes()?)?;
            let output_path = directory.path().join("generated.scip");
            let script_path = directory.path().join("ra-generate");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho generated >&2\ncat \"$2/fixture.scip\" > \"$4\"\nexit 0\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let Json(result) = server
                .generate_rust_scip(Parameters(GenerateRustScipParams {
                    path: directory.path().display().to_string(),
                    output_path: output_path.display().to_string(),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                    exclude_vendored_libraries: Some(false),
                    num_threads: None,
                }))
                .await
                .map_err(to_io_error)?;

            assert_eq!(output_path.display().to_string(), result.path);
            assert!(result.stderr_digest.contains("generated"));
            assert!(result.file_size > 0);
            assert_eq!(Some("rust-analyzer"), result.producer_name.as_deref());
            assert_eq!(Some("test"), result.producer_version.as_deref());
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_fake_rust_analyzer_when_scanning_project_then_cache_contains_project(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let fixture_path = directory.path().join("fixture.scip");
            std::fs::write(&fixture_path, sample_index().write_to_bytes()?)?;
            let output_path = directory.path().join("generated.scip");
            let script_path = directory.path().join("ra-success");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho scanned >&2\ncat \"$2/fixture.scip\" > \"$4\"\nexit 0\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let Json(result) = server
                .scan_project(Parameters(ScanProjectParams {
                    path: directory.path().display().to_string(),
                    output_path: Some(output_path.display().to_string()),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                }))
                .await
                .map_err(to_io_error)?;

            assert_eq!("fixture", result.project_id);
            assert!(output_path.exists());
            assert!(server.cache.read().await.models.contains_key("fixture"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_failing_rust_analyzer_when_scanning_project_then_stderr_is_structured_error_data(
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let server = RefactorRadarMcpServer::new();
            let directory = tempdir()?;
            let output_path = directory.path().join("failed.scip");
            let script_path = directory.path().join("ra-fails");
            std::fs::write(
                &script_path,
                "#!/bin/sh\necho scan failed >&2\nexit 7\n",
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script_path, permissions)?;

            let error = server
                .scan_project(Parameters(ScanProjectParams {
                    path: directory.path().display().to_string(),
                    output_path: Some(output_path.display().to_string()),
                    rust_analyzer_path: Some(script_path.display().to_string()),
                    config_path: None,
                }))
                .await
                .err();
            let data = error.and_then(|error| error.data);
            let stderr = data
                .as_ref()
                .and_then(|value| value.get("stderr"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();

            assert!(stderr.contains("scan failed"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_summary_then_structured_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_project_summary(Parameters(GetProjectSummaryParams {
                project_id: "fixture".to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some("fixture"), value.get("project_id").and_then(|value| value.as_str()));
        assert_eq!(Some(1), value.get("document_count").and_then(|value| value.as_u64()));
        assert_eq!(Some(2), value.get("element_count").and_then(|value| value.as_u64()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_listing_elements_then_kind_path_and_limit_filters_work(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .list_elements(Parameters(ElementQueryParams {
                project_id: "fixture".to_owned(),
                kind: Some("function".to_owned()),
                path: Some("src/lib.rs".to_owned()),
                limit: Some(1),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some(1), value.as_array().map(|items| items.len()));
        assert_eq!(
            Some("public_sum"),
            value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item.get("display_name"))
                .and_then(|value| value.as_str())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_element_then_stable_id_or_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let by_scip = server
            .get_element(Parameters(GetElementParams {
                symbol_id: SYMBOL.to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .get_element(Parameters(GetElementParams {
                symbol_id: format!("scip:{SYMBOL}"),
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(SYMBOL), by_scip.get("scip_symbol").and_then(|value| value.as_str()));
        assert_eq!(Some(SYMBOL), by_stable.get("scip_symbol").and_then(|value| value.as_str()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_function_when_getting_signature_then_signature_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_function_signature(Parameters(GetFunctionSignatureParams {
                symbol_id: SYMBOL.to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some(SYMBOL), value.get("symbol_id").and_then(|value| value.as_str()));
        assert_eq!(
            Some("fn public_sum(left: i32, right: i32) -> i32"),
            value
                .get("signature")
                .and_then(|signature| signature.get("signature_text"))
                .and_then(|value| value.as_str())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_function_when_getting_element_brief_then_five_w_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let brief = server
            .get_element_brief(Parameters(GetElementBriefParams {
                symbol_id: SYMBOL.to_owned(),
                include_inferred: Some(true),
                reference_limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let five_w = server
            .get_5w_summary(Parameters(Get5wSummaryParams {
                symbol_id: SYMBOL.to_owned(),
                reference_limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let brief = require_structured(brief)?;
        let five_w = require_structured(five_w)?;

        assert!(brief.get("five_w").and_then(|value| value.get("what")).is_some());
        assert!(brief.get("evidence").is_some());
        assert!(five_w.get("five_w").and_then(|value| value.get("what")).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_references_then_stable_id_or_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let by_scip = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: PRIVATE_SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: format!("scip:{PRIVATE_SYMBOL}"),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(1), by_scip.as_array().map(|items| items.len()));
        assert_eq!(Some(1), by_stable.as_array().map(|items| items.len()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_external_references_then_raw_scip_id_is_accepted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;
        let external_span = refactor_radar_core::SourceSpan::from_scip_range(
            "fixture",
            "src/lib.rs",
            &[8, 2, 8],
        )?;
        {
            let mut cache = server.cache.write().await;
            let model = cache
                .models
                .get_mut("fixture")
                .ok_or_else(|| std::io::Error::other("missing fixture model"))?;
            model.references.push(refactor_radar_core::SymbolReferenceSummary {
                referenced_symbol_id: refactor_radar_core::SymbolId::new(EXTERNAL_SYMBOL),
                source_span: external_span.clone(),
                document_path: "src/lib.rs".to_owned(),
                role_flags: 0,
            });
            model.spans.push(external_span);
        }

        let by_scip = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: EXTERNAL_SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_stable = server
            .find_references(Parameters(FindReferencesParams {
                symbol_id: format!("scip:{EXTERNAL_SYMBOL}"),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let by_scip = require_structured(by_scip)?;
        let by_stable = require_structured(by_stable)?;

        assert_eq!(Some(1), by_scip.as_array().map(|items| items.len()));
        assert_eq!(Some(1), by_stable.as_array().map(|items| items.len()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_finding_related_symbols_then_function_like_relations_are_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .find_related_symbols(Parameters(FindRelatedSymbolsParams {
                symbol_id: SYMBOL.to_owned(),
                limit: Some(10),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(
            Some(1),
            value
                .get("outgoing_function_like_references")
                .and_then(|value| value.as_array())
                .map(|items| items.len())
        );
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_getting_source_span_then_span_metadata_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .get_source_span(Parameters(GetSourceSpanParams {
                span_id: "fixture::src/lib.rs:1:1".to_owned(),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;

        assert_eq!(Some("src/lib.rs"), value.get("document_path").and_then(|value| value.as_str()));
        assert_eq!(Some(1), value.get("start_line").and_then(|value| value.as_u64()));
        Ok(())
    }

    #[tokio::test]
    async fn given_cached_project_when_generating_llm_brief_then_markdown_result_is_returned(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = load_sample_server().await?;

        let result = server
            .generate_llm_brief(Parameters(GenerateLlmBriefParams {
                project_id: "fixture".to_owned(),
                budget_tokens: Some(100),
            }))
            .await
            .map_err(to_io_error)?;
        let value = require_structured(result)?;
        let markdown = value
            .get("markdown")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        assert!(markdown.contains("# Project fixture"));
        Ok(())
    }

    async fn load_sample_server() -> Result<RefactorRadarMcpServer, Box<dyn std::error::Error>> {
        let server = RefactorRadarMcpServer::new();
        let (_directory, path) = write_fixture_scip()?;
        let Json(_) = server
            .load_scip_project(Parameters(LoadScipProjectParams {
                path: path.display().to_string(),
                format: None,
            }))
            .await
            .map_err(to_io_error)?;
        Ok(server)
    }

    fn write_fixture_scip(
    ) -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("fixture.scip");
        std::fs::write(&path, sample_index().write_to_bytes()?)?;
        Ok((directory, path))
    }

    fn sample_index() -> Index {
        Index {
            metadata: MessageField::some(Metadata {
                project_root: "fixture".to_owned(),
                tool_info: MessageField::some(ToolInfo {
                    name: "rust-analyzer".to_owned(),
                    version: "test".to_owned(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            documents: vec![Document {
                relative_path: "src/lib.rs".to_owned(),
                language: "rust".to_owned(),
                symbols: vec![
                    SymbolInformation {
                        symbol: SYMBOL.to_owned(),
                        kind: symbol_information::Kind::Function.into(),
                        display_name: "public_sum".to_owned(),
                        signature_documentation: MessageField::some(Signature {
                            language: "rust".to_owned(),
                            text: "fn public_sum(left: i32, right: i32) -> i32".to_owned(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    SymbolInformation {
                        symbol: PRIVATE_SYMBOL.to_owned(),
                        kind: symbol_information::Kind::Function.into(),
                        display_name: "private_offset".to_owned(),
                        ..Default::default()
                    },
                ],
                occurrences: vec![
                    Occurrence {
                        range: vec![0, 0, 10],
                        symbol: SYMBOL.to_owned(),
                        symbol_roles: SymbolRole::Definition as i32,
                        enclosing_range: vec![0, 0, 3, 1],
                        ..Default::default()
                    },
                    Occurrence {
                        range: vec![1, 4, 18],
                        symbol: PRIVATE_SYMBOL.to_owned(),
                        symbol_roles: 0,
                        ..Default::default()
                    },
                    Occurrence {
                        range: vec![4, 0, 14],
                        symbol: PRIVATE_SYMBOL.to_owned(),
                        symbol_roles: SymbolRole::Definition as i32,
                        enclosing_range: vec![4, 0, 6, 1],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn require_structured(
        result: CallToolResult,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        match result.structured_content {
            Some(value) => Ok(value),
            None => Err(Box::new(std::io::Error::other("missing structured MCP result"))),
        }
    }

    fn to_io_error(error: rmcp::ErrorData) -> std::io::Error {
        std::io::Error::other(format!("{error:?}"))
    }
}
```

## Phase 9: Verification Commands

Run formatting, tests, clippy, and build:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-core
cargo test -p refactor-radar-scip
cargo test -p refactor-radar-report
cargo test -p refactor-radar-mcp
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

Verify the one-type-per-file rule:

```bash
for file in $(rg -l '^(pub )?(struct|enum|trait|type) ' crates/refactor-radar-*/src); do
    count=$(rg -n '^(pub )?(struct|enum|trait|type) ' "$file" | wc -l)
    if [ "$count" -gt 1 ]; then
        echo "$file has $count named type definitions"
        exit 1
    fi
done
```

Verify the application/library error and MCP stdout rules:

```bash
if rg -n 'anyhow|unwrap\(|expect\(|panic!|dbg!' crates/refactor-radar-*; then
    echo "forbidden application/library error or debug pattern found"
    exit 1
else
    status=$?
    if [ "$status" -gt 1 ]; then
        exit "$status"
    fi
fi

if rg -n '\bprintln!' crates/refactor-radar-mcp; then
    echo "stdout write found in refactor-radar-mcp"
    exit 1
else
    status=$?
    if [ "$status" -gt 1 ]; then
        exit "$status"
    fi
fi
```

Expected results:

- No `anyhow` in RefactorRadar application/library crates.
- No `unwrap()`, `expect()`, `panic!`, or `dbg!`.
- No `println!` in `refactor-radar-mcp`.
- Any `eprintln!` is confined to the MCP binary entrypoint or explicit stderr
  diagnostics outside MCP JSON-RPC traffic.

## Acceptance Criteria

- Workspace contains the four RefactorRadar crates plus existing `crates/wip`.
- `refactor-radar-scip` can generate and load rust-analyzer SCIP indexes.
- `refactor-radar-scip` projects SCIP into a compact RefactorRadar semantic
  model with stable IDs, source spans, elements, and references.
- `refactor-radar-report` can generate project summaries, element briefs, and
  5W summaries from the projected model.
- Function briefs include signature facts with parameter SCIP IDs and return
  type SCIP IDs where SCIP provides enough data.
- 5W summaries explicitly separate confirmed facts, inference, and unknowns.
- MCP exposes compact SCIP-backed query tools without returning raw source by
  default.
- MCP responses preserve IDs for follow-up queries.
- SCIP-derived call/function relationship data is labeled as SCIP-derived and
  not overstated as exact AST call expressions.
- Runtime query operation is local-only and does not require network access.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo build --workspace` pass.

## Deferred Scope

- Full CLI implementation.
- Persistent `.refactor-radar/` artifact cache.
- Tree-sitter Rust syntax supplement for exact calls and metrics.
- Git history integration for real `when` and ownership signals.
- Workspace-wide cross-project analysis.
- Refactor candidate ranking.
- C# support.
- HTTP MCP transport.
- Agentic orchestration.
