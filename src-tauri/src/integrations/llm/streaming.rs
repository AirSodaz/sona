use super::*;
use log::warn;
use sona_online_llm::try_stream_text_with_provider;

pub(crate) async fn try_stream_text<EmitFn>(
    request: &LlmGenerateRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    try_stream_text_with_provider(request, accumulator).await
}

pub(crate) async fn generate_with_optional_streaming<EmitFn>(
    request: LlmGenerateRequest,
    emit_delta: &mut EmitFn,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let mut accumulator = StreamTextAccumulator::new(emit_delta);
    let stream_result = try_stream_text(&request, &mut accumulator).await;
    let emitted_any = accumulator.emitted_any();
    drop(accumulator);

    match stream_result {
        Ok(Some(response)) => Ok(response),
        Ok(None) => generate_with_rig(request).await,
        Err(error) if !emitted_any => {
            warn!(
                "[LLM] streaming unavailable or failed before first token, falling back to buffered generate: provider={:?} error={}",
                request.config.provider, error
            );
            generate_with_rig(request).await
        }
        Err(error) => Err(error),
    }
}

pub(crate) async fn generate_with_rig(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, String> {
    generate_text_with_provider(request).await
}
