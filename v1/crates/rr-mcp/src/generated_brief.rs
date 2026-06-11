use crate::{BriefCacheKey, GeneratedFiveW};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeneratedBrief {
    pub summary: String,
    pub five_w: GeneratedFiveW,
    pub unknowns: Vec<String>,
    pub evidence_hash: String,
    pub cache_key: BriefCacheKey,
    pub generated_by_model: String,
    pub prompt_version: String,
}
