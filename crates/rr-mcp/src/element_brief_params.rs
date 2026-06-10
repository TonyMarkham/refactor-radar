use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ElementBriefParams {
    pub symbol_id: String,
    pub project_id: Option<String>,
    pub include_inferred: Option<bool>,
    pub reference_limit: Option<usize>,
}
