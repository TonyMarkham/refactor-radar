# VS-01: Data Model

## Target Outcome

Replace the temporary `crates/wip` workspace member with `crates/rr-data-model`
and implement the concrete serializable RefactorRadar records shared by scanner
binaries, `rr-core`, `rr-mcp`, summary artifacts, and future editor integrations.

This slice creates nouns, schema/version constants, constructors, validation,
and tests only. It does not scan source, parse SCIP, compute hashes, compare an
artifact DB, invoke MCP, or alter any submodule.

## Hard Constraints

Every implementation step in this plan must preserve these constraints:

```text
DO edit:
  Cargo.toml
  Cargo.lock
  crates/wip/**
  crates/rr-data-model/**

DO NOT edit:
  submodules/**
  v1/**
```

Use the repo error policy from `AGENTS.md`:

```rust
// Pattern to preserve in every crate-local error type.
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataModelError {
    #[error("{message} at {location}")]
    InvalidId {
        message: &'static str,
        value: String,
        location: ErrorLocation,
    },
}

impl DataModelError {
    #[track_caller]
    pub fn invalid_id(value: impl Into<String>) -> Self {
        Self::InvalidId {
            message: "data-model ID must not be empty",
            value: value.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidId { message, .. } => message,
        }
    }
}
```

Keep at most one top-level Rust type definition per source file:

```bash
violations="$(
  rg -n '^(pub(\([^)]*\))?\s+)?(struct|enum|trait)\s+|^(pub(\([^)]*\))?\s+)?type\s+[A-Z][A-Za-z0-9_]*\s*=' crates/rr-data-model/src/*.rs \
    | cut -d: -f1 \
    | sort \
    | uniq -c \
    | awk '$1 > 1 {print}'
)"
test -z "$violations" || {
  printf 'more than one top-level type in file:\n%s\n' "$violations"
  exit 1
}
```

## Confirmed Facts

The v2 root workspace currently contains only `crates/wip`:

```toml
[workspace]
members = [
    "crates/wip"
]
resolver = "3"
```

Workspace dependencies already include the dependencies needed by this slice:

```toml
serde          = { version = "1.0.228", features = ["derive"] }
serde_json     = { version = "1.0.150" }
schemars       = { version = "1.2.1", features = ["derive"] }
thiserror      = { version = "2.0.18" }
error-location = { version = "0.1.0" }
```

SCIP concepts to mirror without depending on SCIP protobuf types:

```text
submodules/scip/scip.proto

SCIP Index          -> ScanIndex / ScanResponse
SCIP Metadata       -> ScanMetadata
SCIP Document       -> Document
SCIP SymbolInformation -> Symbol
SCIP Occurrence     -> Occurrence
SCIP typed ranges   -> Range + RangeShape
SCIP Signature      -> Signature
SCIP Relationship   -> Relationship
SCIP diagnostics    -> Diagnostic
```

The data-model crate must not depend on SCIP or any runtime crate:

```toml
# crates/rr-data-model/Cargo.toml
[dependencies]
error-location = { workspace = true }
schemars       = { workspace = true }
serde          = { workspace = true }
thiserror      = { workspace = true }

[dev-dependencies]
serde_json     = { workspace = true }
```

## Implementation Steps

### 1. Replace The Workspace Member

Run these shell commands from the repo root:

```bash
git rm -r crates/wip
mkdir -p crates/rr-data-model/src crates/rr-data-model/tests
```

Update the root workspace members:

```toml
# Cargo.toml
[workspace]
members = [
    "crates/rr-data-model"
]
resolver = "3"
```

Create the new crate manifest:

```toml
# crates/rr-data-model/Cargo.toml
[package]
name = "rr-data-model"
version.workspace = true
edition.workspace = true
repository.workspace = true

[dependencies]
error-location = { workspace = true }
schemars       = { workspace = true }
serde          = { workspace = true }
thiserror      = { workspace = true }

[dev-dependencies]
serde_json     = { workspace = true }

[lints]
workspace = true
```

### 2. Create One-Type-Per-File Module Layout

Use this file layout. Each listed `.rs` file defines either zero types or one
type. Do not add `ids.rs`, `hash.rs`, `scan.rs`, or `artifact.rs` files that
bundle multiple structs/enums. Create the directories and source files now;
wire `lib.rs` after the concrete module files exist.

```text
crates/rr-data-model/
  Cargo.toml
  src/
    lib.rs
    artifact_aspect.rs
    artifact_db.rs
    artifact_document.rs
    artifact_element.rs
    artifact_project.rs
    artifact_status.rs
    content_hash.rs
    diagnostic.rs
    diagnostic_severity.rs
    diagnostic_tag.rs
    document.rs
    document_encoding.rs
    element.rs
    element_id.rs
    element_locator.rs
    element_state.rs
    error.rs
    occurrence.rs
    position_encoding.rs
    project.rs
    range.rs
    range_shape.rs
    range_wire.rs
    relationship.rs
    result.rs
    scan_index.rs
    scan_metadata.rs
    scan_request.rs
    scan_response.rs
    scan_run.rs
    schema_version.rs
    signature.rs
    symbol.rs
    symbol_kind.rs
    symbol_role.rs
    try_from_data_model.rs
    try_into_data_model.rs
    work_item.rs
    work_manifest.rs
  tests/
    id_hash_tests.rs
    range_tests.rs
    serialization_tests.rs
    trait_tests.rs
```

### 3. Add Error And Result Types

Implement the crate-local typed error enum:

```rust
// crates/rr-data-model/src/error.rs
use error_location::ErrorLocation;
use std::panic::Location;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataModelError {
    #[error("{message} at {location}")]
    InvalidId {
        message: &'static str,
        value: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    InvalidHash {
        message: &'static str,
        value: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    InvalidRange {
        message: &'static str,
        details: String,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    UnsupportedSchemaVersion {
        message: &'static str,
        schema_version: u32,
        location: ErrorLocation,
    },
    #[error("{message} at {location}")]
    DuplicateMapKey {
        message: &'static str,
        key: String,
        location: ErrorLocation,
    },
}

impl DataModelError {
    #[track_caller]
    pub fn invalid_id(value: impl Into<String>) -> Self {
        Self::InvalidId {
            message: "data-model ID must not be empty",
            value: value.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn invalid_hash(value: impl Into<String>) -> Self {
        Self::InvalidHash {
            message: "content hash must use sha256:<64 lowercase hex chars>",
            value: value.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn invalid_range(details: impl Into<String>) -> Self {
        Self::InvalidRange {
            message: "range must be a valid half-open zero-based source range",
            details: details.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn unsupported_schema_version(schema_version: u32) -> Self {
        Self::UnsupportedSchemaVersion {
            message: "schema version is not supported by rr-data-model",
            schema_version,
            location: ErrorLocation::from(Location::caller()),
        }
    }

    #[track_caller]
    pub fn duplicate_map_key(key: impl Into<String>) -> Self {
        Self::DuplicateMapKey {
            message: "duplicate key while building data-model map",
            key: key.into(),
            location: ErrorLocation::from(Location::caller()),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidId { message, .. }
            | Self::InvalidHash { message, .. }
            | Self::InvalidRange { message, .. }
            | Self::UnsupportedSchemaVersion { message, .. }
            | Self::DuplicateMapKey { message, .. } => message,
        }
    }
}
```

Add the crate-local result alias:

```rust
// crates/rr-data-model/src/result.rs
use crate::DataModelError;

pub type DataModelResult<T> = std::result::Result<T, DataModelError>;
```

### 4. Add Schema Version Constants

Keep constants and functions in `schema_version.rs`; it defines no type:

```rust
// crates/rr-data-model/src/schema_version.rs
use crate::{DataModelError, DataModelResult};

pub const SCHEMA_VERSION: u32 = 1;
pub const SCHEMA_VERSION_FIELD: &str = "schema_version";

pub fn validate_schema_version(schema_version: u32) -> DataModelResult<()> {
    if schema_version == SCHEMA_VERSION {
        return Ok(());
    }

    Err(DataModelError::unsupported_schema_version(schema_version))
}
```

### 5. Add Conversion Traits

The generic traits live in `rr-data-model`; implementations live in scanner
crates such as `rr-scip-rust` and `rr-scip-dotnet`. Because Rust orphan rules
do not allow a scanner crate to implement an external trait for an external
SCIP/protobuf type, scanner crates must implement these traits for
scanner-local adapter or newtype types instead of raw SCIP/protobuf types.

```rust
// crates/rr-data-model/src/try_into_data_model.rs
pub trait TryIntoDataModel<T> {
    type Error;

    fn try_into_data_model(self) -> Result<T, Self::Error>;
}
```

```rust
// crates/rr-data-model/src/try_from_data_model.rs
pub trait TryFromDataModel<T>: Sized {
    type Error;

    fn try_from_data_model(value: T) -> Result<Self, Self::Error>;
}
```

### 6. Add ID And Hash Value Types

Use `ElementId` instead of raw SCIP symbols for durable RR identity:

```rust
// crates/rr-data-model/src/element_id.rs
use crate::{DataModelError, DataModelResult};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ElementId(String);

impl ElementId {
    pub fn new(value: impl Into<String>) -> DataModelResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DataModelError::invalid_id(value));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ElementId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(|error| serde::de::Error::custom(error.message()))
    }
}
```

Use prefixed hashes now, even though later slices compute them:

```rust
// crates/rr-data-model/src/content_hash.rs
use crate::{DataModelError, DataModelResult};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn parse(value: impl Into<String>) -> DataModelResult<Self> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(DataModelError::invalid_hash(value));
        };

        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(DataModelError::invalid_hash(value));
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(|error| serde::de::Error::custom(error.message()))
    }
}
```

### 7. Add Encoding And Range Types

Model SCIP document text encoding separately from range position encoding:

```rust
// crates/rr-data-model/src/document_encoding.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentEncoding {
    Unspecified,
    Utf8,
    Utf16,
}
```

```rust
// crates/rr-data-model/src/position_encoding.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionEncoding {
    Unspecified,
    Utf8CodeUnitOffsetFromLineStart,
    Utf16CodeUnitOffsetFromLineStart,
    Utf32CodeUnitOffsetFromLineStart,
}
```

Preserve whether a scanner emitted a single-line or multi-line typed range:

```rust
// crates/rr-data-model/src/range_shape.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeShape {
    SingleLine,
    MultiLine,
}
```

Normalize range coordinates to one half-open shape for RR logic:

```rust
// crates/rr-data-model/src/range_wire.rs
use crate::RangeShape;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub(crate) struct RangeWire {
    pub(crate) shape: RangeShape,
    pub(crate) start_line: u32,
    pub(crate) start_character: u32,
    pub(crate) end_line: u32,
    pub(crate) end_character: u32,
}
```

```rust
// crates/rr-data-model/src/range.rs
use crate::{range_wire::RangeWire, DataModelError, DataModelResult, RangeShape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(try_from = "RangeWire")]
pub struct Range {
    pub shape: RangeShape,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

impl Range {
    pub fn single_line(
        line: u32,
        start_character: u32,
        end_character: u32,
    ) -> DataModelResult<Self> {
        if end_character < start_character {
            return Err(DataModelError::invalid_range(
                "single-line end_character precedes start_character",
            ));
        }

        Ok(Self {
            shape: RangeShape::SingleLine,
            start_line: line,
            start_character,
            end_line: line,
            end_character,
        })
    }

    pub fn multi_line(
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> DataModelResult<Self> {
        if end_line < start_line {
            return Err(DataModelError::invalid_range(
                "multi-line end_line precedes start_line",
            ));
        }

        if end_line == start_line && end_character < start_character {
            return Err(DataModelError::invalid_range(
                "multi-line end_character precedes start_character on same line",
            ));
        }

        Ok(Self {
            shape: RangeShape::MultiLine,
            start_line,
            start_character,
            end_line,
            end_character,
        })
    }
}

impl TryFrom<RangeWire> for Range {
    type Error = DataModelError;

    fn try_from(value: RangeWire) -> Result<Self, Self::Error> {
        let RangeWire {
            shape,
            start_line,
            start_character,
            end_line,
            end_character,
        } = value;

        match shape {
            RangeShape::SingleLine => {
                if end_line != start_line {
                    return Err(DataModelError::invalid_range(
                        "single-line end_line must equal start_line",
                    ));
                }

                Self::single_line(start_line, start_character, end_character)
            }
            RangeShape::MultiLine => {
                Self::multi_line(start_line, start_character, end_line, end_character)
            }
        }
    }
}
```

### 8. Add SCIP-Aligned Symbol And Occurrence Types

Represent symbol kind as raw SCIP-compatible evidence, not a Rust/C# enum:

```rust
// crates/rr-data-model/src/symbol_kind.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct SymbolKind {
    pub value: i32,
    pub name: String,
}

impl SymbolKind {
    pub fn new(value: i32, name: impl Into<String>) -> Self {
        Self {
            value,
            name: name.into(),
        }
    }

    pub fn unspecified() -> Self {
        Self::new(0, "UnspecifiedKind")
    }
}
```

Preserve SCIP role bitset semantics and unknown future bits:

```rust
// crates/rr-data-model/src/symbol_role.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct SymbolRole {
    pub bits: u32,
}

impl SymbolRole {
    pub const DEFINITION: u32 = 0x1;
    pub const IMPORT: u32 = 0x2;
    pub const WRITE_ACCESS: u32 = 0x4;
    pub const READ_ACCESS: u32 = 0x8;
    pub const GENERATED: u32 = 0x10;
    pub const TEST: u32 = 0x20;
    pub const FORWARD_DEFINITION: u32 = 0x40;
    pub const KNOWN_BITS: u32 = Self::DEFINITION
        | Self::IMPORT
        | Self::WRITE_ACCESS
        | Self::READ_ACCESS
        | Self::GENERATED
        | Self::TEST
        | Self::FORWARD_DEFINITION;

    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    pub fn contains(&self, flag: u32) -> bool {
        self.bits & flag != 0
    }

    pub fn unknown_bits(&self) -> u32 {
        self.bits & !Self::KNOWN_BITS
    }
}
```

Relationships mirror SCIP relationship flags:

```rust
// crates/rr-data-model/src/relationship.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Relationship {
    pub symbol: String,
    pub is_reference: bool,
    pub is_implementation: bool,
    pub is_type_definition: bool,
    pub is_definition: bool,
}
```

Signatures mirror SCIP signatures:

```rust
// crates/rr-data-model/src/signature.rs
use crate::Occurrence;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Signature {
    pub language: String,
    pub text: String,
    #[serde(default)]
    pub occurrences: Vec<Occurrence>,
}
```

Symbols preserve scanner/indexer evidence without becoming durable identity:

```rust
// crates/rr-data-model/src/symbol.rs
use crate::{Relationship, Signature, SymbolKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Symbol {
    pub raw_symbol: String,
    pub display_name: String,
    pub kind: SymbolKind,
    #[serde(default)]
    pub documentation: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
}
```

Diagnostics carry scanner/indexer source diagnostics:

```rust
// crates/rr-data-model/src/diagnostic_severity.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Unspecified,
    Error,
    Warning,
    Information,
    Hint,
}
```

```rust
// crates/rr-data-model/src/diagnostic_tag.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTag {
    Unspecified,
    Unnecessary,
    Deprecated,
    Unknown(String),
}
```

```rust
// crates/rr-data-model/src/diagnostic.rs
use crate::{DiagnosticSeverity, DiagnosticTag, Range};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub tags: Vec<DiagnosticTag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
}
```

Occurrences mirror SCIP occurrence data and preserve typed enclosing ranges:

```rust
// crates/rr-data-model/src/occurrence.rs
use crate::{Diagnostic, Range, SymbolRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Occurrence {
    pub range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default)]
    pub symbol_roles: SymbolRole,
    #[serde(default)]
    pub override_documentation: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_kind: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_range: Option<Range>,
}
```

### 9. Add Scan-Level Types

Scanner request:

```rust
// crates/rr-data-model/src/scan_request.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ScanRequest {
    pub target_path: String,
    pub scanner_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub scanner_args: Vec<String>,
}
```

RR-owned scan run metadata:

```rust
// crates/rr-data-model/src/scan_run.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ScanRun {
    pub run_id: String,
    pub scanner_key: String,
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub command: Vec<String>,
}
```

SCIP-aligned metadata:

```rust
// crates/rr-data-model/src/scan_metadata.rs
use crate::DocumentEncoding;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ScanMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_schema_version: Option<String>,
    pub scanner_tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanner_tool_version: Option<String>,
    #[serde(default)]
    pub scanner_tool_arguments: Vec<String>,
    pub project_root: String,
    pub document_encoding: DocumentEncoding,
}
```

Project identity:

```rust
// crates/rr-data-model/src/project.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Project {
    pub project_key: String,
    pub project_root: String,
    pub scanner_key: String,
}
```

Document records:

```rust
// crates/rr-data-model/src/document.rs
use crate::{ContentHash, Occurrence, PositionEncoding, Symbol};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Document {
    pub relative_path: String,
    pub language: String,
    pub position_encoding: PositionEncoding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub occurrences: Vec<Occurrence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<ContentHash>,
}
```

Scan index:

```rust
// crates/rr-data-model/src/scan_index.rs
use crate::{Document, Project, ScanMetadata, Symbol};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ScanIndex {
    pub metadata: ScanMetadata,
    pub project: Project,
    #[serde(default)]
    pub documents: Vec<Document>,
    #[serde(default)]
    pub external_symbols: Vec<Symbol>,
}
```

Scan response:

```rust
// crates/rr-data-model/src/scan_response.rs
use crate::{schema_version::SCHEMA_VERSION, Diagnostic, ScanIndex, ScanRun};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ScanResponse {
    pub schema_version: u32,
    pub run: ScanRun,
    pub index: ScanIndex,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl ScanResponse {
    pub fn new(run: ScanRun, index: ScanIndex) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            run,
            index,
            diagnostics: Vec::new(),
        }
    }
}
```

### 10. Add Element Types

Element locators are current-scan locators, not durable IDs:

```rust
// crates/rr-data-model/src/element_locator.rs
use crate::Range;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ElementLocator {
    pub document_path: String,
    pub source_symbol: String,
    pub definition_range: Range,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_range: Option<Range>,
}
```

Elements are RR-owned summarization targets derived by scanner crates:

```rust
// crates/rr-data-model/src/element.rs
use crate::{
    ContentHash, ElementId, ElementLocator, Occurrence, Relationship, Signature, SymbolKind,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct Element {
    pub id: ElementId,
    pub document_path: String,
    pub source_symbol: String,
    pub display_name: String,
    pub kind: SymbolKind,
    pub definition: Occurrence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing: Option<ElementLocator>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_slice_hash: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
}
```

### 11. Add Artifact DB Types

Artifact element state:

```rust
// crates/rr-data-model/src/element_state.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementState {
    Active,
    Stale,
    Deleted,
}
```

Artifact aspect status:

```rust
// crates/rr-data-model/src/artifact_status.rs
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Pending,
    Current,
    Stale,
    Deleted,
    Failed,
}
```

One summary target for one element:

```rust
// crates/rr-data-model/src/artifact_aspect.rs
use crate::{ArtifactStatus, ContentHash, ElementId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactAspect {
    pub aspect_id: String,
    pub element_id: ElementId,
    pub artifact_path: String,
    pub source_hash: ContentHash,
    pub status: ArtifactStatus,
}
```

One cached element:

```rust
// crates/rr-data-model/src/artifact_element.rs
use crate::{ArtifactAspect, ContentHash, ElementId, ElementState, SymbolKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactElement {
    pub id: ElementId,
    pub display_name: String,
    pub kind: SymbolKind,
    pub source_symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_slice_hash: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_summarized_hash: Option<ContentHash>,
    pub state: ElementState,
    #[serde(default)]
    pub aspects: BTreeMap<String, ArtifactAspect>,
}
```

One cached document:

```rust
// crates/rr-data-model/src/artifact_document.rs
use crate::{ArtifactElement, ContentHash, ElementId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactDocument {
    pub relative_path: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<ContentHash>,
    #[serde(default)]
    pub elements: BTreeMap<ElementId, ArtifactElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scan_run_id: Option<String>,
}
```

One cached project:

```rust
// crates/rr-data-model/src/artifact_project.rs
use crate::ArtifactDocument;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactProject {
    pub project_key: String,
    pub project_root: String,
    pub scanner_key: String,
    #[serde(default)]
    pub documents: BTreeMap<String, ArtifactDocument>,
}
```

The durable artifact DB:

```rust
// crates/rr-data-model/src/artifact_db.rs
use crate::{schema_version::SCHEMA_VERSION, ArtifactProject, ScanRun};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactDb {
    pub schema_version: u32,
    #[serde(default)]
    pub projects: BTreeMap<String, ArtifactProject>,
    #[serde(default)]
    pub scans: Vec<ScanRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_scan: Option<ScanRun>,
}

impl ArtifactDb {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            projects: BTreeMap::new(),
            scans: Vec::new(),
            latest_scan: None,
        }
    }
}
```

### 12. Add Work Manifest Types

One summarizer work item:

```rust
// crates/rr-data-model/src/work_item.rs
use crate::{ContentHash, ElementId, ElementLocator};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct WorkItem {
    pub work_item_id: String,
    pub document_path: String,
    pub element_id: ElementId,
    pub aspect_id: String,
    pub target_artifact_path: String,
    pub source_slice_hash: ContentHash,
    pub scanner_evidence: ElementLocator,
}
```

The manifest emitted by later hash-gate logic:

```rust
// crates/rr-data-model/src/work_manifest.rs
use crate::{schema_version::SCHEMA_VERSION, WorkItem};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct WorkManifest {
    pub schema_version: u32,
    pub manifest_id: String,
    pub scan_run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub work_items: Vec<WorkItem>,
}

impl WorkManifest {
    pub fn new(manifest_id: impl Into<String>, scan_run_id: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            manifest_id: manifest_id.into(),
            scan_run_id: scan_run_id.into(),
            generated_at: None,
            work_items: Vec::new(),
        }
    }
}
```

### 13. Wire The Crate Root

After all concrete module files exist, wire the modules and public exports from
`lib.rs`:

```rust
// crates/rr-data-model/src/lib.rs
pub mod artifact_aspect;
pub mod artifact_db;
pub mod artifact_document;
pub mod artifact_element;
pub mod artifact_project;
pub mod artifact_status;
pub mod content_hash;
pub mod diagnostic;
pub mod diagnostic_severity;
pub mod diagnostic_tag;
pub mod document;
pub mod document_encoding;
pub mod element;
pub mod element_id;
pub mod element_locator;
pub mod element_state;
pub mod error;
pub mod occurrence;
pub mod position_encoding;
pub mod project;
pub mod range;
pub mod range_shape;
mod range_wire;
pub mod relationship;
pub mod result;
pub mod scan_index;
pub mod scan_metadata;
pub mod scan_request;
pub mod scan_response;
pub mod scan_run;
pub mod schema_version;
pub mod signature;
pub mod symbol;
pub mod symbol_kind;
pub mod symbol_role;
pub mod try_from_data_model;
pub mod try_into_data_model;
pub mod work_item;
pub mod work_manifest;

pub use artifact_aspect::ArtifactAspect;
pub use artifact_db::ArtifactDb;
pub use artifact_document::ArtifactDocument;
pub use artifact_element::ArtifactElement;
pub use artifact_project::ArtifactProject;
pub use artifact_status::ArtifactStatus;
pub use content_hash::ContentHash;
pub use diagnostic::Diagnostic;
pub use diagnostic_severity::DiagnosticSeverity;
pub use diagnostic_tag::DiagnosticTag;
pub use document::Document;
pub use document_encoding::DocumentEncoding;
pub use element::Element;
pub use element_id::ElementId;
pub use element_locator::ElementLocator;
pub use element_state::ElementState;
pub use error::DataModelError;
pub use occurrence::Occurrence;
pub use position_encoding::PositionEncoding;
pub use project::Project;
pub use range::Range;
pub use range_shape::RangeShape;
pub use relationship::Relationship;
pub use result::DataModelResult;
pub use scan_index::ScanIndex;
pub use scan_metadata::ScanMetadata;
pub use scan_request::ScanRequest;
pub use scan_response::ScanResponse;
pub use scan_run::ScanRun;
pub use signature::Signature;
pub use symbol::Symbol;
pub use symbol_kind::SymbolKind;
pub use symbol_role::SymbolRole;
pub use try_from_data_model::TryFromDataModel;
pub use try_into_data_model::TryIntoDataModel;
pub use work_item::WorkItem;
pub use work_manifest::WorkManifest;
```

## Tests

### ID And Hash Tests

```rust
// crates/rr-data-model/tests/id_hash_tests.rs
use rr_data_model::{ContentHash, DataModelError, ElementId};

#[test]
fn empty_element_ids_are_rejected() {
    let result = ElementId::new("   ");

    assert!(matches!(&result, Err(DataModelError::InvalidId { .. })));
    if let Err(error) = result {
        assert_eq!("data-model ID must not be empty", error.message());
    }
}

#[test]
fn element_id_deserialization_uses_validation() -> Result<(), Box<dyn std::error::Error>> {
    let id: ElementId = serde_json::from_str("\"rr:valid\"")?;

    assert_eq!("rr:valid", id.as_str());
    assert!(serde_json::from_str::<ElementId>("\"   \"").is_err());
    Ok(())
}

#[test]
fn sha256_hashes_require_supported_prefix_and_hex_length() {
    let valid = ContentHash::parse(
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );
    assert!(valid.is_ok());

    let invalid_values = [
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:not-hex",
    ];

    for invalid in invalid_values {
        let result = ContentHash::parse(invalid);

        assert!(matches!(&result, Err(DataModelError::InvalidHash { .. })));
        if let Err(error) = result {
            assert_eq!(
                "content hash must use sha256:<64 lowercase hex chars>",
                error.message()
            );
        }
    }
}

#[test]
fn content_hash_deserialization_uses_validation() -> Result<(), Box<dyn std::error::Error>> {
    let hash: ContentHash = serde_json::from_str(
        "\"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"",
    )?;

    assert_eq!(
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        hash.as_str()
    );
    assert!(serde_json::from_str::<ContentHash>("\"sha256:not-hex\"").is_err());
    Ok(())
}
```

### Range Tests

```rust
// crates/rr-data-model/tests/range_tests.rs
use rr_data_model::{DataModelError, Range, RangeShape};

#[test]
fn single_line_ranges_are_half_open_and_zero_width_is_allowed()
-> Result<(), Box<dyn std::error::Error>> {
    let range = Range::single_line(2, 4, 4)?;

    assert_eq!(RangeShape::SingleLine, range.shape);
    assert_eq!(2, range.start_line);
    assert_eq!(4, range.start_character);
    assert_eq!(2, range.end_line);
    assert_eq!(4, range.end_character);
    Ok(())
}

#[test]
fn invalid_single_line_ranges_are_rejected() {
    let result = Range::single_line(2, 7, 4);

    assert!(matches!(&result, Err(DataModelError::InvalidRange { .. })));
    if let Err(error) = result {
        assert_eq!(
            "range must be a valid half-open zero-based source range",
            error.message()
        );
    }
}

#[test]
fn range_deserialization_uses_validation() {
    let reversed_single_line = r#"{
        "shape": "single_line",
        "start_line": 2,
        "start_character": 7,
        "end_line": 2,
        "end_character": 4
    }"#;
    let mismatched_single_line = r#"{
        "shape": "single_line",
        "start_line": 2,
        "start_character": 4,
        "end_line": 3,
        "end_character": 8
    }"#;

    assert!(serde_json::from_str::<Range>(reversed_single_line).is_err());
    assert!(serde_json::from_str::<Range>(mismatched_single_line).is_err());
}

#[test]
fn multi_line_ranges_preserve_typed_shape() -> Result<(), Box<dyn std::error::Error>> {
    let range = Range::multi_line(1, 3, 4, 8)?;

    assert_eq!(RangeShape::MultiLine, range.shape);
    assert_eq!(1, range.start_line);
    assert_eq!(3, range.start_character);
    assert_eq!(4, range.end_line);
    assert_eq!(8, range.end_character);
    Ok(())
}
```

### Trait Tests

```rust
// crates/rr-data-model/tests/trait_tests.rs
use rr_data_model::{DataModelError, ElementId, TryIntoDataModel};

struct RawElementId<'a>(&'a str);

impl<'a> TryIntoDataModel<ElementId> for RawElementId<'a> {
    type Error = DataModelError;

    fn try_into_data_model(self) -> Result<ElementId, Self::Error> {
        ElementId::new(self.0)
    }
}

#[test]
fn conversion_trait_can_be_implemented_for_local_adapter_outside_the_crate()
-> Result<(), Box<dyn std::error::Error>> {
    let id = RawElementId("rr:test").try_into_data_model()?;

    assert_eq!("rr:test", id.as_str());
    Ok(())
}
```

### Serialization Tests

```rust
// crates/rr-data-model/tests/serialization_tests.rs
use rr_data_model::{
    schema_version::SCHEMA_VERSION, ArtifactDb, ContentHash, Document, DocumentEncoding,
    ElementId, Occurrence, PositionEncoding, Range, ScanIndex, ScanMetadata, ScanResponse,
    ScanRun, SymbolKind, WorkItem, WorkManifest,
};
use serde_json::Value;

#[test]
fn artifact_db_serializes_schema_version() -> Result<(), Box<dyn std::error::Error>> {
    let db = ArtifactDb::empty();
    let value = serde_json::to_value(db)?;

    assert_eq!(
        Some(SCHEMA_VERSION as u64),
        value.get("schema_version").and_then(Value::as_u64)
    );
    Ok(())
}

#[test]
fn scan_response_round_trip_preserves_required_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let run = ScanRun {
        run_id: "run-1".to_owned(),
        scanner_key: "rust".to_owned(),
        target_path: ".".to_owned(),
        started_at: None,
        finished_at: None,
        command: vec!["rr-scip-rust".to_owned(), "scan".to_owned()],
    };
    let metadata = ScanMetadata {
        source_schema_version: Some("SCIP UnspecifiedProtocolVersion".to_owned()),
        scanner_tool_name: "rust-analyzer".to_owned(),
        scanner_tool_version: Some("dev".to_owned()),
        scanner_tool_arguments: Vec::new(),
        project_root: ".".to_owned(),
        document_encoding: DocumentEncoding::Utf8,
    };
    let index = ScanIndex {
        metadata,
        project: rr_data_model::Project {
            project_key: "fixture".to_owned(),
            project_root: ".".to_owned(),
            scanner_key: "rust".to_owned(),
        },
        documents: vec![Document {
            relative_path: "src/lib.rs".to_owned(),
            language: "Rust".to_owned(),
            position_encoding: PositionEncoding::Utf8CodeUnitOffsetFromLineStart,
            text: None,
            symbols: Vec::new(),
            occurrences: Vec::new(),
            content_hash: None,
        }],
        external_symbols: Vec::new(),
    };
    let response = ScanResponse::new(run, index);

    let json = serde_json::to_string(&response)?;
    let restored: ScanResponse = serde_json::from_str(&json)?;
    let first_document = restored.index.documents.first();

    assert_eq!(SCHEMA_VERSION, restored.schema_version);
    assert_eq!(
        Some("src/lib.rs"),
        first_document.map(|document| document.relative_path.as_str())
    );
    Ok(())
}

#[test]
fn occurrence_deserialization_defaults_missing_role_bits()
-> Result<(), Box<dyn std::error::Error>> {
    let occurrence: Occurrence = serde_json::from_str(r#"{
        "range": {
            "shape": "single_line",
            "start_line": 0,
            "start_character": 0,
            "end_line": 0,
            "end_character": 4
        }
    }"#)?;

    assert_eq!(0, occurrence.symbol_roles.bits);
    Ok(())
}

#[test]
fn work_manifest_serializes_work_item_paths_and_ids()
-> Result<(), Box<dyn std::error::Error>> {
    let source_hash = ContentHash::parse(
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )?;
    let element_id = ElementId::new("rr:fixture:src/lib.rs:public_sum")?;
    let locator = rr_data_model::ElementLocator {
        document_path: "src/lib.rs".to_owned(),
        source_symbol: "rust-analyzer cargo fixture 0.1.0 fixture/public_sum().".to_owned(),
        definition_range: Range::single_line(0, 0, 10)?,
        enclosing_range: None,
    };
    let work_item = WorkItem {
        work_item_id: "work-1".to_owned(),
        document_path: "src/lib.rs".to_owned(),
        element_id,
        aspect_id: "brief".to_owned(),
        target_artifact_path: ".refactor-radar/summaries/src/lib/public_sum.md".to_owned(),
        source_slice_hash: source_hash,
        scanner_evidence: locator,
    };
    let mut manifest = WorkManifest::new("manifest-1", "run-1");
    manifest.work_items.push(work_item);

    let value = serde_json::to_value(manifest)?;
    let first_item = value
        .get("work_items")
        .and_then(Value::as_array)
        .and_then(|items| items.first());

    assert_eq!(Some("src/lib.rs"), first_item
        .and_then(|item| item.get("document_path"))
        .and_then(Value::as_str));
    assert_eq!(Some("brief"), first_item
        .and_then(|item| item.get("aspect_id"))
        .and_then(Value::as_str));
    assert_eq!(SymbolKind::unspecified().name, "UnspecifiedKind");
    Ok(())
}
```

## Validation

Run formatting and the focused crate checks:

```bash
cargo fmt --package rr-data-model
cargo test -p rr-data-model
cargo check -p rr-data-model
```

Run the one-type-per-file audit:

```bash
violations="$(
  rg -n '^(pub(\([^)]*\))?\s+)?(struct|enum|trait)\s+|^(pub(\([^)]*\))?\s+)?type\s+[A-Z][A-Za-z0-9_]*\s*=' crates/rr-data-model/src/*.rs \
    | cut -d: -f1 \
    | sort \
    | uniq -c \
    | awk '$1 > 1 {print}'
)"
test -z "$violations" || {
  printf 'more than one top-level type in file:\n%s\n' "$violations"
  exit 1
}
```

Check that `rr-data-model` has no forbidden dependencies:

```bash
cargo tree -p rr-data-model --prefix none \
  | rg '^(scip|protobuf|protobuf-json-mapping|rmcp|tokio|rr-core|rr-scip|rr-scip-rust|rr-scip-dotnet)($| )' \
  && exit 1 || true
```

Check that no submodule was modified:

```bash
git status --short submodules
```

Expected output:

```text
```

## Non-Goals

Do not implement any of these in VS-01:

```text
parse SCIP
run rust-analyzer
run scip-dotnet
compute file hashes
compute element source-slice hashes
compare artifact DB entries
schedule summarizer work from hash gates
publish artifacts
expose MCP tools
add optional future crates
edit submodules
```

## Unknowns For Later Slices

These remain intentionally unresolved until the scanner/core slices have
runtime evidence:

```text
exact deterministic ElementId formula
whether ArtifactAspect needs multiple summary aspect kinds on day one
whether external SCIP symbols should become elements or only Symbol evidence
whether scanner timestamps should stay stringly typed or gain a time crate
whether JSON schema files should be emitted as checked-in artifacts
```
