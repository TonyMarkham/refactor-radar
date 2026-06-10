use crate::{SourceSpan, SymbolId};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallEdge {
    pub enclosing_symbol_id: SymbolId,
    pub referenced_symbol_id: SymbolId,
    pub evidence_span: SourceSpan,
    pub confidence: String,
}
