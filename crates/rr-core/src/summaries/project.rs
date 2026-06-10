use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub project_id: String,
    pub project_root: String,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
}
