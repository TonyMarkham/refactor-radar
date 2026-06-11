use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct RustScipGenerateResult {
    pub path: String,
    pub stderr_digest: String,
    pub producer_name: Option<String>,
    pub producer_version: Option<String>,
    pub file_size: u64,
}
