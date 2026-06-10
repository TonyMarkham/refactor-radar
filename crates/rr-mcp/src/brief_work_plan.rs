use crate::{BriefGenerationCapabilities, BriefWorkTask};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefWorkPlan {
    pub project_id: String,
    pub prompt_version: String,
    pub execution_model: String,
    pub generation_capabilities: BriefGenerationCapabilities,
    pub parent_instructions: Vec<String>,
    pub tasks: Vec<BriefWorkTask>,
    pub merge_instructions: Vec<String>,
    pub verification_commands: Vec<String>,
    pub unknowns: Vec<String>,
}
