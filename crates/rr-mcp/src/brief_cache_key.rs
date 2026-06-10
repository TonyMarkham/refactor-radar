use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BriefCacheKey {
    pub project_id: String,
    pub symbol_id: String,
    pub evidence_hash: String,
    pub prompt_version: String,
    pub brief_model: String,
}
