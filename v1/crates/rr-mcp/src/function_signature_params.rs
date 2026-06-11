use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct FunctionSignatureParams {
    pub symbol_id: String,
}
