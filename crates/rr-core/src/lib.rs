pub mod element_brief;
pub mod element_kind;
pub mod error;
pub mod evidence_record;
pub mod ids;
pub mod language;
pub mod result;
pub mod semantic_model;
pub mod source_span;
pub mod summaries;

pub use element_brief::ElementBrief;
pub use element_kind::ElementKind;
pub use error::CoreError;
pub use evidence_record::EvidenceRecord;
pub use ids::{span::Span as SpanId, stable::Stable as StableId, symbol::Symbol as SymbolId};
pub use language::Language;
pub use result::CoreResult;
pub use semantic_model::SemanticModel;
pub use source_span::SourceSpan;
pub use summaries::{
    call_edge::CallEdge as CallEdgeSummary, element::Element as ElementSummary,
    file::File as FileSummary, five_w::FiveW as FiveWSummary,
    function_parameter::FunctionParameter as FunctionParameterSummary,
    function_signature::FunctionSignature as FunctionSignatureSummary,
    project::Project as ProjectSummary, project_report::ProjectReport as ProjectReportSummary,
    symbol_reference::SymbolReference as SymbolReferenceSummary,
    type_reference::TypeReference as TypeReferenceSummary,
};
