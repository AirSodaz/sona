import type {
  PolishedSegment,
  PolishSegmentsRequest,
  SummarizeTranscriptRequest,
  TranscriptSummaryResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from '../llmTaskService';
import type { LlmGenerateCommandRequest } from '../../types/dashboard';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function generateLlmText(request: LlmGenerateCommandRequest): Promise<string> {
  return invokeTauri(TauriCommand.llm.generateText, { request });
}

export async function listLlmModels(request: {
  provider: string;
  baseUrl?: string;
  apiKey?: string;
}): Promise<string[]> {
  return invokeTauri(TauriCommand.llm.listModels, { request });
}

export async function polishTranscriptSegments(
  request: PolishSegmentsRequest,
): Promise<PolishedSegment[]> {
  return invokeTauri(TauriCommand.llm.polishTranscriptSegments, { request });
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
