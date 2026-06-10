use crate::{BriefTaskOutputSchema, BriefTaskToolCall};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefWorkTask {
    pub task_id: String,
    pub title: String,
    pub preferred_agent: String,
    pub scope: String,
    pub symbol_ids: Vec<String>,
    pub tool_calls: Vec<BriefTaskToolCall>,
    pub expected_output_schema: BriefTaskOutputSchema,
    pub merge_key: String,
    pub budget_tokens: usize,
    pub dependencies: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}
