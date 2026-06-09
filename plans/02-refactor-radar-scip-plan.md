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

Also verify that `refactor-radar-core` exports the exact model surface used by
the snippets below before creating this crate:

- `SemanticModel` with public `project`, `files`, `elements`, `references`,
  `call_edges`, and `spans` fields.
- `ProjectSummary`, `FileSummary`, `ElementSummary`,
  `SymbolReferenceSummary`, `CallEdgeSummary`, `FunctionParameterSummary`,
  `FunctionSignatureSummary`, and `TypeReferenceSummary` with the public fields
  constructed in this plan.
- `StableId::from_scip_symbol`, `SymbolId::new`, `SymbolId::as_str`, and
  `SourceSpan::from_scip_range`.
- `Language::Rust` and every `ElementKind` variant used by
  `symbol_kind_projection`, including `Unknown(String)`.

If any of that surface is missing or incompatible, stop and update the core
crate first; do not add duplicate model types or adapter shims in
`refactor-radar-scip`.

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

## Concrete Implementation Snippets

Use these snippets as the concrete starting implementation for the SCIP crate.
Function-only modules may have zero named type definitions; named type modules
must keep at most one named type definition.

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
```

```rust
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
        ScipKind::Attribute => ElementKind::Unknown("scip::Attribute".to_owned()),
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
        other => ElementKind::Unknown(format!("scip::{other:?}")),
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
                    .unwrap_or_else(|| ElementKind::Unknown("scip::UnspecifiedKind".to_owned())),
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
            let source_span =
                SourceSpan::from_scip_range(&project_id, &document.relative_path, &range)
                    .map_err(|_| ScipError::core_projection_failed())?;
            spans.push(source_span.clone());
            references.push(SymbolReferenceSummary {
                referenced_symbol_id: SymbolId::new(occurrence.symbol.clone()),
                source_span,
                document_path: document.relative_path.clone(),
                symbol_roles: occurrence.symbol_roles,
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
    let parameters = parameter_text_ranges(&signature_text)
        .into_iter()
        .map(|(parameter_start, parameter_end)| {
            let parameter_text = &signature_text[parameter_start..parameter_end];
            let display_name = parameter_display_name(parameter_text);
            let parameter_symbol =
                parameter_symbol_for(enclosing_symbol, &display_name, symbols, occurrences);
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
            let type_reference =
                parameter_type_range(&signature_text, parameter_start, parameter_end).map(
                    |(type_start, type_end)| {
                        type_reference_from_text(
                            &signature_text[type_start..type_end],
                            type_start,
                            type_end,
                            signature,
                        )
                    },
                );
            FunctionParameterSummary {
                parameter_kind: format!("{parameter_kind:?}"),
                name: display_name,
                scip_symbol: parameter_symbol.map(|symbol| symbol.symbol.clone()),
                source_span,
                type_reference,
            }
        })
        .collect::<ScipResult<Vec<_>>>()?;
    let return_type = return_type_range(&signature_text).map(|(type_start, type_end)| {
        type_reference_from_text(
            &signature_text[type_start..type_end],
            type_start,
            type_end,
            signature,
        )
    });
    let mut unknown = Vec::new();
    if signature.occurrences.is_empty() {
        unknown.push(
            "SCIP signature occurrences were unavailable; parameter and return types are text-only"
                .to_owned(),
        );
    }
    Ok(FunctionSignatureSummary {
        signature_text: Some(signature_text),
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
    occurrences: &[Occurrence],
) -> Option<&'a SymbolInformation> {
    symbols.iter().find(|symbol| {
        if !is_parameter_symbol(display_name, symbol) {
            return false;
        }
        symbol.enclosing_symbol == enclosing_symbol
            || parameter_definition_inside_enclosing_symbol(
                &symbol.symbol,
                enclosing_symbol,
                occurrences,
            )
    })
}

fn is_parameter_symbol(display_name: &str, symbol: &SymbolInformation) -> bool {
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
}

fn parameter_definition_inside_enclosing_symbol(
    parameter_symbol: &str,
    enclosing_symbol: &str,
    occurrences: &[Occurrence],
) -> bool {
    let Some(enclosing_range) = occurrences
        .iter()
        .find(|occurrence| {
            occurrence.symbol == enclosing_symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
        })
        .and_then(occurrence_enclosing_range)
    else {
        return false;
    };
    occurrences.iter().any(|occurrence| {
        occurrence.symbol == parameter_symbol
            && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
            && occurrence_range(occurrence)
                .is_some_and(|range| range_contains(&enclosing_range, &range))
    })
}

fn is_self_parameter(display_name: &str) -> bool {
    display_name == "self"
}

fn parameter_text_ranges(signature_text: &str) -> Vec<(usize, usize)> {
    let Some(open_index) = signature_text.find('(') else {
        return Vec::new();
    };
    let Some(close_index) = matching_close_paren(signature_text, open_index) else {
        return Vec::new();
    };
    split_top_level_commas_with_offsets(signature_text, open_index + 1, close_index)
        .into_iter()
        .filter_map(|(start, end)| trimmed_range(signature_text, start, end))
        .collect()
}

fn parameter_type_range(
    signature_text: &str,
    parameter_start: usize,
    parameter_end: usize,
) -> Option<(usize, usize)> {
    let parameter_text = signature_text.get(parameter_start..parameter_end)?;
    let colon_index = parameter_text.find(':')?;
    trimmed_range(
        signature_text,
        parameter_start + colon_index + ':'.len_utf8(),
        parameter_end,
    )
}

fn return_type_range(signature_text: &str) -> Option<(usize, usize)> {
    let open_index = signature_text.find('(')?;
    let close_index = matching_close_paren(signature_text, open_index)?;
    let after_params = signature_text.get(close_index + 1..)?;
    let arrow_index = after_params.find("->")?;
    trimmed_range(
        signature_text,
        close_index + 1 + arrow_index + "->".len(),
        signature_text.len(),
    )
}

fn trimmed_range(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let slice = text.get(start..end)?;
    let trimmed_start = start + slice.len() - slice.trim_start().len();
    let trimmed_end = end - (slice.len() - slice.trim_end().len());
    (trimmed_start < trimmed_end).then_some((trimmed_start, trimmed_end))
}

fn type_reference_from_text(
    type_text: &str,
    text_start: usize,
    text_end: usize,
    signature: &Signature,
) -> TypeReferenceSummary {
    let occurrence = occurrence_in_text_range(signature, text_start, text_end);
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

fn occurrence_in_text_range<'a>(
    signature: &'a Signature,
    text_start: usize,
    text_end: usize,
) -> Option<&'a Occurrence> {
    signature.occurrences.iter().find(|occurrence| {
        if occurrence.symbol.is_empty() {
            return false;
        }
        let Some(range) = occurrence_range(occurrence) else {
            return false;
        };
        let Some((start_line, start_column, end_line, end_column)) = range_bounds(&range) else {
            return false;
        };
        if start_line != 0 || end_line != 0 {
            return false;
        }
        let Ok(start_column) = usize::try_from(start_column) else {
            return false;
        };
        let Ok(end_column) = usize::try_from(end_column) else {
            return false;
        };
        text_start <= start_column && end_column <= text_end
    })
}

fn signature_range_from_scip(range: &[i32]) -> Option<Vec<u32>> {
    let (start_line, start_column, end_line, end_column) = range_bounds(range)?;
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

fn split_top_level_commas_with_offsets(
    text: &str,
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    let Some(slice) = text.get(start..end) else {
        return parts;
    };
    for (relative_index, character) in slice.char_indices() {
        let index = start + relative_index;
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                parts.push((part_start, index));
                part_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push((part_start, end));
    parts
}
```

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
- Parameter SCIP symbols are linked from enclosing-symbol or definition evidence.
- Parameter and return-type SCIP IDs are preserved when
  `Signature.occurrences` provides them.
- Missing rust-analyzer maps to `RustAnalyzerMissing`.
- Non-NotFound launch failures map to `RustAnalyzerLaunchFailed`.
- Nonzero rust-analyzer exit captures stderr in `RustAnalyzerFailed`.
- Successful rust-analyzer stderr diagnostics are returned, not printed.

Use in-memory SCIP fixtures with `scip::types::Index` and fake unix shell
scripts for rust-analyzer subprocess behavior. Do not require a real
rust-analyzer binary for unit tests.

Concrete starting test file:

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
const PARAMETER_SYMBOL: &str = "local 0";
const PARAMETER_TYPE_SYMBOL: &str = "rust-analyzer cargo other 0.0.0 primitive/i32#";
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
    assert_eq!(0, model.references[0].symbol_roles);
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
    let index = sample_index_with_legacy_document_signature_text()?;
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    let signature_text = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.signature_text.as_deref());
    assert_eq!(Some("fn public_sum(left: i32, right: i32) -> i32"), signature_text);
    Ok(())
}

#[test]
fn given_signature_occurrence_when_projecting_then_parameter_and_return_type_scip_ids_are_preserved(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index_with_signature_occurrence().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    let first_parameter = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.parameters.first());
    assert_eq!(
        Some(PARAMETER_SYMBOL),
        first_parameter.and_then(|parameter| parameter.scip_symbol.as_deref())
    );
    assert!(first_parameter.and_then(|parameter| parameter.source_span.as_ref()).is_some());
    let parameter_type = first_parameter.and_then(|parameter| parameter.type_reference.as_ref());
    assert_eq!(
        Some(PARAMETER_TYPE_SYMBOL),
        parameter_type.and_then(|type_reference| type_reference.scip_symbol.as_deref())
    );
    assert_eq!(
        Some("scip_symbol"),
        parameter_type.map(|type_reference| type_reference.confidence.as_str())
    );
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

fn sample_index_with_legacy_document_signature_text(
) -> Result<Index, Box<dyn std::error::Error>> {
    let mut index = sample_index();
    let legacy_document = Document {
        language: "rust".to_owned(),
        text: "fn public_sum(left: i32, right: i32) -> i32".to_owned(),
        ..Default::default()
    };
    let signature = Signature::parse_from_bytes(&legacy_document.write_to_bytes()?)?;
    if let Some(document) = index.documents.first_mut() {
        if let Some(symbol) = document.symbols.first_mut() {
            symbol.signature_documentation = MessageField::some(signature);
        }
    }
    Ok(index)
}

fn sample_index_with_signature_occurrence() -> Index {
    let mut index = sample_index();
    let mut signature = Signature::default();
    signature.language = "rust".to_owned();
    signature.text = "fn public_sum(left: i32, right: i32) -> i32".to_owned();
    signature.occurrences.push(Occurrence {
        range: vec![0, 20, 23],
        symbol: PARAMETER_TYPE_SYMBOL.to_owned(),
        ..Default::default()
    });
    signature.occurrences.push(Occurrence {
        range: vec![0, 40, 43],
        symbol: RETURN_SYMBOL.to_owned(),
        ..Default::default()
    });
    if let Some(document) = index.documents.first_mut() {
        document.symbols.push(SymbolInformation {
            symbol: PARAMETER_SYMBOL.to_owned(),
            kind: symbol_information::Kind::Parameter.into(),
            display_name: "left".to_owned(),
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![0, 14, 18],
            symbol: PARAMETER_SYMBOL.to_owned(),
            symbol_roles: SymbolRole::Definition as i32,
            ..Default::default()
        });
        if let Some(symbol) = document.symbols.first_mut() {
            symbol.signature_documentation = MessageField::some(signature);
        }
    }
    index
}
```

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
