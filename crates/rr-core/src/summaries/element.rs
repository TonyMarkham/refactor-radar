use crate::{ElementKind, FunctionSignatureSummary, Language, SourceSpan, StableId, SymbolId};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Element {
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
