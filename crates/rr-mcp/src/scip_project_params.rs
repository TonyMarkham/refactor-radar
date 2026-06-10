use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct ScipProjectParams {
    pub path: String,
    pub format: Option<String>,
}
