import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { AppConfig } from '../types/config';
import type {
  ResolvedSummaryTemplate,
  SummaryTemplateId,
  TranscriptSegment,
  TranscriptSummaryRecord,
  TranscriptSummaryState,
} from '../types/transcript';
import { computeSummarySourceFingerprint } from '../utils/segmentUtils';
import { coerceSummaryTemplateId, resolveSummaryTemplate } from '../utils/summaryTemplates';
import { getFeatureLlmConfig, isSummaryLlmConfigComplete } from './llm/configUtils';
import { runTranscriptLlmTaskJob } from './llm/segmentTask';
import type { SummaryTranscriptLlmJobRequest } from './llmTaskTypes';
import { resolveItemPipeline } from './projectPipeline';
import { summarySidecarService } from './summarySidecarService';
import { createLlmTaskLedgerId, isTaskLedgerCancelRequested } from './taskLedgerBuilders';
import { runTranscriptLlmJob } from './tauri/llm';

interface RetrySummaryTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  templateId?: SummaryTemplateId;
  config?: AppConfig;
}

function normalizeSummaryFingerprint(fingerprint: string): string {
  if (!fingerprint) return '';
  return fingerprint
    .split('|')
    .map((seg) => {
      let colonCount = 0;
      for (let i = 0; i < seg.length; i++) {
        if (seg[i] === ':') colonCount++;
      }
      if (colonCount >= 8) {
        const lastColon = seg.lastIndexOf(':');
        const candidateScore = seg.slice(lastColon + 1);
        if (candidateScore === '' || Number.isFinite(Number(candidateScore))) {
          return seg.slice(0, lastColon);
        }
      }
      return seg;
    })
    .join('|');
}

export function isSummaryRecordStale(
  record: TranscriptSummaryRecord | undefined,
  segments: TranscriptSegment[]
): boolean {
  if (!record) {
    return false;
  }

  const currentFingerprint = computeSummarySourceFingerprint(segments);
  if (record.sourceFingerprint === currentFingerprint) {
    return false;
  }

  // Handle legacy fingerprints that included speaker score (9 fields)
  const normalizedRecordFingerprint = normalizeSummaryFingerprint(record.sourceFingerprint);
  if (normalizedRecordFingerprint === currentFingerprint) {
    return false;
  }

  // Handle legacy fingerprints that lacked speaker fields entirely (5 fields)
  const normalizedLegacyNoSpeaker = record.sourceFingerprint
    .split('|')
    .map((seg) => (seg.split(':').length === 5 ? `${seg}:::` : seg))
    .join('|');
  if (normalizedLegacyNoSpeaker === currentFingerprint) {
    return false;
  }

  return true;
}

export interface SummaryServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  runTranscriptLlmTaskJob: typeof runTranscriptLlmTaskJob;
  runTranscriptLlmJob: typeof runTranscriptLlmJob;
  summarySidecarService: typeof summarySidecarService;
}

export class SummaryService {
  constructor(private readonly ports: SummaryServicePorts) {}

  async loadSummary(historyId: string): Promise<void> {
    await this.ports.summarySidecarService.loadSummary(historyId);
  }

  async persistSummary(historyId: string): Promise<void> {
    await this.ports.summarySidecarService.persistSummary(historyId);
  }

  async setActiveTemplate(templateId: SummaryTemplateId, historyId?: string): Promise<void> {
    const sessionStore = this.ports.getTranscriptSessionStore();
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const targetHistoryId = historyId || sessionStore.sourceHistoryId || 'current';
    const resolvedTemplateId = coerceSummaryTemplateId(templateId, config.summaryCustomTemplates);
    sidecarStore.updateSummaryState({ activeTemplateId: resolvedTemplateId }, targetHistoryId);

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async updateSummaryRecord(content: string, historyId?: string): Promise<void> {
    const sessionStore = this.ports.getTranscriptSessionStore();
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const targetHistoryId = historyId || sessionStore.sourceHistoryId || 'current';
    const summaryState = sidecarStore.getSummaryState(targetHistoryId);
    const activeTemplateId = coerceSummaryTemplateId(
      summaryState.activeTemplateId || config.summaryTemplateId,
      config.summaryCustomTemplates
    );
    const hasMeaningfulContent = content.trim().length > 0;

    if (!summaryState.record && !hasMeaningfulContent) {
      return;
    }

    const sourceFingerprint = computeSummarySourceFingerprint(sessionStore.segments);
    sidecarStore.updateSummaryState(
      {
        activeTemplateId,
        record: {
          templateId: activeTemplateId,
          content,
          generatedAt: new Date().toISOString(),
          sourceFingerprint,
        },
        streamingContent: undefined,
        streamingThought: undefined,
      },
      targetHistoryId
    );

    if (targetHistoryId !== 'current') {
      await this.persistSummary(targetHistoryId);
    }
  }

  async generateSummary(templateId?: SummaryTemplateId): Promise<void> {
    const sessionStore = this.ports.getTranscriptSessionStore();
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
    config: configOverride,
  }: RetrySummaryTranscriptJobOptions): Promise<void> {
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = configOverride ?? this.ports.getEffectiveConfigSnapshot();

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
    const activeProjectId = useProjectStore.getState().activeProjectId;
    const currentItem =
      historyId && historyId !== 'current'
        ? useHistoryStore.getState().items.find((i) => i.id === historyId)
        : null;
    const projectId = currentItem?.projectId ?? activeProjectId;
    const pipeline = resolveItemPipeline(projectId, useProjectStore.getState().projects, config);
    const candidateTemplateId =
      templateId ??
      sidecarStore.getSummaryState(jobHistoryId).activeTemplateId ??
      pipeline.summaryTemplateId ??
      config.summaryTemplateId;
    const resolvedTemplate = resolveSummaryTemplate(
      candidateTemplateId,
      config.summaryCustomTemplates
    );
    const activeTemplateId = resolvedTemplate.id;

    await this.ports.runTranscriptLlmTaskJob({
      taskType: 'summary',
      segments,
      sourceHistoryId: historyId,
      templateId: activeTemplateId,
      onStart: (startedHistoryId) => {
        sidecarStore.updateSummaryState(
          {
            activeTemplateId,
            isGenerating: true,
            generationProgress: 0,
            // Keep a dedicated transient buffer for streamed text so the final record can still
            // be written atomically once the backend returns the finished summary payload.
            streamingContent: '',
            streamingThought: '',
          },
          startedHistoryId
        );
      },
      onProgress: (generationProgress, progressHistoryId) => {
        this.updateJobSummaryState(progressHistoryId, {
          generationProgress,
        });
      },
      onText: ({ text, isThought, reset }, textHistoryId) => {
        if (reset) {
          this.updateJobSummaryState(textHistoryId, {
            streamingContent: '',
            streamingThought: '',
          });
          return;
        }
        if (isThought) {
          this.updateJobSummaryState(textHistoryId, {
            streamingThought: text,
          });
          return;
        }
        this.updateJobSummaryState(textHistoryId, {
          streamingContent: text,
        });
      },
      runTask: async (taskId, runningHistoryId) => {
        const result = await this.ports.runTranscriptLlmJob(
          this.buildRequest(taskId, runningHistoryId, resolvedTemplate, segments, config)
        );
        const summary = result.summary;
        const summaryRecord = summary?.record;

        if (!summary || !summaryRecord) {
          throw new Error('Summary job did not return a summary record.');
        }

        if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
          return;
        }

        const resultTemplateId = coerceSummaryTemplateId(
          summary.activeTemplateId,
          config.summaryCustomTemplates
        );
        const record: TranscriptSummaryRecord = {
          templateId: coerceSummaryTemplateId(
            summaryRecord.templateId,
            config.summaryCustomTemplates
          ),
          content: summaryRecord.content,
          thought: summaryRecord.thought ?? undefined,
          generatedAt: summaryRecord.generatedAt,
          sourceFingerprint:
            summaryRecord.sourceFingerprint || computeSummarySourceFingerprint(segments),
        };

        const targetHistoryId = this.updateJobSummaryState(runningHistoryId, {
          activeTemplateId: resultTemplateId,
          record,
          streamingContent: undefined,
          streamingThought: undefined,
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
    config: AppConfig
  ): SummaryTranscriptLlmJobRequest {
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
    state: Partial<TranscriptSummaryState>
  ): string {
    const targetHistoryId = this.resolveTargetHistoryId(jobHistoryId);
    this.ports.getTranscriptSidecarStore().updateSummaryState(state, targetHistoryId);
    return targetHistoryId;
  }

  private resolveTargetHistoryId(jobHistoryId: string): string {
    if (jobHistoryId !== 'current') {
      return jobHistoryId;
    }

    const sessionStore = this.ports.getTranscriptSessionStore();
    // A "current" job can become history-backed after save. The coordinator rekeys
    // the transient summary state during that save, so follow-up UI updates should
    // continue on the newly durable history id.
    if (sessionStore.sourceHistoryId) {
      return sessionStore.sourceHistoryId;
    }

    return 'current';
  }
}

export function createSummaryService(ports: SummaryServicePorts): SummaryService {
  return new SummaryService(ports);
}

export const summaryService = createSummaryService({
  getEffectiveConfigSnapshot,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  getTranscriptSidecarStore: useTranscriptSidecarStore.getState,
  runTranscriptLlmTaskJob,
  runTranscriptLlmJob,
  summarySidecarService,
});
