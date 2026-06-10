use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LlmBriefParams {
    pub project_id: String,
    pub budget_tokens: Option<usize>,
}
