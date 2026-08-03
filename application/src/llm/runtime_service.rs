use sona_core::llm::provider_protocol::LlmModelSummary;
use sona_core::llm::requests::{LlmConfig, LlmModelsRequest};
use sona_core::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionRequest, LlmCompletionResponse, LlmResponseFormat,
    LlmResponseFormatKind, LlmRuntimeError, LlmStreamDelta, finish_response,
    validate_completion_request,
};
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortError, LlmStreamingPort,
};

pub struct LlmRuntimeService<'a, Completion, Metadata> {
    completion: &'a Completion,
    metadata: Metadata,
}

impl<'a, Completion, Metadata> LlmRuntimeService<'a, Completion, Metadata>
where
    Completion: LlmCompletionPort,
    Metadata: LlmModelMetadataPort,
{
    pub fn new(completion: &'a Completion, metadata: Metadata) -> Self {
        Self {
            completion,
            metadata,
        }
    }

    pub async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmRuntimeError> {
        let prepared = self.prepare_request(request).await?;
        let applied_format = prepared.request.options.response_format.clone();
        let response = self.completion.complete(prepared.request).await?;
        finish_response(
            response,
            prepared.requested_format,
            applied_format,
            prepared.validation_format,
            prepared.warnings,
        )
    }

    pub async fn stream(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<LlmCompletionResponse, LlmRuntimeError>
    where
        Completion: LlmStreamingPort,
    {
        let prepared = self.prepare_request(request).await?;
        let applied_format = prepared.request.options.response_format.clone();
        let response = self
            .completion
            .stream_completion(prepared.request, emit_delta)
            .await?;
        finish_response(
            response,
            prepared.requested_format,
            applied_format,
            prepared.validation_format,
            prepared.warnings,
        )
    }

    async fn prepare_request(
        &self,
        mut request: LlmCompletionRequest,
    ) -> Result<PreparedRequest, LlmRuntimeError> {
        validate_completion_request(&mut request)?;

        let requested_format = LlmResponseFormatKind::from(&request.options.response_format);
        let validation_format = request.options.response_format.clone();
        let mut warnings = Vec::new();
        let requested_schema = match &request.options.response_format {
            LlmResponseFormat::JsonSchema { schema, .. } => Some(schema.clone()),
            _ => None,
        };
        if let Some(schema) = requested_schema
            && self
                .metadata
                .describe_model(&request.config)
                .await?
                .and_then(|model| model.supports_structured_output)
                == Some(false)
        {
            if request.options.capability_policy == LlmCapabilityPolicy::Strict {
                return Err(LlmRuntimeError::UnsupportedCapability {
                    model: request.config.model.clone(),
                    capability: "structured output".to_string(),
                });
            }
            request
                .input
                .push_str("\n\nReturn only a JSON object that satisfies this JSON Schema:\n");
            request.input.push_str(&schema.to_string());
            request.options.response_format = LlmResponseFormat::JsonObject;
            warnings.push(
                "Model metadata reports no structured output support; using JSON object mode"
                    .to_string(),
            );
        }

        Ok(PreparedRequest {
            request,
            requested_format,
            validation_format,
            warnings,
        })
    }

    pub async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmRuntimeError>
    where
        Completion: LlmModelDiscoveryPort,
    {
        self.completion
            .list_models(request)
            .await
            .map_err(Into::into)
    }

    pub async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmRuntimeError> {
        self.metadata
            .describe_model(config)
            .await
            .map_err(Into::into)
    }
}

struct PreparedRequest {
    request: LlmCompletionRequest,
    requested_format: LlmResponseFormatKind,
    validation_format: LlmResponseFormat,
    warnings: Vec<String>,
}
