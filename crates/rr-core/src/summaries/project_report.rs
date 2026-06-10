use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectReport {
    pub project_id: String,
    pub document_count: usize,
    pub element_count: usize,
    pub reference_count: usize,
    pub hotspot_files: Vec<String>,
}
