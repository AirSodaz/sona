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

export interface SummarySidecarServicePorts {
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  historyService: typeof historyService;
}

export class SummarySidecarService {
  constructor(private readonly ports: SummarySidecarServicePorts) {}

  async loadSummaryPayload(historyId: string): Promise<HistorySummaryPayload | null> {
    return this.ports.historyService.loadSummary(historyId);
  }

  async loadSummary(historyId: string): Promise<void> {
    if (!historyId) {
      return;
    }

    const existingState = this.ports.getTranscriptSidecarStore().summaryStates[historyId];
    if (hasStoredSummaryState(existingState)) {
      return;
    }

    const payload = await this.loadSummaryPayload(historyId);
    const latestState = this.ports.getTranscriptSidecarStore().summaryStates[historyId];
    if (hasStoredSummaryState(latestState)) {
      return;
    }

    if (payload) {
      this.ports.getTranscriptSidecarStore().hydrateSummaryState(payload, historyId);
    }
  }

  async persistSummary(historyId: string): Promise<void> {
    if (!historyId || historyId === 'current') {
      return;
    }

    const storedSummaryState = this.ports.getTranscriptSidecarStore().summaryStates[historyId];
    if (!storedSummaryState) {
      return;
    }

    const summaryState = this.ports.getTranscriptSidecarStore().getSummaryState(historyId);
    if (!hasPersistableSummaryData(summaryState)) {
      await this.ports.historyService.deleteSummary(historyId);
      return;
    }

    await this.ports.historyService.saveSummary(historyId, buildSummaryPayload(summaryState));
  }
}

export function createSummarySidecarService(ports: SummarySidecarServicePorts): SummarySidecarService {
  return new SummarySidecarService(ports);
}

export const summarySidecarService = createSummarySidecarService({
  getTranscriptSidecarStore: useTranscriptSidecarStore.getState,
  historyService,
});
