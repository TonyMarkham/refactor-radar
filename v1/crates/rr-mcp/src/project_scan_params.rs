use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectScanParams {
    pub path: String,
    pub output_path: Option<String>,
    pub config_path: Option<String>,
}
