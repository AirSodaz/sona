import { getPlatform, TauriCommand } from '../platform';
import type { TranscriptLlmJobRequest, TranscriptLlmJobResult } from '../types/llmTask';
import { normalizeTranscriptJobRequest } from './llmTransportUtils';

export async function runTranscriptLlmJob(
  request: TranscriptLlmJobRequest
): Promise<TranscriptLlmJobResult> {
  return getPlatform().transport.invoke<TranscriptLlmJobResult>(TauriCommand.llm.runTranscriptJob, {
    request: normalizeTranscriptJobRequest(request),
  });
}
