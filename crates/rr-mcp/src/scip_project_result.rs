use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ScipProjectResult {
    pub project_id: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
