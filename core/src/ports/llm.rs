use crate::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use crate::llm::requests::{LlmGenerateRequest, LlmModelsRequest};
use async_trait::async_trait;

#[async_trait]
pub trait LlmTextGenerator: Send + Sync {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, String>;
}

#[async_trait]
pub trait LlmModelLister: Send + Sync {
    async fn list_models(&self, request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String>;
}
