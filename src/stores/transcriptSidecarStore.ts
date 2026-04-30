import { create } from 'zustand';
import type {
  HistorySummaryPayload,
  SummaryTemplateId,
  TranscriptSummaryRecord,
  TranscriptSummaryState,
} from '../types/transcript';
import { coerceSummaryTemplateId } from '../utils/summaryTemplates';
import { getEffectiveConfigSnapshot } from './effectiveConfigStore';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import {
  createDefaultSummaryState,
  DEFAULT_LLM_STATE,
  INITIAL_TRANSCRIPT_HISTORY_SIDECAR_STATE,
  rekeyCurrentSummaryState as rekeyCurrentSummaryStateEntry,
  resolveTranscriptHistoryKey,
  type AutoSaveState,
  type AutoSaveStatus,
  type LlmState,
  type TranscriptHistorySidecarState,
} from './transcriptSidecarState';

type LegacySummaryRecord = Partial<TranscriptSummaryRecord> & {
  template?: string;
};

interface LegacyHistorySummaryPayload extends Omit<HistorySummaryPayload, 'record'> {
  activeTemplate?: string;
  record?: LegacySummaryRecord;
  records?: Record<string, LegacySummaryRecord | undefined>;
}

export interface TranscriptSidecarStore extends TranscriptHistorySidecarState {
  getLlmState: (historyId?: string) => LlmState;
  updateLlmState: (updates: Partial<LlmState>, historyId?: string) => void;
  getSummaryState: (historyId?: string) => TranscriptSummaryState;
  setSummaryState: (summaryState: Partial<TranscriptSummaryState>, historyId?: string) => void;
  updateSummaryState: (updates: Partial<TranscriptSummaryState>, historyId?: string) => void;
  setActiveSummaryTemplate: (templateId: SummaryTemplateId, historyId?: string) => void;
  hydrateSummaryState: (payload: HistorySummaryPayload, historyId?: string) => void;
  clearSummaryState: (historyId?: string) => void;
  rekeyCurrentSummaryState: (nextHistoryId: string | null) => void;
  setAutoSaveState: (historyId: string, status: AutoSaveStatus) => void;
  clearAutoSaveState: (historyId?: string) => void;
}

function resolveHistoryKey(historyId?: string): string {
  return resolveTranscriptHistoryKey(
    historyId,
    useTranscriptSessionStore.getState().sourceHistoryId,
  );
}

export const useTranscriptSidecarStore = create<TranscriptSidecarStore>((set, get) => ({
  ...INITIAL_TRANSCRIPT_HISTORY_SIDECAR_STATE,

  getLlmState: (historyId) => {
    const id = resolveHistoryKey(historyId);
    return get().llmStates[id] || { ...DEFAULT_LLM_STATE };
  },

  updateLlmState: (updates, historyId) => {
    const id = resolveHistoryKey(historyId);
    set((state) => ({
      llmStates: {
        ...state.llmStates,
        [id]: {
          ...(state.llmStates[id] || { ...DEFAULT_LLM_STATE }),
          ...updates,
        },
      },
    }));
  },

  getSummaryState: (historyId) => {
    const id = resolveHistoryKey(historyId);
    return get().summaryStates[id] || createDefaultSummaryState();
  },

  setSummaryState: (summaryState, historyId) => {
    const id = resolveHistoryKey(historyId);
    set((state) => ({
      summaryStates: {
        ...state.summaryStates,
        [id]: {
          ...createDefaultSummaryState(),
          ...summaryState,
          record: summaryState.record,
        },
      },
    }));
  },

  updateSummaryState: (updates, historyId) => {
    const id = resolveHistoryKey(historyId);
    set((state) => ({
      summaryStates: {
        ...state.summaryStates,
        [id]: {
          ...(state.summaryStates[id] || createDefaultSummaryState()),
          ...updates,
        },
      },
    }));
  },

  setActiveSummaryTemplate: (templateId, historyId) => {
    get().updateSummaryState({ activeTemplateId: templateId }, historyId);
  },

  hydrateSummaryState: (payload, historyId) => {
    const customTemplates = getEffectiveConfigSnapshot().summaryCustomTemplates;
    const payloadWithLegacyFields = payload as LegacyHistorySummaryPayload;
    const activeTemplateId = coerceSummaryTemplateId(
      payloadWithLegacyFields.activeTemplateId || payloadWithLegacyFields.activeTemplate,
      customTemplates,
    );

    let record: LegacySummaryRecord | undefined = payloadWithLegacyFields.record;
    if (!record && payloadWithLegacyFields.records) {
      const records = payloadWithLegacyFields.records;
      record = records[
        payloadWithLegacyFields.activeTemplateId
        || payloadWithLegacyFields.activeTemplate
        || activeTemplateId
      ] || Object.values(records)[0];
    }

    const normalizedRecord: TranscriptSummaryRecord | undefined = record
      && typeof record.content === 'string'
      && typeof record.generatedAt === 'string'
      && typeof record.sourceFingerprint === 'string'
      ? {
        content: record.content,
        generatedAt: record.generatedAt,
        sourceFingerprint: record.sourceFingerprint,
        templateId: coerceSummaryTemplateId(
          record.templateId || record.template || activeTemplateId,
          customTemplates,
        ),
      }
      : undefined;

    get().setSummaryState({
      activeTemplateId,
      record: normalizedRecord,
      streamingContent: undefined,
      isGenerating: false,
      generationProgress: 0,
    }, historyId);
  },

  clearSummaryState: (historyId) => {
    const id = resolveHistoryKey(historyId);
    set((state) => {
      if (!state.summaryStates[id]) {
        return state;
      }

      const summaryStates = { ...state.summaryStates };
      delete summaryStates[id];
      return { summaryStates };
    });
  },

  rekeyCurrentSummaryState: (nextHistoryId) => {
    set((state) => ({
      summaryStates: rekeyCurrentSummaryStateEntry(state.summaryStates, nextHistoryId),
    }));
  },

  setAutoSaveState: (historyId, status) => {
    if (!historyId || historyId === 'current') {
      return;
    }

    set((state) => ({
      autoSaveStates: {
        ...state.autoSaveStates,
        [historyId]: {
          status,
          updatedAt: Date.now(),
        },
      },
    }));
  },

  clearAutoSaveState: (historyId) => {
    const id = resolveHistoryKey(historyId);
    set((state) => {
      if (!id || id === 'current' || !state.autoSaveStates[id]) {
        return state;
      }

      const autoSaveStates = { ...state.autoSaveStates };
      delete autoSaveStates[id];
      return { autoSaveStates };
    });
  },
}));

export type { AutoSaveState, AutoSaveStatus, LlmState };
