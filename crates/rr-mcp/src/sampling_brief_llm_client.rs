use crate::{
    BriefLlmClient, BriefLlmRequest, GENERATED_BRIEF_USER_PROMPT_TEMPLATE, GeneratedBriefContent,
    McpError, McpResult,
};

use rmcp::{
    Peer, RoleServer,
    model::{
        ContextInclusion, CreateMessageRequestParams, CreateMessageResult, ModelHint,
        ModelPreferences, Role, SamplingMessage, SamplingMessageContent,
    },
};

#[derive(Clone, Debug)]
pub struct SamplingBriefLlmClient {
    peer: Peer<RoleServer>,
}

impl SamplingBriefLlmClient {
    pub fn new(peer: Peer<RoleServer>) -> Self {
        Self { peer }
    }
}

#[async_trait::async_trait]
impl BriefLlmClient for SamplingBriefLlmClient {
    async fn generate(
        &self,
        request: BriefLlmRequest,
    ) -> McpResult<(GeneratedBriefContent, String)> {
        let supports_sampling = self
            .peer
            .peer_info()
            .map(|info| {
                info.capabilities.sampling.is_some()
                    || info
                        .capabilities
                        .tasks
                        .as_ref()
                        .map(|tasks| tasks.supports_sampling_create_message())
                        .unwrap_or(false)
            })
            .unwrap_or(false);

        if !supports_sampling {
            return Err(McpError::brief_sampling_unavailable());
        }

        let evidence_json = serde_json::to_string_pretty(&request.evidence_packet)
            .map_err(|error| McpError::serialization_failed(error.to_string()))?;
        let user_prompt =
            GENERATED_BRIEF_USER_PROMPT_TEMPLATE.replace("{evidence_json}", &evidence_json);

        let mut params = CreateMessageRequestParams::new(
            vec![SamplingMessage::user_text(user_prompt)],
            request.max_tokens,
        )
        .with_system_prompt(request.instructions)
        .with_include_context(ContextInclusion::None)
        .with_temperature(0.0);

        if let Some(model_hint) = request.model_hint {
            params = params.with_model_preferences(
                ModelPreferences::new().with_hints(vec![ModelHint::new(model_hint)]),
            );
        }

        let result = self
            .peer
            .create_message(params)
            .await
            .map_err(|error| McpError::brief_generation_request_failed(error.to_string()))?;
        let output_text = assistant_text(&result).ok_or_else(|| {
            McpError::brief_generation_response_invalid(
                "sampling response did not contain assistant text",
            )
        })?;
        let content = parse_generated_brief_content(output_text)?;

        Ok((content, result.model))
    }
}

fn assistant_text(result: &CreateMessageResult) -> Option<&str> {
    if result.message.role != Role::Assistant {
        return None;
    }

    result
        .message
        .content
        .iter()
        .find_map(|content| match content {
            SamplingMessageContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
}

fn parse_generated_brief_content(output_text: &str) -> McpResult<GeneratedBriefContent> {
    serde_json::from_str::<GeneratedBriefContent>(output_text)
        .map_err(|error| McpError::brief_generation_response_invalid(error.to_string()))
}

// ---------------------------------------------------------------------------------------------- //

#[cfg(test)]
#[test]
fn given_invalid_sampling_json_when_parsing_then_typed_error_is_returned() {
    let error = parse_generated_brief_content("not json").err();

    assert!(matches!(
        error,
        Some(McpError::BriefGenerationResponseInvalid { .. })
    ));
}
