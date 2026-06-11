use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ElementQueryParams {
    pub project_id: String,
    pub kind: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
}
