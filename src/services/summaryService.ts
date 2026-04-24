import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import {
  DEFAULT_SUMMARY_TEMPLATE,
  HistorySummaryPayload,
  SummaryTemplate,
  TranscriptSegment,
  TranscriptSummaryRecord,
  TranscriptSummaryState,
} from '../types/transcript';
import { computeSummarySourceFingerprint } from '../utils/segmentUtils';
import { historyService } from './historyService';
import { getFeatureLlmConfig, isSummaryLlmConfigComplete } from './llmConfig';
import { normalizeError } from '../utils/errorUtils';
import {
  createLlmTaskId,
  listenToLlmTaskProgress,
  SummarizeTranscriptRequest,
  SummarySegmentInput,
  TranscriptSummaryResult,
} from './llmTaskService';

function hasStoredSummaryState(summaryState: TranscriptSummaryState | undefined): boolean {
  if (!summaryState) {
    return false;
  }

  return (
    summaryState.isGenerating ||
    summaryState.generationProgress > 0 ||
    !!summaryState.record ||
    summaryState.activeTemplate !== DEFAULT_SUMMARY_TEMPLATE
  );
}

function buildSummaryPayload(summaryState: TranscriptSummaryState): HistorySummaryPayload {
  return {
    activeTemplate: summaryState.activeTemplate,
    record: summaryState.record,
  };
}

function hasPersistableSummaryData(summaryState: TranscriptSummaryState): boolean {
  return (
    !!summaryState.record ||
    summaryState.activeTemplate !== DEFAULT_SUMMARY_TEMPLATE
  );
}

export function isSummaryRecordStale(
  record: TranscriptSummaryRecord | undefined,
  segments: TranscriptSegment[],
): boolean {
  if (!record) {
    return false;
  }

  return record.sourceFingerprint !== computeSummarySourceFingerprint(segments);
}

class SummaryService {
  async loadSummary(historyId: string): Promise<void> {
    if (!historyId) {
      return;
    }

    const existingState = useTranscriptStore.getState().summaryStates[historyId];
    if (hasStoredSummaryState(existingState)) {
      return;
    }

    const payload = await historyService.loadSummary(historyId);
    const latestState = useTranscriptStore.getState().summaryStates[historyId];
    if (hasStoredSummaryState(latestState)) {
      return;
    }

    if (payload) {
      useTranscriptStore.getState().hydrateSummaryState(payload, historyId);
    }
  }

  async persistSummary(historyId: string): Promise<void> {
    if (!historyId || historyId === 'current') {
      return;
    }

    const storedSummaryState = useTranscriptStore.getState().summaryStates[historyId];
    if (!storedSummaryState) {
      return;
    }

    const summaryState = useTranscriptStore.getState().getSummaryState(historyId);
    if (!hasPersistableSummaryData(summaryState)) {
      await historyService.deleteSummary(historyId);
      return;
    }

    await historyService.saveSummary(historyId, buildSummaryPayload(summaryState));
  }

  async setActiveTemplate(template: SummaryTemplate, historyId?: string): Promise<void> {
    const store = useTranscriptStore.getState();
    const targetHistoryId = historyId || store.sourceHistoryId || 'current';
    store.setActiveSummaryTemplate(template, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async updateSummaryRecord(content: string, historyId?: string): Promise<void> {
    const store = useTranscriptStore.getState();
    const targetHistoryId = historyId || store.sourceHistoryId || 'current';
    const summaryState = store.getSummaryState(targetHistoryId);

    if (!summaryState.record) {
      return;
    }

    store.updateSummaryState({
      record: {
        ...summaryState.record,
        content,
      },
    }, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async generateSummary(template?: SummaryTemplate): Promise<void> {
    const store = useTranscriptStore.getState();

    if (store.config.summaryEnabled === false) {
      throw new Error('AI Summary is disabled.');
    }

    if (!isSummaryLlmConfigComplete(store.config)) {
      throw new Error('LLM Service not fully configured.');
    }

    const segments = store.segments;
    if (!segments || segments.length === 0) {
      return;
    }

    const activeTemplate = template ?? store.getSummaryState().activeTemplate;
    const jobHistoryId = store.sourceHistoryId || 'current';
    const sourceFingerprint = computeSummarySourceFingerprint(segments);
    const taskId = createLlmTaskId('summary');

    store.updateSummaryState({
      activeTemplate,
      isGenerating: true,
      generationProgress: 0,
    }, jobHistoryId);

    const unlistenProgress = await listenToLlmTaskProgress(taskId, 'summary', ({ completedChunks, totalChunks }) => {
      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        generationProgress: Math.round((completedChunks / Math.max(totalChunks, 1)) * 100),
      }, targetHistoryId);
    });

    try {
      const result = await invoke<TranscriptSummaryResult>('summarize_transcript', {
        request: this.buildRequest(taskId, activeTemplate, segments),
      });

      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      const record: TranscriptSummaryRecord = {
        template: result.template,
        content: result.content.trim(),
        generatedAt: new Date().toISOString(),
        sourceFingerprint,
      };

      useTranscriptStore.getState().updateSummaryState({
        activeTemplate: result.template,
        record,
      }, targetHistoryId);

      if (targetHistoryId !== 'current') {
        await this.persistSummary(targetHistoryId);
      }
    } catch (error) {
      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        generationProgress: 0,
      }, targetHistoryId);
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenProgress();

      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        isGenerating: false,
        generationProgress: 0,
      }, targetHistoryId);
    }
  }

  private buildRequest(
    taskId: string,
    template: SummaryTemplate,
    segments: TranscriptSegment[],
  ): SummarizeTranscriptRequest {
    const config = useTranscriptStore.getState().config;

    return {
      taskId,
      config: getFeatureLlmConfig(config, 'summary')!,
      template,
      segments: segments.map<SummarySegmentInput>(({ id, text, start, end, isFinal }) => ({
        id,
        text,
        start,
        end,
        isFinal,
      })),
    };
  }

  private resolveTargetHistoryId(jobHistoryId: string, sourceFingerprint: string): string {
    if (jobHistoryId !== 'current') {
      return jobHistoryId;
    }

    const store = useTranscriptStore.getState();
    if (
      store.sourceHistoryId &&
      computeSummarySourceFingerprint(store.segments) === sourceFingerprint
    ) {
      return store.sourceHistoryId;
    }

    return 'current';
  }
}

export const summaryService = new SummaryService();
