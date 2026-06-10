#[derive(Clone, Debug)]
pub struct BriefLlmRequest {
    pub model_hint: Option<String>,
    pub instructions: String,
    pub evidence_packet: serde_json::Value,
    pub max_tokens: u32,
}
