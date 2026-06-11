use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectScanResult {
    pub project_id: String,
    pub output_path: String,
    pub document_count: usize,
    pub symbol_count: usize,
    pub occurrence_count: usize,
}
