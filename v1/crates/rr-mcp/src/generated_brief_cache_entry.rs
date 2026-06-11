use crate::GeneratedBrief;

#[derive(Clone, Debug)]
pub struct GeneratedBriefCacheEntry {
    pub brief: GeneratedBrief,
    pub evidence_hash: String,
    pub prompt_version: String,
    pub model_hint: String,
    pub generated_by_model: String,
}
