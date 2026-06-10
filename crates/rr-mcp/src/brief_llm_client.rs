use crate::{BriefLlmRequest, GeneratedBriefContent, McpResult};

#[async_trait::async_trait]
pub trait BriefLlmClient: Send + Sync {
    async fn generate(
        &self,
        request: BriefLlmRequest,
    ) -> McpResult<(GeneratedBriefContent, String)>;
}
