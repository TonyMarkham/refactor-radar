use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FiveWSummaryParams {
    pub symbol_id: String,
    pub reference_limit: Option<usize>,
}
