pub mod decode;
mod error;
mod scip;

pub use decode::{decode_scip_from_bytes, decode_scip_from_path};
pub use error::{error::ScipError, result::ScipResult};
pub use scip::{
    Descriptor, Diagnostic, DiagnosticTag, Document, Index, Language, Metadata, MultiLineRange,
    Occurrence, Package, PositionEncoding, ProtocolVersion, Relationship, Severity, Signature,
    SingleLineRange, Symbol, SymbolInformation, SymbolRole, SyntaxKind, TextEncoding, ToolInfo,
    descriptor::Suffix as DescriptorSuffix,
    occurrence::{
        TypedEnclosingRange as OccurrenceTypedEnclosingRange, TypedRange as OccurrenceTypedRange,
    },
    symbol_information::Kind as SymbolInformationKind,
};
