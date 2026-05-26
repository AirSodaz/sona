import {
  DEFAULT_SUMMARY_TEMPLATE_ID,
  HistorySummaryPayload,
  ResolvedSummaryTemplate,
  SummaryTemplateId,
  TranscriptSegment,
  TranscriptSummaryRecord,
  TranscriptSummaryState,
} from '../types/transcript';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { computeSummarySourceFingerprint } from '../utils/segmentUtils';
import { historyService } from './historyService';
import { getFeatureLlmConfig, isSummaryLlmConfigComplete } from './llm/runtime';
import type { SummaryTranscriptLlmJobRequest } from './llmTaskService';
import { runTranscriptLlmTaskJob } from './llm/segmentTask';
import { coerceSummaryTemplateId, resolveSummaryTemplate } from '../utils/summaryTemplates';
import { runTranscriptLlmJob } from './tauri/llm';
import {
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
} from './taskLedgerRuntime';

interface RetrySummaryTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  templateId?: SummaryTemplateId;
}

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

    const existingState = useTranscriptSidecarStore.getState().summaryStates[historyId];
    if (hasStoredSummaryState(existingState)) {
      return;
    }

    const payload = await historyService.loadSummary(historyId);
    const latestState = useTranscriptSidecarStore.getState().summaryStates[historyId];
    // Another async path may have populated or modified the state while the sidecar was
    // loading, so re-check before hydrating to avoid overwriting fresher in-memory data.
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

  async setActiveTemplate(templateId: SummaryTemplateId, historyId?: string): Promise<void> {
    const sessionStore = useTranscriptSessionStore.getState();
    const sidecarStore = useTranscriptSidecarStore.getState();
    const config = getEffectiveConfigSnapshot();
    const targetHistoryId = historyId || sessionStore.sourceHistoryId || 'current';
    const resolvedTemplateId = coerceSummaryTemplateId(templateId, config.summaryCustomTemplates);
    sidecarStore.setActiveSummaryTemplate(resolvedTemplateId, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async updateSummaryRecord(content: string, historyId?: string): Promise<void> {
    const sessionStore = useTranscriptSessionStore.getState();
    const sidecarStore = useTranscriptSidecarStore.getState();
    const config = getEffectiveConfigSnapshot();
    const targetHistoryId = historyId || sessionStore.sourceHistoryId || 'current';
    const summaryState = sidecarStore.getSummaryState(targetHistoryId);
    const activeTemplateId = coerceSummaryTemplateId(
      summaryState.activeTemplateId || config.summaryTemplateId,
      config.summaryCustomTemplates,
    );
    const hasMeaningfulContent = content.trim().length > 0;

    if (!summaryState.record && !hasMeaningfulContent) {
      return;
    }

    const sourceFingerprint = computeSummarySourceFingerprint(sessionStore.segments);
    sidecarStore.updateSummaryState({
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
    const sessionStore = useTranscriptSessionStore.getState();
    await this.retrySummaryTranscriptJob({
      segments: sessionStore.segments,
      historyId: sessionStore.sourceHistoryId,
      templateId,
    });
  }

  async retrySummaryTranscriptJob({
    segments,
    historyId,
    templateId,
  }: RetrySummaryTranscriptJobOptions): Promise<void> {
    await this.runSummaryTranscriptJob({
      segments,
      historyId,
      templateId,
    });
  }

  private async runSummaryTranscriptJob({
    segments,
    historyId,
    templateId,
  }: RetrySummaryTranscriptJobOptions): Promise<void> {
    const sidecarStore = useTranscriptSidecarStore.getState();
    const config = getEffectiveConfigSnapshot();

    if (config.summaryEnabled === false) {
      throw new Error('Summary is disabled.');
    }

    if (!isSummaryLlmConfigComplete(config)) {
      throw new Error('LLM Service not fully configured.');
    }

    if (!segments || segments.length === 0) {
      return;
    }

    const jobHistoryId = historyId || 'current';
    const resolvedTemplate = resolveSummaryTemplate(
      templateId ?? sidecarStore.getSummaryState(jobHistoryId).activeTemplateId ?? config.summaryTemplateId,
      config.summaryCustomTemplates,
    );
    const activeTemplateId = resolvedTemplate.id;

    await runTranscriptLlmTaskJob({
      taskType: 'summary',
      segments,
      sourceHistoryId: historyId,
      templateId: activeTemplateId,
      onStart: (startedHistoryId) => {
        sidecarStore.updateSummaryState({
          activeTemplateId,
          isGenerating: true,
          generationProgress: 0,
          // Keep a dedicated transient buffer for streamed text so the final record can still
          // be written atomically once the backend returns the finished summary payload.
          streamingContent: '',
        }, startedHistoryId);
      },
      onProgress: (generationProgress, progressHistoryId) => {
        this.updateJobSummaryState(progressHistoryId, {
          generationProgress,
        });
      },
      onText: ({ text }, textHistoryId) => {
        this.updateJobSummaryState(textHistoryId, {
          streamingContent: text,
        });
      },
      runTask: async (taskId, runningHistoryId) => {
        const result = await runTranscriptLlmJob(
          this.buildRequest(taskId, runningHistoryId, resolvedTemplate, segments),
        );
        const summary = result.summary;
        const summaryRecord = summary?.record;

        if (!summary || !summaryRecord) {
          throw new Error('Summary job did not return a summary record.');
        }

        if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
          return;
        }

        const resultTemplateId = coerceSummaryTemplateId(summary.activeTemplateId, config.summaryCustomTemplates);
        const record: TranscriptSummaryRecord = {
          templateId: coerceSummaryTemplateId(summaryRecord.templateId, config.summaryCustomTemplates),
          content: summaryRecord.content,
          generatedAt: summaryRecord.generatedAt,
          sourceFingerprint: summaryRecord.sourceFingerprint,
        };

        const targetHistoryId = this.updateJobSummaryState(runningHistoryId, {
          activeTemplateId: resultTemplateId,
          record,
          streamingContent: undefined,
        });

        if (runningHistoryId === 'current' && targetHistoryId !== 'current') {
          await this.persistSummary(targetHistoryId);
        }
      },
      onError: (errorHistoryId) => {
        this.updateJobSummaryState(errorHistoryId, {
          generationProgress: 0,
        });
      },
      onFinally: (finishedHistoryId) => {
        this.updateJobSummaryState(finishedHistoryId, {
          isGenerating: false,
          generationProgress: 0,
        });
      },
    });
  }

  private buildRequest(
    taskId: string,
    jobHistoryId: string,
    template: ResolvedSummaryTemplate,
    segments: TranscriptSegment[],
  ): SummaryTranscriptLlmJobRequest {
    const config = getEffectiveConfigSnapshot();

    return {
      taskId,
      taskType: 'summary',
      jobHistoryId: jobHistoryId === 'current' ? null : jobHistoryId,
      config: getFeatureLlmConfig(config, 'summary')!,
      template,
      segments,
    };
  }

  private updateJobSummaryState(
    jobHistoryId: string,
    state: Partial<TranscriptSummaryState>,
  ): string {
    const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId);
    useTranscriptSidecarStore.getState().updateSummaryState(state, targetHistoryId);
    return targetHistoryId;
  }

  private resolveTargetHistoryId(jobHistoryId: string): string {
    if (jobHistoryId !== 'current') {
      return jobHistoryId;
    }

    const sessionStore = useTranscriptSessionStore.getState();
    // A "current" job can become history-backed after save. The coordinator rekeys
    // the transient summary state during that save, so follow-up UI updates should
    // continue on the newly durable history id.
    if (sessionStore.sourceHistoryId) {
      return sessionStore.sourceHistoryId;
    }

    return 'current';
  }
}

export const summaryService = new SummaryService();
