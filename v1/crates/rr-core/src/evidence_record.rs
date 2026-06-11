use crate::SourceSpan;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub kind: String,
    pub confidence_label: String,
    pub source_spans: Vec<SourceSpan>,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}
