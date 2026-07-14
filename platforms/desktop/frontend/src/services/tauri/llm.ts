import type {
  PolishedSegment,
  PolishSegmentsRequest,
  SummarizeTranscriptRequest,
  TranscriptSummaryResult,
  TranscriptLlmJobRequest,
  TranscriptLlmJobResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from '../llmTaskTypes';
import type { LlmGenerateCommandRequest } from '../../types/dashboard';
import type {
  LlmCompletionRequest,
  LlmCompletionResponse,
  LlmConfig,
  LlmDiscoveredModelSummary,
} from '../../types/transcript';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function generateLlmText(request: LlmGenerateCommandRequest): Promise<string> {
  return invokeTauri(TauriCommand.llm.generateText, { request });
}

export async function completeLlm(request: LlmCompletionRequest): Promise<LlmCompletionResponse> {
  return invokeTauri(TauriCommand.llm.complete, { request });
}

export async function describeLlmModel(config: LlmConfig): Promise<LlmDiscoveredModelSummary | null> {
  return invokeTauri(TauriCommand.llm.describeModel, { config });
}

export async function listLlmModels(request: {
  provider: string;
  strategy?: string;
  baseUrl: string;
  apiKey: string;
}): Promise<LlmDiscoveredModelSummary[]> {
  return invokeTauri(TauriCommand.llm.listModels, { request });
}

export async function polishTranscriptSegments(
  request: PolishSegmentsRequest,
): Promise<PolishedSegment[]> {
  return invokeTauri(TauriCommand.llm.polishTranscriptSegments, { request });
}

export async function runTranscriptLlmJob(
  request: TranscriptLlmJobRequest,
): Promise<TranscriptLlmJobResult> {
  return invokeTauri(TauriCommand.llm.runTranscriptJob, { request });
}

export async function summarizeTranscript(
  request: SummarizeTranscriptRequest,
): Promise<TranscriptSummaryResult> {
  return invokeTauri(TauriCommand.llm.summarizeTranscript, { request });
}

export async function translateTranscriptSegments(
  request: TranslateSegmentsRequest,
): Promise<TranslatedSegment[]> {
  return invokeTauri(TauriCommand.llm.translateTranscriptSegments, { request });
}
