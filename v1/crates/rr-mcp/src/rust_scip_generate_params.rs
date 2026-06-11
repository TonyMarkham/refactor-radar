use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RustScipGenerateParams {
    pub path: String,
    pub output_path: String,
    pub config_path: Option<String>,
    pub exclude_vendored_libraries: Option<bool>,
    pub num_threads: Option<usize>,
}
