use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefTaskOutputSchema {
    pub format: String,
    pub required_fields: Vec<String>,
    pub json_schema: serde_json::Value,
}
