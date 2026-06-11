use crate::{SourceSpan, SymbolId};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolReference {
    pub referenced_symbol_id: SymbolId,
    pub source_span: SourceSpan,
    pub document_path: String,
    pub symbol_roles: i32,
}
