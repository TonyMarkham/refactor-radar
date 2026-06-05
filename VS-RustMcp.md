# Plan: Rust MCP Server Implementation

## Objective

Implement a local stdio MCP server for RefactorRadar that exposes compact Rust
semantic scan facts to MCP clients.

The repository is currently a seed Rust workspace with `crates/wip` and the
ADR/PRD. This plan therefore includes the minimum prerequisite library crates
needed by the MCP server:

- `crates/refactor-radar-core`
- `crates/refactor-radar-rust`
- `crates/refactor-radar-report`
- `crates/refactor-radar-mcp`

Use the ADR's `crates/refactor-radar-*` layout instead of `crates/core`,
`crates/scanners/rust`, `crates/report`, or `crates/mcp/rust`. The MCP crate is
named `refactor-radar-mcp`; it supports Rust only for the first implementation.

## Hard Constraints

- Follow `AGENTS.md` for every Rust crate.
- Do not use `anyhow` for application or library errors.
- Every crate must define a crate-local typed error enum and result alias.
- Error variants must carry `location: ErrorLocation`.
- Public error constructor functions must be `#[track_caller]` and attach
  `ErrorLocation::from(Location::caller())`.
- Stable plain messages must be available through `message()` when MCP or
  report output needs text without source-location detail.
- Use exactly one named type definition per Rust source file.
  - `struct`, `enum`, `trait`, and `type` aliases each count as a named type.
  - Put result aliases in their own files, such as `core_result.rs`.
  - `impl` blocks may live in the same file as their type.
  - `lib.rs`, `mod.rs`, and function-only files should not define named types.
- Keep `crates/wip` untouched unless a later plan explicitly removes it.
- The MCP stdio server must never write logs or diagnostics to stdout. Stdout is
  reserved for MCP JSON-RPC traffic.
- The tool must run locally without network access at runtime.

## Repo Facts To Respect

- Root `Cargo.toml` currently has one workspace member: `crates/wip`.
- Root lints deny `unwrap_used`, `expect_used`, `panic`, and unused must-use
  values.
- Root workspace dependencies already include `error-location`, `thiserror`,
  and `soul-attributes`.
- The ADR states "CLI first, MCP later", but this plan is explicitly for the
  Rust MCP server. The plan builds the core scanner/report prerequisites needed
  by MCP and leaves a full CLI implementation to a separate plan.
- The ADR's initial MCP tools are:
  - `scan_project(path)`
  - `get_project_summary(project_id)`
  - `list_symbols(project_id)`
  - `get_symbol(symbol_id)`
  - `get_source_span(span_id)`
  - `generate_llm_brief(project_id, budget_tokens)`

## Target Architecture

```text
MCP client / coding agent
    |
    v
crates/refactor-radar-mcp
    stdio MCP server, rmcp tool definitions, cache, query handlers
    |
    v
crates/refactor-radar-report
    project summaries and LLM brief rendering
    |
    v
crates/refactor-radar-rust
    Rust semantic extraction with tree-sitter and ignore
    |
    v
crates/refactor-radar-core
    semantic model, source spans, stable IDs, serialization
```

`tree-sitter-rust` is only the Rust parser dependency. It is not the MCP server.

## Phase 0: Bootstrap Commands

Run these from the repository root:

```bash
git status --short
cargo metadata --no-deps --format-version=1

cargo new --lib crates/refactor-radar-core --name refactor-radar-core --vcs none
cargo new --lib crates/refactor-radar-rust --name refactor-radar-rust --vcs none
cargo new --lib crates/refactor-radar-report --name refactor-radar-report --vcs none
cargo new --lib crates/refactor-radar-mcp --name refactor-radar-mcp --vcs none

mkdir -p crates/refactor-radar-mcp/src/bin
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
    "crates/refactor-radar-rust",
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
ignore                          = { version = "0.4.24" }
rmcp                            = { version = "1.7.0", default-features = false, features = ["server", "macros", "schemars", "transport-io"] }
schemars                        = { version = "1.1", features = ["derive"] }
serde                           = { version = "1.0", features = ["derive"] }
serde_json                      = { version = "1.0" }
soul-attributes                 = { version = "0.1.1" }
tempfile                        = { version = "3.23.0" }
thiserror                       = { version = "2.0.18" }
tokio                           = { version = "1", features = ["macros", "rt-multi-thread", "io-std", "io-util", "sync"] }
tree-sitter                     = { version = "0.26.9" }
tree-sitter-rust                = { version = "0.24.2" }

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"

[workspace.lints.rust]
unused_must_use = "deny"
```

`crates/refactor-radar-core/Cargo.toml`:

```toml
[package]
name = "refactor-radar-core"
version.workspace = true
edition.workspace = true
repository.workspace = true

[dependencies]
error-location.workspace = true
schemars.workspace = true
serde.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_json.workspace = true

[lints]
workspace = true
```

`crates/refactor-radar-rust/Cargo.toml`:

```toml
[package]
name = "refactor-radar-rust"
version.workspace = true
edition.workspace = true
repository.workspace = true

[dependencies]
error-location.workspace = true
ignore.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
thiserror.workspace = true
tree-sitter.workspace = true
tree-sitter-rust.workspace = true

[dev-dependencies]
tempfile.workspace = true

[lints]
workspace = true
```

`crates/refactor-radar-report/Cargo.toml`:

```toml
[package]
name = "refactor-radar-report"
version.workspace = true
edition.workspace = true
repository.workspace = true

[dependencies]
error-location.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
thiserror.workspace = true

[lints]
workspace = true
```

`crates/refactor-radar-mcp/Cargo.toml`:

```toml
[package]
name = "refactor-radar-mcp"
version.workspace = true
edition.workspace = true
repository.workspace = true

[dependencies]
error-location.workspace = true
refactor-radar-core = { path = "../refactor-radar-core" }
refactor-radar-report = { path = "../refactor-radar-report" }
refactor-radar-rust = { path = "../refactor-radar-rust" }
rmcp.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true

[dev-dependencies]
tempfile.workspace = true

[[bin]]
name = "refactor-radar-mcp"
path = "src/bin/refactor-radar-mcp.rs"

[lints]
workspace = true
```

## Phase 2: Core Model Crate

Create one file per named type. Do not collapse these into `models.rs`.

Recommended `crates/refactor-radar-core/src` layout:

```text
lib.rs
call_summary.rs
core_error.rs
core_result.rs
file_summary.rs
language.rs
metric_summary.rs
module_summary.rs
project_summary.rs
semantic_model.rs
signature_summary.rs
source_span.rs
span_id.rs
stable_id.rs
symbol_id.rs
symbol_kind.rs
symbol_summary.rs
test_summary.rs
visibility.rs
```

`lib.rs`:

```rust
pub mod call_summary;
pub mod core_error;
pub mod core_result;
pub mod file_summary;
pub mod language;
pub mod metric_summary;
pub mod module_summary;
pub mod project_summary;
pub mod semantic_model;
pub mod signature_summary;
pub mod source_span;
pub mod span_id;
pub mod stable_id;
pub mod symbol_id;
pub mod symbol_kind;
pub mod symbol_summary;
pub mod test_summary;
pub mod visibility;

pub use call_summary::CallSummary;
pub use core_error::CoreError;
pub use core_result::CoreResult;
pub use file_summary::FileSummary;
pub use language::Language;
pub use metric_summary::MetricSummary;
pub use module_summary::ModuleSummary;
pub use project_summary::ProjectSummary;
pub use semantic_model::SemanticModel;
pub use signature_summary::SignatureSummary;
pub use source_span::SourceSpan;
pub use span_id::SpanId;
pub use stable_id::StableId;
pub use symbol_id::SymbolId;
pub use symbol_kind::SymbolKind;
pub use symbol_summary::SymbolSummary;
pub use test_summary::TestSummary;
pub use visibility::Visibility;
```

`core_error.rs`:

```rust
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{message}")]
    InvalidStableId {
        message: String,
        location: ErrorLocation,
    },
}

impl CoreError {
    #[track_caller]
    pub fn invalid_stable_id(value: impl Into<String>) -> Self {
        let value = value.into();
        Self::InvalidStableId {
            message: format!("invalid stable id: {value}"),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidStableId { message, .. } => message,
        }
    }

    pub fn location(&self) -> &ErrorLocation {
        match self {
            Self::InvalidStableId { location, .. } => location,
        }
    }
}
```

`core_result.rs`:

```rust
use crate::CoreError;

pub type CoreResult<T> = std::result::Result<T, CoreError>;
```

`language.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
        }
    }
}
```

`symbol_kind.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Enum,
    EnumVariant,
    Function,
    Impl,
    Method,
    Module,
    Struct,
    Trait,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::EnumVariant => "enum_variant",
            Self::Function => "function",
            Self::Impl => "impl",
            Self::Method => "method",
            Self::Module => "module",
            Self::Struct => "struct",
            Self::Trait => "trait",
        }
    }
}
```

`stable_id.rs`:

```rust
use crate::{CoreResult, Language, SymbolKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct StableId(String);

impl StableId {
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(crate::CoreError::invalid_stable_id(value));
        }
        Ok(Self(value))
    }

    pub fn symbol(
        language: Language,
        project: &str,
        path: &str,
        kind: SymbolKind,
        qualified_name: &str,
    ) -> CoreResult<Self> {
        Self::new(format!(
            "{}:{}:{}:{}:{}",
            language.as_str(),
            normalize_component(project),
            normalize_path(path),
            kind.as_str(),
            normalize_component(qualified_name)
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn normalize_component(value: &str) -> String {
    value
        .trim()
        .replace("::", ".")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/') {
                character
            } else {
                '-'
            }
        })
        .collect()
}
```

`source_span.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SourceSpan {
    pub id: String,
    pub path: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl SourceSpan {
    pub fn from_zero_based(
        id: impl Into<String>,
        path: impl Into<String>,
        start_row: usize,
        start_column: usize,
        end_row: usize,
        end_column: usize,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            start_line: start_row.saturating_add(1),
            start_column: start_column.saturating_add(1),
            end_line: end_row.saturating_add(1),
            end_column: end_column.saturating_add(1),
        }
    }
}
```

`visibility.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}
```

`signature_summary.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SignatureSummary {
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
}
```

`call_summary.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CallSummary {
    pub callee_name: String,
}
```

`file_summary.rs`:

```rust
use crate::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FileSummary {
    pub path: String,
    pub language: Language,
    pub line_count: usize,
}
```

`project_summary.rs`:

```rust
use crate::Language;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectSummary {
    pub project_id: String,
    pub language: Language,
    pub file_count: usize,
    pub symbol_count: usize,
    pub test_count: usize,
}

impl Default for ProjectSummary {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            language: Language::Rust,
            file_count: 0,
            symbol_count: 0,
            test_count: 0,
        }
    }
}
```

`symbol_summary.rs`:

```rust
use crate::{CallSummary, Language, SignatureSummary, SourceSpan, SymbolKind, Visibility};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolSummary {
    pub id: String,
    pub language: Language,
    pub kind: SymbolKind,
    pub name: String,
    pub qualified_name: String,
    pub visibility: Visibility,
    pub span: SourceSpan,
    pub signature: Option<SignatureSummary>,
    pub calls: Vec<CallSummary>,
    pub is_test: bool,
}
```

`semantic_model.rs` should own model-level collections and helper lookups:

```rust
use crate::{FileSummary, ProjectSummary, SourceSpan, SymbolSummary};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SemanticModel {
    pub project: ProjectSummary,
    pub files: Vec<FileSummary>,
    pub symbols: Vec<SymbolSummary>,
    pub spans: Vec<SourceSpan>,
}

impl SemanticModel {
    pub fn project_id(&self) -> &str {
        &self.project.project_id
    }

    pub fn symbol_by_id(&self, symbol_id: &str) -> Option<&SymbolSummary> {
        self.symbols.iter().find(|symbol| symbol.id == symbol_id)
    }

    pub fn span_by_id(&self, span_id: &str) -> Option<&SourceSpan> {
        self.spans.iter().find(|span| span.id == span_id)
    }
}
```

`metric_summary.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MetricSummary {
    pub name: String,
    pub value: usize,
}
```

`module_summary.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleSummary {
    pub name: String,
    pub path: String,
    pub symbol_count: usize,
}
```

`span_id.rs`:

```rust
use crate::StableId;

pub type SpanId = StableId;
```

`symbol_id.rs`:

```rust
use crate::StableId;

pub type SymbolId = StableId;
```

`test_summary.rs`:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TestSummary {
    pub symbol_id: String,
    pub name: String,
    pub span_id: String,
}
```

Keep these model files as one serializable type per file, no broad catch-all
data blobs, and no source snippets by default.

## Phase 3: Rust Scanner Crate

Recommended `crates/refactor-radar-rust/src` layout:

```text
lib.rs
extract_file_items.rs
extract_visibility.rs
rust_scanner.rs
rust_scanner_error.rs
rust_scanner_result.rs
span_from_node.rs
```

`lib.rs`:

```rust
pub mod extract_file_items;
pub mod extract_visibility;
pub mod rust_scanner;
pub mod rust_scanner_error;
pub mod rust_scanner_result;
pub mod span_from_node;

pub use rust_scanner::RustScanner;
pub use rust_scanner_error::RustScannerError;
pub use rust_scanner_result::RustScannerResult;

use refactor_radar_core::SemanticModel;
use std::path::Path;

pub fn scan_project(path: &Path) -> RustScannerResult<SemanticModel> {
    let scanner = RustScanner;
    scanner.scan_project(path)
}
```

`rust_scanner_error.rs`:

```rust
use error_location::ErrorLocation;
use std::path::PathBuf;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RustScannerError {
    #[error("{message}")]
    WalkFailed {
        message: String,
        #[source]
        source: ignore::Error,
        location: ErrorLocation,
    },
    #[error("{message}")]
    ReadFileFailed {
        message: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
        location: ErrorLocation,
    },
    #[error("{message}")]
    TreeSitterLanguageFailed {
        message: String,
        #[source]
        source: tree_sitter::LanguageError,
        location: ErrorLocation,
    },
    #[error("{message}")]
    ParseCancelled {
        message: String,
        location: ErrorLocation,
    },
    #[error("{message}")]
    CoreModelFailed {
        message: String,
        #[source]
        source: refactor_radar_core::CoreError,
        location: ErrorLocation,
    },
}

impl RustScannerError {
    #[track_caller]
    pub fn walk_failed(source: ignore::Error) -> Self {
        Self::WalkFailed {
            message: "failed while walking Rust project files".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn read_file_failed(path: PathBuf, source: std::io::Error) -> Self {
        Self::ReadFileFailed {
            message: format!("failed to read Rust source file: {}", path.display()),
            path,
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn tree_sitter_language_failed(source: tree_sitter::LanguageError) -> Self {
        Self::TreeSitterLanguageFailed {
            message: "failed to load tree-sitter Rust language".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn parse_cancelled() -> Self {
        Self::ParseCancelled {
            message: "tree-sitter returned no parse tree".to_owned(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn core_model_failed(source: refactor_radar_core::CoreError) -> Self {
        Self::CoreModelFailed {
            message: "failed to build Rust semantic model".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::WalkFailed { message, .. }
            | Self::ReadFileFailed { message, .. }
            | Self::TreeSitterLanguageFailed { message, .. }
            | Self::ParseCancelled { message, .. }
            | Self::CoreModelFailed { message, .. } => message,
        }
    }

    pub fn location(&self) -> &ErrorLocation {
        match self {
            Self::WalkFailed { location, .. }
            | Self::ReadFileFailed { location, .. }
            | Self::TreeSitterLanguageFailed { location, .. }
            | Self::ParseCancelled { location, .. }
            | Self::CoreModelFailed { location, .. } => location,
        }
    }
}
```

`rust_scanner_result.rs`:

```rust
use crate::RustScannerError;

pub type RustScannerResult<T> = std::result::Result<T, RustScannerError>;
```

`rust_scanner.rs`:

```rust
use crate::{extract_file_items::extract_file_items, RustScannerError, RustScannerResult};
use ignore::WalkBuilder;
use refactor_radar_core::{FileSummary, Language, ProjectSummary, SemanticModel};
use std::fs;
use std::path::Path;
use tree_sitter::Parser;

#[derive(Clone, Default)]
pub struct RustScanner;

impl RustScanner {
    pub fn scan_project(&self, path: &Path) -> RustScannerResult<SemanticModel> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(RustScannerError::tree_sitter_language_failed)?;

        let mut model = SemanticModel {
            project: ProjectSummary {
                project_id: project_id_from_path(path),
                language: Language::Rust,
                ..ProjectSummary::default()
            },
            ..SemanticModel::default()
        };

        for entry in WalkBuilder::new(path).standard_filters(true).build() {
            let entry = entry.map_err(RustScannerError::walk_failed)?;
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                continue;
            }

            if !entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension == "rs")
            {
                continue;
            }

            let source = fs::read_to_string(entry.path())
                .map_err(|source| RustScannerError::read_file_failed(entry.path().to_path_buf(), source))?;
            let model_path = model_path_from_entry(path, entry.path());
            self.scan_file(&mut parser, &mut model, model_path, &source)?;
        }

        model.project.file_count = model.files.len();
        model.project.symbol_count = model.symbols.len();
        model.project.test_count = model
            .symbols
            .iter()
            .filter(|symbol| symbol.is_test)
            .count();

        Ok(model)
    }

    fn scan_file(
        &self,
        parser: &mut Parser,
        model: &mut SemanticModel,
        path: &Path,
        source: &str,
    ) -> RustScannerResult<()> {
        let tree = parser
            .parse(source, None)
            .ok_or_else(RustScannerError::parse_cancelled)?;
        model.files.push(FileSummary {
            path: path.to_string_lossy().to_string(),
            language: Language::Rust,
            line_count: source.lines().count(),
        });
        extract_file_items(model, path, source, tree.root_node())
    }
}

fn project_id_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "rust-project".to_owned())
}

fn model_path_from_entry<'a>(root: &Path, entry_path: &'a Path) -> &'a Path {
    match entry_path.strip_prefix(root) {
        Ok(relative_path) if !relative_path.as_os_str().is_empty() => relative_path,
        Ok(_) | Err(_) => entry_path,
    }
}
```

`extract_file_items.rs`:

```rust
use crate::extract_visibility::extract_visibility;
use crate::span_from_node::span_from_node;
use crate::{RustScannerError, RustScannerResult};
use refactor_radar_core::{
    Language, SemanticModel, SignatureSummary, StableId, SymbolKind, SymbolSummary,
};
use std::path::Path;
use tree_sitter::Node;

pub fn extract_file_items(
    model: &mut SemanticModel,
    path: &Path,
    source: &str,
    root: Node<'_>,
) -> RustScannerResult<()> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_item" => extract_function(model, path, source, child)?,
            "struct_item" => extract_named_symbol(model, path, source, child, SymbolKind::Struct)?,
            "enum_item" => extract_named_symbol(model, path, source, child, SymbolKind::Enum)?,
            "trait_item" => extract_named_symbol(model, path, source, child, SymbolKind::Trait)?,
            "impl_item" => extract_impl(model, path, source, child)?,
            "mod_item" => extract_named_symbol(model, path, source, child, SymbolKind::Module)?,
            _ => {}
        }
    }
    Ok(())
}

fn extract_function(
    model: &mut SemanticModel,
    path: &Path,
    source: &str,
    node: Node<'_>,
) -> RustScannerResult<()> {
    let mut symbol = build_named_symbol(model, path, source, node, SymbolKind::Function)?;
    symbol.signature = Some(extract_signature(source, node));
    symbol.is_test = has_test_attribute(source, node);
    model.spans.push(symbol.span.clone());
    model.symbols.push(symbol);
    Ok(())
}

fn extract_named_symbol(
    model: &mut SemanticModel,
    path: &Path,
    source: &str,
    node: Node<'_>,
    kind: SymbolKind,
) -> RustScannerResult<()> {
    let symbol = build_named_symbol(model, path, source, node, kind)?;
    model.spans.push(symbol.span.clone());
    model.symbols.push(symbol);
    Ok(())
}

fn extract_impl(
    model: &mut SemanticModel,
    path: &Path,
    _source: &str,
    node: Node<'_>,
) -> RustScannerResult<()> {
    let start = node.start_position();
    let name = format!(
        "impl@{}:{}",
        start.row.saturating_add(1),
        start.column.saturating_add(1)
    );
    let symbol = build_synthetic_symbol(model, path, node, SymbolKind::Impl, name)?;
    model.spans.push(symbol.span.clone());
    model.symbols.push(symbol);
    Ok(())
}

fn build_named_symbol(
    model: &SemanticModel,
    path: &Path,
    source: &str,
    node: Node<'_>,
    kind: SymbolKind,
) -> RustScannerResult<SymbolSummary> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return build_synthetic_symbol(model, path, node, kind, kind.as_str().to_owned());
    };
    let name = node_text(source, name_node).unwrap_or_else(|| kind.as_str().to_owned());
    build_synthetic_symbol(model, path, node, kind, name)
}

fn build_synthetic_symbol(
    model: &SemanticModel,
    path: &Path,
    node: Node<'_>,
    kind: SymbolKind,
    name: String,
) -> RustScannerResult<SymbolSummary> {
    let path_text = path.to_string_lossy().to_string();
    let span_id = format!(
        "span:{}:{}:{}:{}",
        model.project_id(),
        path_text,
        kind.as_str(),
        name
    );
    let span = span_from_node(span_id, path, node);
    let stable_id = StableId::symbol(
        Language::Rust,
        model.project_id(),
        &path_text,
        kind,
        &name,
    )
    .map_err(RustScannerError::core_model_failed)?;

    Ok(SymbolSummary {
        id: stable_id.as_str().to_owned(),
        language: Language::Rust,
        kind,
        name: name.clone(),
        qualified_name: name,
        visibility: extract_visibility(node),
        span,
        signature: None,
        calls: Vec::new(),
        is_test: false,
    })
}

fn extract_signature(source: &str, node: Node<'_>) -> SignatureSummary {
    let return_type = node
        .child_by_field_name("return_type")
        .and_then(|return_type| node_text(source, return_type));

    SignatureSummary {
        parameters: Vec::new(),
        return_type,
    }
}

fn has_test_attribute(source: &str, node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "attribute_item" {
            continue;
        }
        if node_text(source, child).is_some_and(|text| text.contains("#[test]")) {
            return true;
        }
    }
    false
}

fn node_text(source: &str, node: Node<'_>) -> Option<String> {
    node.utf8_text(source.as_bytes()).ok().map(str::to_owned)
}
```

This first extraction pass must be completed before Phase 7 tests are added.
The code above is only the starting shape; extend `extract_file_items.rs` in
Phase 3 so the later tests depend on scanner behavior that already exists:

- Track pending outer `attribute_item` siblings while walking declaration
  children. In tree-sitter Rust, attributes such as `#[test]` appear as
  declaration statements before the item they annotate, so do not rely only on
  `function_item` children to detect tests.
- Recurse into module and impl bodies while carrying a conservative module path
  context from Rust crate path conventions and inline `mod` declarations. Treat
  `src/lib.rs` and `src/main.rs` as crate roots, map `src/foo.rs` and
  `src/foo/mod.rs` to module `foo`, and do not include the literal `src` segment
  in qualified names. Top-level and module-level `function_item` nodes should
  produce `SymbolKind::Function` with a qualified name that includes the module
  path when present; `function_item` nodes under an `impl_item` should produce
  `SymbolKind::Method` with a qualified name that includes the module path and
  impl target when they can be extracted.
- When adding qualified-name extraction, change the symbol-building helpers to
  accept both the short display name and the full qualified name. Use the
  qualified name for `StableId::symbol`, `SymbolSummary.qualified_name`, and the
  symbol span ID suffix so nested functions, methods, and enum variants do not
  collide with same-named symbols in the same file.
- When extracting an `enum_item`, also extract its `enum_variant` children as
  `SymbolKind::EnumVariant` using the current module path plus
  `EnumName::VariantName` as the qualified name.
- For each function or method, walk descendant `call_expression` nodes and fill
  `SymbolSummary.calls` with compact `CallSummary { callee_name }` entries.
- Keep spans for every emitted symbol, and avoid double-emitting a descendant
  after the parent-specific extractor has already handled it.

After that scanner increment is green, add an `import_summary.rs` core type,
module/export it from core `lib.rs`, and wire `use_declaration` extraction into
`FileSummary`. Keep import modeling as a separate small scanner increment:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportSummary {
    pub path: String,
}
```

Update `file_summary.rs` to import `ImportSummary` and add
`pub imports: Vec<ImportSummary>`. Update `rust_scanner.rs` so every
`FileSummary` construction populates `imports` from the file's
`use_declaration` nodes. It is acceptable for malformed Rust files to produce
partial facts plus parse diagnostics; scanning should not fail solely because
`root.has_error()` is true.

`extract_visibility.rs`:

```rust
use refactor_radar_core::Visibility;
use tree_sitter::Node;

pub fn extract_visibility(node: Node<'_>) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return Visibility::Public;
        }
    }
    Visibility::Private
}
```

`span_from_node.rs`:

```rust
use refactor_radar_core::SourceSpan;
use std::path::Path;
use tree_sitter::Node;

pub fn span_from_node(span_id: impl Into<String>, path: &Path, node: Node<'_>) -> SourceSpan {
    let start = node.start_position();
    let end = node.end_position();
    SourceSpan::from_zero_based(
        span_id,
        path.to_string_lossy().to_string(),
        start.row,
        start.column,
        end.row,
        end.column,
    )
}
```

Tree-sitter Rust parser setup must use the current `LANGUAGE` constant pattern:

```rust
let language = tree_sitter_rust::LANGUAGE;
parser.set_language(&language.into())?;
```

## Phase 4: Report Crate

Recommended `crates/refactor-radar-report/src` layout:

```text
lib.rs
generate_llm_brief.rs
report_error.rs
report_result.rs
summarize_project.rs
```

`lib.rs`:

```rust
pub mod generate_llm_brief;
pub mod report_error;
pub mod report_result;
pub mod summarize_project;

pub use generate_llm_brief::generate_llm_brief;
pub use report_error::ReportError;
pub use report_result::ReportResult;
pub use summarize_project::summarize_project;
```

`report_error.rs`:

```rust
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("{message}")]
    InvalidTokenBudget {
        message: String,
        location: ErrorLocation,
    },
}

impl ReportError {
    #[track_caller]
    pub fn invalid_token_budget(value: u32) -> Self {
        Self::InvalidTokenBudget {
            message: format!("invalid token budget: {value}"),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidTokenBudget { message, .. } => message,
        }
    }
}
```

`report_result.rs`:

```rust
use crate::ReportError;

pub type ReportResult<T> = std::result::Result<T, ReportError>;
```

`summarize_project.rs`:

```rust
use refactor_radar_core::{ProjectSummary, SemanticModel};

pub fn summarize_project(model: &SemanticModel) -> ProjectSummary {
    let mut summary = model.project.clone();
    summary.file_count = model.files.len();
    summary.symbol_count = model.symbols.len();
    summary.test_count = model
        .symbols
        .iter()
        .filter(|symbol| symbol.is_test)
        .count();
    summary
}
```

`generate_llm_brief.rs`:

```rust
use refactor_radar_core::SemanticModel;

pub fn generate_llm_brief(model: &SemanticModel, budget_tokens: Option<u32>) -> String {
    let mut brief = String::new();
    brief.push_str("# RefactorRadar LLM Brief\n\n");
    brief.push_str("## Task Framing\n");
    brief.push_str("Separate confirmed facts from inference and unknowns.\n\n");
    brief.push_str("## Project\n");
    brief.push_str(&format!("- Project ID: {}\n", model.project.project_id));
    brief.push_str(&format!("- Language: {:?}\n", model.project.language));
    brief.push_str(&format!("- Files: {}\n", model.files.len()));
    brief.push_str(&format!("- Symbols: {}\n", model.symbols.len()));

    if let Some(budget_tokens) = budget_tokens {
        brief.push_str(&format!("\n## Budget\n- Approximate token budget: {budget_tokens}\n"));
    }

    brief
}
```

The first brief can use approximate token budgeting, but implement the sections
that make budget reduction observable before adding the Phase 7 report budget
test. Add compact sections for files, public symbols, private symbols, and call
lists. Because the current model does not include raw snippets or complexity
metrics, the first reduction pass must use only model-backed sections. Reduce in
this order when the brief exceeds budget: private function details, full call
lists, private module details when module summaries are present, low-signal
files, then project-level summary only. The minimal project-only brief above is
not enough to exercise the reduction test.

## Phase 5: MCP Server Crate

Recommended `crates/refactor-radar-mcp/src` layout:

```text
lib.rs
generate_llm_brief_params.rs
get_project_summary_params.rs
get_source_span_params.rs
get_symbol_params.rs
list_symbols_params.rs
mcp_error_conversion.rs
mcp_server_error.rs
mcp_server_result.rs
refactor_radar_mcp_server.rs
scan_cache.rs
scan_project_params.rs
scan_project_result.rs
```

`lib.rs`:

```rust
pub mod generate_llm_brief_params;
pub mod get_project_summary_params;
pub mod get_source_span_params;
pub mod get_symbol_params;
pub mod list_symbols_params;
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

use rmcp::{service::QuitReason, transport::stdio, ServiceExt};

pub async fn run_stdio() -> McpServerResult<()> {
    let service = RefactorRadarMcpServer::default();
    let running_service = service
        .serve(stdio())
        .await
        .map_err(McpServerError::serve_failed)?;
    let quit_reason = running_service
        .waiting()
        .await
        .map_err(McpServerError::wait_failed)?;
    match quit_reason {
        QuitReason::Cancelled | QuitReason::Closed => Ok(()),
        QuitReason::JoinError(source) => Err(McpServerError::runtime_task_failed(source)),
    }
}
```

`mcp_server_error.rs`:

```rust
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpServerError {
    #[error("{message}")]
    ServeFailed {
        message: String,
        #[source]
        source: rmcp::service::ServerInitializeError,
        location: ErrorLocation,
    },
    #[error("{message}")]
    WaitFailed {
        message: String,
        #[source]
        source: tokio::task::JoinError,
        location: ErrorLocation,
    },
    #[error("{message}")]
    RuntimeTaskFailed {
        message: String,
        #[source]
        source: tokio::task::JoinError,
        location: ErrorLocation,
    },
}

impl McpServerError {
    #[track_caller]
    pub fn serve_failed(source: rmcp::service::ServerInitializeError) -> Self {
        Self::ServeFailed {
            message: "failed to initialize RefactorRadar MCP stdio server".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn wait_failed(source: tokio::task::JoinError) -> Self {
        Self::WaitFailed {
            message: "RefactorRadar MCP stdio server task failed".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn runtime_task_failed(source: tokio::task::JoinError) -> Self {
        Self::RuntimeTaskFailed {
            message: "RefactorRadar MCP stdio server runtime task failed".to_owned(),
            source,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::ServeFailed { message, .. }
            | Self::WaitFailed { message, .. }
            | Self::RuntimeTaskFailed { message, .. } => message,
        }
    }
}
```

`mcp_server_result.rs`:

```rust
use crate::McpServerError;

pub type McpServerResult<T> = std::result::Result<T, McpServerError>;
```

`scan_cache.rs`:

```rust
use refactor_radar_core::{ProjectSummary, SemanticModel};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub struct ScanCache {
    models: HashMap<String, SemanticModel>,
    summaries: HashMap<String, ProjectSummary>,
}

impl ScanCache {
    pub fn insert(&mut self, model: SemanticModel, summary: ProjectSummary) -> String {
        let project_id = model.project_id().to_owned();
        let _ = self.summaries.insert(project_id.clone(), summary);
        let _ = self.models.insert(project_id.clone(), model);
        project_id
    }

    pub fn model(&self, project_id: &str) -> Option<&SemanticModel> {
        self.models.get(project_id)
    }

    pub fn summary(&self, project_id: &str) -> Option<&ProjectSummary> {
        self.summaries.get(project_id)
    }

    pub fn project_ids(&self) -> impl Iterator<Item = &String> {
        self.models.keys()
    }
}
```

`scan_project_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanProjectParams {
    pub path: String,
}
```

`scan_project_result.rs`:

```rust
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScanProjectResult {
    pub project_id: String,
}
```

`generate_llm_brief_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenerateLlmBriefParams {
    pub project_id: String,
    pub budget_tokens: Option<u32>,
}
```

Create the remaining parameter files with the fields used by
`refactor_radar_mcp_server.rs`. Each file gets only one named type.

`get_project_summary_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProjectSummaryParams {
    pub project_id: String,
}
```

`list_symbols_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSymbolsParams {
    pub project_id: String,
}
```

`get_symbol_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolParams {
    pub symbol_id: String,
}
```

`get_source_span_params.rs`:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSourceSpanParams {
    pub span_id: String,
}
```

`mcp_error_conversion.rs`:

```rust
use refactor_radar_rust::RustScannerError;
use rmcp::ErrorData as McpError;
use serde_json::json;

pub fn scanner_error_to_mcp(error: RustScannerError) -> McpError {
    McpError::internal_error(
        "scan_project failed",
        Some(json!({
            "message": error.message(),
            "location": format!("{:?}", error.location()),
        })),
    )
}

pub fn not_found_to_mcp(kind: &str, id: &str) -> McpError {
    McpError::resource_not_found(
        "requested RefactorRadar item was not found",
        Some(json!({
            "kind": kind,
            "id": id,
        })),
    )
}
```

`refactor_radar_mcp_server.rs`:

```rust
use crate::generate_llm_brief_params::GenerateLlmBriefParams;
use crate::get_project_summary_params::GetProjectSummaryParams;
use crate::get_source_span_params::GetSourceSpanParams;
use crate::get_symbol_params::GetSymbolParams;
use crate::list_symbols_params::ListSymbolsParams;
use crate::mcp_error_conversion::{not_found_to_mcp, scanner_error_to_mcp};
use crate::scan_cache::ScanCache;
use crate::scan_project_params::ScanProjectParams;
use crate::scan_project_result::ScanProjectResult;
use refactor_radar_report::{generate_llm_brief, summarize_project};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router, ErrorData as McpError};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct RefactorRadarMcpServer {
    cache: Arc<RwLock<ScanCache>>,
}

#[tool_router(server_handler)]
impl RefactorRadarMcpServer {
    #[tool(description = "Scan a Rust project and cache compact semantic facts")]
    pub async fn scan_project(
        &self,
        Parameters(params): Parameters<ScanProjectParams>,
    ) -> Result<Json<ScanProjectResult>, McpError> {
        let path = PathBuf::from(params.path);
        let model = refactor_radar_rust::scan_project(&path).map_err(scanner_error_to_mcp)?;
        let summary = summarize_project(&model);
        let project_id = self.cache.write().await.insert(model, summary);
        Ok(Json(ScanProjectResult { project_id }))
    }

    #[tool(description = "Get the cached project summary for a scanned project")]
    pub async fn get_project_summary(
        &self,
        Parameters(params): Parameters<GetProjectSummaryParams>,
    ) -> Result<Json<refactor_radar_core::ProjectSummary>, McpError> {
        let cache = self.cache.read().await;
        let summary = cache
            .summary(&params.project_id)
            .ok_or_else(|| not_found_to_mcp("project", &params.project_id))?;
        Ok(Json(summary.clone()))
    }

    #[tool(description = "List compact symbols for a scanned project")]
    pub async fn list_symbols(
        &self,
        Parameters(params): Parameters<ListSymbolsParams>,
    ) -> Result<Json<Vec<refactor_radar_core::SymbolSummary>>, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .model(&params.project_id)
            .ok_or_else(|| not_found_to_mcp("project", &params.project_id))?;
        Ok(Json(model.symbols.clone()))
    }

    #[tool(description = "Get one compact symbol by stable symbol ID")]
    pub async fn get_symbol(
        &self,
        Parameters(params): Parameters<GetSymbolParams>,
    ) -> Result<Json<refactor_radar_core::SymbolSummary>, McpError> {
        let cache = self.cache.read().await;
        for model in cache_models(&cache) {
            if let Some(symbol) = model.symbol_by_id(&params.symbol_id) {
                return Ok(Json(symbol.clone()));
            }
        }
        Err(not_found_to_mcp("symbol", &params.symbol_id))
    }

    #[tool(description = "Get source span metadata by stable span ID")]
    pub async fn get_source_span(
        &self,
        Parameters(params): Parameters<GetSourceSpanParams>,
    ) -> Result<Json<refactor_radar_core::SourceSpan>, McpError> {
        let cache = self.cache.read().await;
        for model in cache_models(&cache) {
            if let Some(span) = model.span_by_id(&params.span_id) {
                return Ok(Json(span.clone()));
            }
        }
        Err(not_found_to_mcp("span", &params.span_id))
    }

    #[tool(description = "Generate an LLM-ready brief for a scanned project")]
    pub async fn generate_llm_brief(
        &self,
        Parameters(params): Parameters<GenerateLlmBriefParams>,
    ) -> Result<String, McpError> {
        let cache = self.cache.read().await;
        let model = cache
            .model(&params.project_id)
            .ok_or_else(|| not_found_to_mcp("project", &params.project_id))?;
        Ok(generate_llm_brief(model, params.budget_tokens))
    }
}

fn cache_models(cache: &ScanCache) -> impl Iterator<Item = &refactor_radar_core::SemanticModel> {
    cache.project_ids().filter_map(|project_id| cache.model(project_id))
}
```

`src/bin/refactor-radar-mcp.rs`:

```rust
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match refactor_radar_mcp::run_stdio().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", error.message());
            ExitCode::FAILURE
        }
    }
}
```

Do not use `println!`, `dbg!`, or stdout logging anywhere in the MCP server.

## Phase 6: Fixture Coverage

Create small Rust fixture crates under `testdata/rust`. Keep fixtures focused
and easy to inspect.

Example fixture:

```bash
mkdir -p testdata/rust/basic_crate/src
```

`testdata/rust/basic_crate/Cargo.toml`:

```toml
[package]
name = "basic-crate"
version = "0.1.0"
edition = "2024"
```

`testdata/rust/basic_crate/src/lib.rs`:

```rust
pub mod orders;

pub enum OrderError {
    MissingId,
    InvalidQuantity,
}

pub fn create_order(id: &str, quantity: u32) -> Result<(), OrderError> {
    validate_id(id)?;
    validate_quantity(quantity)?;
    Ok(())
}

fn validate_id(id: &str) -> Result<(), OrderError> {
    if id.is_empty() {
        return Err(OrderError::MissingId);
    }
    Ok(())
}

fn validate_quantity(quantity: u32) -> Result<(), OrderError> {
    if quantity == 0 {
        return Err(OrderError::InvalidQuantity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_valid_order_when_created_then_result_is_ok() {
        assert!(create_order("order-1", 1).is_ok());
    }
}
```

`testdata/rust/basic_crate/src/orders.rs`:

```rust
pub struct Order {
    pub id: String,
}

impl Order {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}
```

## Phase 7: Tests

Add tests in the crate that owns the behavior being tested.

Core tests:

- stable ID generation is deterministic
- stable IDs normalize path separators
- source span conversion returns 1-based line and column fields
- serializable model structs round-trip through JSON when needed

Rust scanner tests:

- detects files and modules
- extracts public/private functions
- extracts structs, enums, enum variants, traits, impls, and methods
- assigns module-qualified names for nested module functions, methods, and enum
  variants
- detects `#[test]` functions
- extracts simple call names from function bodies
- extracts `use` declarations into file import summaries
- does not fail on a malformed `.rs` file with tree-sitter parse errors
- respects `.gitignore` via `ignore::WalkBuilder::standard_filters(true)`

Report tests:

- project summary counts files, symbols, and tests
- LLM brief separates task framing from observed project facts
- token budget reduction removes lower-signal sections first

MCP tests:

- direct server handler test for `scan_project`
- `get_project_summary` returns a compact structured JSON result
- `list_symbols` does not return raw source
- `get_symbol` returns one symbol by stable ID
- `get_source_span` returns span metadata only
- `generate_llm_brief` returns Markdown text for a cached project
- unknown IDs map to MCP resource-not-found errors

Use given/when/then test names, for example:

```rust
#[test]
fn given_crate_with_public_function_when_scanned_then_function_has_stable_symbol_id() {
    // ...
}
```

## Phase 8: Verification Commands

Run formatting, tests, clippy, and build:

```bash
cargo fmt --all -- --check
cargo test -p refactor-radar-core
cargo test -p refactor-radar-rust
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

Verify the application/library error rule:

```bash
rg -n 'anyhow|unwrap\(|expect\(|panic!|dbg!' crates/refactor-radar-*
rg -n '\bprintln!' crates/refactor-radar-mcp
```

Expected results:

- No `anyhow` in application/library crates.
- No `unwrap()`, `expect()`, or `panic!`.
- No `println!` or `dbg!` in `refactor-radar-mcp`.
- Any `eprintln!` is confined to the MCP binary entrypoint or explicit stderr
  diagnostics.

## Acceptance Criteria

- Workspace contains the four ADR-aligned RefactorRadar crates plus existing
  `crates/wip`.
- Each new crate has a local typed error enum and result alias in separate files.
- Every error constructor uses `#[track_caller]` and `ErrorLocation::from(Location::caller())`.
- No source file defines more than one named type.
- Rust scanner walks `.rs` files with `.gitignore` support.
- Rust parser setup uses `tree_sitter_rust::LANGUAGE.into()`.
- Public model source spans expose 1-based lines and columns.
- Stable IDs are deterministic and include language, project, path, symbol kind,
  and qualified name.
- MCP server runs over stdio with `rmcp` and `transport-io`.
- MCP tools expose compact structured facts and do not return raw source by
  default.
- `get_source_span` returns explicit span metadata for follow-up inspection.
- Runtime operation is local-only and does not require network access.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo build --workspace` pass.

## Deferred Scope

- Full CLI implementation.
- Persistent `.refactor-radar/` artifact cache.
- Workspace-wide cross-project analysis.
- Refactor candidate ranking.
- C# support.
- HTTP MCP transport.
- Agentic orchestration.
