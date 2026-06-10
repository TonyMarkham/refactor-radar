use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BriefGenerationCapabilities {
    pub sampling_supported: bool,
    pub legacy_sampling_supported: bool,
    pub task_sampling_create_message_supported: bool,
    pub generate_llm_brief_available: bool,
    pub deterministic_work_plan_available: bool,
    pub fallback_tool: String,
    pub unsupported_error_message: String,
    pub notes: Vec<String>,
}
