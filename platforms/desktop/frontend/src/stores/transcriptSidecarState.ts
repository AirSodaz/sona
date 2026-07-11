import {
  DEFAULT_SUMMARY_TEMPLATE_ID,
  type SummaryTemplateId,
  type TranscriptSummaryState,
} from '../types/transcript';

export interface LlmState {
  isTranslating: boolean;
  translationProgress: number;
  isTranslationVisible: boolean;
  isPolishing: boolean;
  polishProgress: number;
  isRetranscribing: boolean;
  retranscribeProgress: number;
}

export type AutoSaveStatus = 'saving' | 'saved' | 'error';

export interface AutoSaveState {
  status: AutoSaveStatus;
  updatedAt: number;
}

export interface TranscriptHistorySidecarState {
  llmStates: Record<string, LlmState>;
  summaryStates: Record<string, TranscriptSummaryState>;
  autoSaveStates: Record<string, AutoSaveState>;
}

export const INITIAL_TRANSCRIPT_HISTORY_SIDECAR_STATE: TranscriptHistorySidecarState = {
  llmStates: {},
  summaryStates: {},
  autoSaveStates: {},
};

export const DEFAULT_LLM_STATE: LlmState = {
  isTranslating: false,
  translationProgress: 0,
  isTranslationVisible: false,
  isPolishing: false,
  polishProgress: 0,
  isRetranscribing: false,
  retranscribeProgress: 0,
};

export const DEFAULT_SUMMARY_STATE: TranscriptSummaryState = {
  activeTemplateId: DEFAULT_SUMMARY_TEMPLATE_ID as SummaryTemplateId,
  record: undefined,
  streamingContent: undefined,
  isGenerating: false,
  generationProgress: 0,
};

export function createDefaultSummaryState(): TranscriptSummaryState {
  return {
    ...DEFAULT_SUMMARY_STATE,
  };
}

export function resolveTranscriptHistoryKey(
  explicitHistoryId: string | undefined,
  sourceHistoryId: string | null,
): string {
  return explicitHistoryId || sourceHistoryId || 'current';
}

export function rekeyCurrentSummaryState(
  summaryStates: Record<string, TranscriptSummaryState>,
  nextHistoryId: string | null,
): Record<string, TranscriptSummaryState> {
  if (!nextHistoryId || !summaryStates.current) {
    return summaryStates;
  }

  const currentSummaryState = summaryStates.current;
  const existingTargetState = summaryStates[nextHistoryId];
  const nextSummaryStates = { ...summaryStates };

  nextSummaryStates[nextHistoryId] = existingTargetState
    ? {
      ...existingTargetState,
      ...currentSummaryState,
      record: currentSummaryState.record || existingTargetState.record,
    }
    : currentSummaryState;

  delete nextSummaryStates.current;
  return nextSummaryStates;
}
