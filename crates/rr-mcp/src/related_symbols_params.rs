use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct RelatedSymbolsParams {
    pub symbol_id: String,
    pub limit: Option<usize>,
}
