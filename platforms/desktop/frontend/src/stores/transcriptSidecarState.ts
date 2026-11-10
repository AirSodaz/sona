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
  errorMessage?: string;
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
  isTranslationVisible: true,
  isPolishing: false,
  polishProgress: 0,
  isRetranscribing: false,
  retranscribeProgress: 0,
};

export const DEFAULT_SUMMARY_STATE: TranscriptSummaryState = {
  activeTemplateId: DEFAULT_SUMMARY_TEMPLATE_ID as SummaryTemplateId,
  record: undefined,
  streamingContent: undefined,
  streamingThought: undefined,
  isGenerating: false,
  generationProgress: 0,
};

export function createDefaultSummaryState(): TranscriptSummaryState {
  return {
    ...DEFAULT_SUMMARY_STATE,
  };
}

export function toSessionSidecarKey(sessionId: string): string {
  return `session:${sessionId || 'default'}`;
}

export function resolveSidecarKey(sessionId: string, historyId?: string | null): string {
  if (historyId && historyId !== 'current') {
    return historyId;
  }
  return toSessionSidecarKey(sessionId);
}

export function resolveTranscriptHistoryKey(
  explicitHistoryId: string | undefined,
  sourceHistoryId: string | null,
  sessionId = 'default'
): string {
  if (explicitHistoryId && explicitHistoryId !== 'current') {
    return explicitHistoryId;
  }
  if (sourceHistoryId && sourceHistoryId !== 'current') {
    return sourceHistoryId;
  }
  return toSessionSidecarKey(sessionId);
}

export function rekeySidecarMapEntry<T>(
  map: Record<string, T>,
  fromKey: string,
  nextHistoryId: string,
  mergeEntries: (existing: T, migrated: T) => T
): Record<string, T> {
  if (!nextHistoryId || nextHistoryId === 'current' || fromKey === nextHistoryId) {
    return map;
  }
  const migrated = map[fromKey];
  if (!migrated) {
    return map;
  }

  const next = { ...map };
  const existing = next[nextHistoryId];
  next[nextHistoryId] = existing ? mergeEntries(existing, migrated) : migrated;
  delete next[fromKey];
  return next;
}

export function rekeySessionSidecarEntries(
  state: TranscriptHistorySidecarState,
  sessionId: string,
  nextHistoryId: string | null
): TranscriptHistorySidecarState {
  if (!nextHistoryId || nextHistoryId === 'current') {
    return state;
  }
  const sessionKey = toSessionSidecarKey(sessionId);
  let nextLlmStates = rekeySidecarMapEntry(
    state.llmStates,
    sessionKey,
    nextHistoryId,
    (existing, migrated) => ({
      ...existing,
      ...migrated,
      isPolishing: existing.isPolishing || migrated.isPolishing,
      isTranslating: existing.isTranslating || migrated.isTranslating,
      isRetranscribing: existing.isRetranscribing || migrated.isRetranscribing,
      isTranslationVisible: existing.isTranslationVisible || migrated.isTranslationVisible,
      polishProgress: Math.max(existing.polishProgress, migrated.polishProgress),
      translationProgress: Math.max(existing.translationProgress, migrated.translationProgress),
      retranscribeProgress: Math.max(existing.retranscribeProgress, migrated.retranscribeProgress),
    })
  );
  nextLlmStates = rekeySidecarMapEntry(
    nextLlmStates,
    'current',
    nextHistoryId,
    (existing, migrated) => ({
      ...existing,
      ...migrated,
      isPolishing: existing.isPolishing || migrated.isPolishing,
      isTranslating: existing.isTranslating || migrated.isTranslating,
      isRetranscribing: existing.isRetranscribing || migrated.isRetranscribing,
    })
  );

  let nextSummaryStates = rekeySidecarMapEntry(
    state.summaryStates,
    sessionKey,
    nextHistoryId,
    (existing, migrated) => ({
      ...existing,
      ...migrated,
      record: migrated.record || existing.record,
    })
  );
  nextSummaryStates = rekeySidecarMapEntry(
    nextSummaryStates,
    'current',
    nextHistoryId,
    (existing, migrated) => ({
      ...existing,
      ...migrated,
      record: migrated.record || existing.record,
    })
  );

  let nextAutoSaveStates = rekeySidecarMapEntry(
    state.autoSaveStates,
    sessionKey,
    nextHistoryId,
    (existing, migrated) => (migrated.updatedAt >= existing.updatedAt ? migrated : existing)
  );
  nextAutoSaveStates = rekeySidecarMapEntry(
    nextAutoSaveStates,
    'current',
    nextHistoryId,
    (existing, migrated) => (migrated.updatedAt >= existing.updatedAt ? migrated : existing)
  );

  if (
    nextLlmStates === state.llmStates &&
    nextSummaryStates === state.summaryStates &&
    nextAutoSaveStates === state.autoSaveStates
  ) {
    return state;
  }

  return {
    llmStates: nextLlmStates,
    summaryStates: nextSummaryStates,
    autoSaveStates: nextAutoSaveStates,
  };
}

export function rekeyCurrentSummaryState(
  summaryStates: Record<string, TranscriptSummaryState>,
  nextHistoryId: string | null,
  sessionId?: string
): Record<string, TranscriptSummaryState> {
  if (!nextHistoryId || nextHistoryId === 'current') {
    return summaryStates;
  }
  let next = summaryStates;
  if (sessionId) {
    next = rekeySidecarMapEntry(
      next,
      toSessionSidecarKey(sessionId),
      nextHistoryId,
      (existing, migrated) => ({
        ...existing,
        ...migrated,
        record: migrated.record || existing.record,
      })
    );
  }
  return rekeySidecarMapEntry(next, 'current', nextHistoryId, (existing, migrated) => ({
    ...existing,
    ...migrated,
    record: migrated.record || existing.record,
  }));
}
