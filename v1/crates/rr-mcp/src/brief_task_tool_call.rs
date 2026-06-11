use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BriefTaskToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}
