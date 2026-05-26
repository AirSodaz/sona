import {
  DEFAULT_SUMMARY_TEMPLATE_ID,
  type HistorySummaryPayload,
  type TranscriptSummaryState,
} from '../types/transcript';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { historyService } from './historyService';

// Once we have local state, prefer it over re-hydrating from disk. This prevents a late
// sidecar read from clobbering in-memory edits, streaming text, or template switches.
function hasStoredSummaryState(summaryState: TranscriptSummaryState | undefined): boolean {
  if (!summaryState) {
    return false;
  }

  return (
    summaryState.isGenerating ||
    summaryState.generationProgress > 0 ||
    !!summaryState.streamingContent ||
    !!summaryState.record ||
    summaryState.activeTemplateId !== DEFAULT_SUMMARY_TEMPLATE_ID
  );
}

function buildSummaryPayload(summaryState: TranscriptSummaryState): HistorySummaryPayload {
  return {
    activeTemplateId: summaryState.activeTemplateId,
    record: summaryState.record,
  };
}

// We intentionally persist only durable summary state. Empty/default state should delete
// the sidecar so opening a transcript without summary data stays equivalent to "no file".
function hasPersistableSummaryData(summaryState: TranscriptSummaryState): boolean {
  return (
    !!summaryState.record ||
    summaryState.activeTemplateId !== DEFAULT_SUMMARY_TEMPLATE_ID
  );
}

class SummarySidecarService {
  async loadSummaryPayload(historyId: string): Promise<HistorySummaryPayload | null> {
    return historyService.loadSummary(historyId);
  }

  async loadSummary(historyId: string): Promise<void> {
    if (!historyId) {
      return;
    }

    const existingState = useTranscriptSidecarStore.getState().summaryStates[historyId];
    if (hasStoredSummaryState(existingState)) {
      return;
    }

    const payload = await this.loadSummaryPayload(historyId);
    const latestState = useTranscriptSidecarStore.getState().summaryStates[historyId];
    if (hasStoredSummaryState(latestState)) {
      return;
    }

    if (payload) {
      useTranscriptSidecarStore.getState().hydrateSummaryState(payload, historyId);
    }
  }

  async persistSummary(historyId: string): Promise<void> {
    if (!historyId || historyId === 'current') {
      return;
    }

    const storedSummaryState = useTranscriptSidecarStore.getState().summaryStates[historyId];
    if (!storedSummaryState) {
      return;
    }

    const summaryState = useTranscriptSidecarStore.getState().getSummaryState(historyId);
    if (!hasPersistableSummaryData(summaryState)) {
      await historyService.deleteSummary(historyId);
      return;
    }

    await historyService.saveSummary(historyId, buildSummaryPayload(summaryState));
  }
}

export const summarySidecarService = new SummarySidecarService();
