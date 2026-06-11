use crate::SourceSpan;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeReference {
    pub display_text: String,
    pub scip_symbol: Option<String>,
    pub source_span: Option<SourceSpan>,
    pub signature_range: Option<Vec<u32>>,
    pub confidence: String,
}
