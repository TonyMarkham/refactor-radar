use crate::{ElementSummary, EvidenceRecord, FiveWSummary};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ElementBrief {
    pub element: ElementSummary,
    pub five_w: FiveWSummary,
    pub evidence: Vec<EvidenceRecord>,
    pub confirmed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}
