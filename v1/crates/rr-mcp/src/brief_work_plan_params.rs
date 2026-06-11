use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BriefWorkPlanParams {
    pub project_id: String,
    pub symbol_ids: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub reference_limit: Option<usize>,
    pub include_inferred: Option<bool>,
    pub budget_tokens: Option<usize>,
    pub max_subagent_tasks: Option<usize>,
    pub preferred_agent: Option<String>,
    pub brief_model: Option<String>,
}
