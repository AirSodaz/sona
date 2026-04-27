import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import {
  DEFAULT_SUMMARY_TEMPLATE_ID,
  HistorySummaryPayload,
  ResolvedSummaryTemplate,
  SummaryTemplateId,
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
  listenToLlmTaskText,
  SummarizeTranscriptRequest,
  SummarySegmentInput,
  TranscriptSummaryResult,
} from './llmTaskService';
import { coerceSummaryTemplateId, resolveSummaryTemplate } from '../utils/summaryTemplates';

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

function hasPersistableSummaryData(summaryState: TranscriptSummaryState): boolean {
  return (
    !!summaryState.record ||
    summaryState.activeTemplateId !== DEFAULT_SUMMARY_TEMPLATE_ID
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

  async setActiveTemplate(templateId: SummaryTemplateId, historyId?: string): Promise<void> {
    const store = useTranscriptStore.getState();
    const targetHistoryId = historyId || store.sourceHistoryId || 'current';
    const resolvedTemplateId = coerceSummaryTemplateId(templateId, store.config.summaryCustomTemplates);
    store.setActiveSummaryTemplate(resolvedTemplateId, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async updateSummaryRecord(content: string, historyId?: string): Promise<void> {
    const store = useTranscriptStore.getState();
    const targetHistoryId = historyId || store.sourceHistoryId || 'current';
    const summaryState = store.getSummaryState(targetHistoryId);
    const activeTemplateId = coerceSummaryTemplateId(
      summaryState.activeTemplateId || store.config.summaryTemplateId,
      store.config.summaryCustomTemplates,
    );
    const hasMeaningfulContent = content.trim().length > 0;

    if (!summaryState.record && !hasMeaningfulContent) {
      return;
    }

    const sourceFingerprint = computeSummarySourceFingerprint(store.segments);
    store.updateSummaryState({
      activeTemplateId,
      record: {
        templateId: activeTemplateId,
        content,
        generatedAt: new Date().toISOString(),
        sourceFingerprint,
      },
      streamingContent: undefined,
    }, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async generateSummary(templateId?: SummaryTemplateId): Promise<void> {
    const store = useTranscriptStore.getState();

    if (store.config.summaryEnabled === false) {
      throw new Error('Summary is disabled.');
    }

    if (!isSummaryLlmConfigComplete(store.config)) {
      throw new Error('LLM Service not fully configured.');
    }

    const segments = store.segments;
    if (!segments || segments.length === 0) {
      return;
    }

    const resolvedTemplate = resolveSummaryTemplate(
      templateId ?? store.getSummaryState().activeTemplateId ?? store.config.summaryTemplateId,
      store.config.summaryCustomTemplates,
    );
    const activeTemplateId = resolvedTemplate.id;
    const jobHistoryId = store.sourceHistoryId || 'current';
    const sourceFingerprint = computeSummarySourceFingerprint(segments);
    const taskId = createLlmTaskId('summary');

    store.updateSummaryState({
      activeTemplateId,
      isGenerating: true,
      generationProgress: 0,
      streamingContent: '',
    }, jobHistoryId);

    const unlistenProgress = await listenToLlmTaskProgress(taskId, 'summary', ({ completedChunks, totalChunks }) => {
      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        generationProgress: Math.round((completedChunks / Math.max(totalChunks, 1)) * 100),
      }, targetHistoryId);
    });
    const unlistenText = await listenToLlmTaskText(taskId, 'summary', ({ text }) => {
      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        streamingContent: text,
      }, targetHistoryId);
    });

    try {
      const result = await invoke<TranscriptSummaryResult>('summarize_transcript', {
        request: this.buildRequest(taskId, resolvedTemplate, segments),
      });

      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      const resultTemplateId = coerceSummaryTemplateId(
        result.templateId,
        store.config.summaryCustomTemplates,
      );
      const record: TranscriptSummaryRecord = {
        templateId: resultTemplateId,
        content: result.content.trim(),
        generatedAt: new Date().toISOString(),
        sourceFingerprint,
      };

      useTranscriptStore.getState().updateSummaryState({
        activeTemplateId: resultTemplateId,
        record,
        streamingContent: undefined,
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
      unlistenText();

      const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId, sourceFingerprint);
      useTranscriptStore.getState().updateSummaryState({
        isGenerating: false,
        generationProgress: 0,
      }, targetHistoryId);
    }
  }

  private buildRequest(
    taskId: string,
    template: ResolvedSummaryTemplate,
    segments: TranscriptSegment[],
  ): SummarizeTranscriptRequest {
    const config = useTranscriptStore.getState().config;

    return {
      taskId,
      config: getFeatureLlmConfig(config, 'summary')!,
      template,
      segments: segments.map<SummarySegmentInput>(({ id, text, start, end, isFinal, speaker }) => ({
        id,
        text: speaker?.label ? `${speaker.label}: ${text}` : text,
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
