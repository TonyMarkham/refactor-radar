use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct SourceSpanParams {
    pub span_id: String,
}
