import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { resolveSidecarKey } from '../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AppConfig } from '../types/config';
import type {
  ResolvedSummaryTemplate,
  SummaryTemplateId,
  TranscriptSegment,
  TranscriptSummaryRecord,
} from '../types/transcript';
import { computeSummarySourceFingerprint } from '../utils/segmentUtils';
import { coerceSummaryTemplateId, resolveSummaryTemplate } from '../utils/summaryTemplates';
import { getFeatureLlmConfig, isSummaryLlmConfigComplete } from './llm/configUtils';
import { LlmJobCancelledError, type LlmJobPriority } from './llm/llmJobScheduler';
import { runTranscriptLlmTaskJob } from './llm/segmentTask';
import { runTranscriptLlmJob } from './llmOperations';
import type { SummaryTranscriptLlmJobRequest } from './llmTaskTypes';
import { resolveItemPipeline } from './projectPipeline';
import { summarySidecarService } from './summarySidecarService';
import { resolveTaskProjectContext } from './task/taskProjectContext';
import {
  captureTaskTarget,
  normalizeHistoryId,
  resolveTargetHistoryId,
  resolveTargetSidecarKey,
  type TaskTarget,
} from './task/taskTarget';

interface RetrySummaryTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  sessionId?: string | null;
  templateId?: SummaryTemplateId;
  config?: AppConfig;
  priority?: LlmJobPriority;
  allowWhenSummaryDisabled?: boolean;
}

type SummaryTargetOptions =
  | string
  | {
      historyId?: string | null;
      sessionId?: string | null;
    };

function parseSummaryTargetOptions(target?: SummaryTargetOptions): {
  historyId?: string | null;
  sessionId?: string | null;
} {
  if (typeof target === 'string') {
    return { historyId: target };
  }
  return target ?? {};
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
  private readonly inFlightSummaryKeys = new Set<string>();

  constructor(private readonly ports: SummaryServicePorts) {}

  async loadSummary(historyId: string): Promise<void> {
    await this.ports.summarySidecarService.loadSummary(historyId);
  }

  async persistSummary(historyId: string): Promise<void> {
    await this.ports.summarySidecarService.persistSummary(historyId);
  }

  async setActiveTemplate(
    templateId: SummaryTemplateId,
    targetOptions?: SummaryTargetOptions
  ): Promise<void> {
    const { historyId, sessionId } = parseSummaryTargetOptions(targetOptions);
    const resolvedSessionId =
      sessionId ?? useTranscriptStore.getState().activeSessionId ?? 'default';
    const sessionSnapshot = sessionId
      ? useTranscriptStore.getState().sessions[sessionId]
      : undefined;
    const sessionStore = this.ports.getTranscriptSessionStore();
    const resolvedHistoryId =
      normalizeHistoryId(historyId) ??
      normalizeHistoryId(
        sessionSnapshot ? sessionSnapshot.sourceHistoryId : sessionStore.sourceHistoryId
      );
    const sidecarKey = resolveSidecarKey(resolvedSessionId, resolvedHistoryId);
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const resolvedTemplateId = coerceSummaryTemplateId(templateId, config.summaryCustomTemplates);
    sidecarStore.updateSummaryState({ activeTemplateId: resolvedTemplateId }, sidecarKey);

    if (resolvedHistoryId) {
      await this.persistSummary(resolvedHistoryId);
    }
  }

  async updateSummaryRecord(content: string, targetOptions?: SummaryTargetOptions): Promise<void> {
    const { historyId, sessionId } = parseSummaryTargetOptions(targetOptions);
    const resolvedSessionId =
      sessionId ?? useTranscriptStore.getState().activeSessionId ?? 'default';
    const sessionSnapshot = sessionId
      ? useTranscriptStore.getState().sessions[sessionId]
      : undefined;
    const sessionStore = this.ports.getTranscriptSessionStore();
    const resolvedHistoryId =
      normalizeHistoryId(historyId) ??
      normalizeHistoryId(
        sessionSnapshot ? sessionSnapshot.sourceHistoryId : sessionStore.sourceHistoryId
      );
    const sidecarKey = resolveSidecarKey(resolvedSessionId, resolvedHistoryId);
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const summaryState = sidecarStore.getSummaryState(sidecarKey);
    const activeTemplateId = coerceSummaryTemplateId(
      summaryState.activeTemplateId || config.summaryTemplateId,
      config.summaryCustomTemplates
    );
    const hasMeaningfulContent = content.trim().length > 0;

    if (!summaryState.record && !hasMeaningfulContent) {
      return;
    }

    const segments = sessionSnapshot?.segments ?? sessionStore.segments;
    const sourceFingerprint = computeSummarySourceFingerprint(segments);
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
      sidecarKey
    );

    if (resolvedHistoryId) {
      await this.persistSummary(resolvedHistoryId);
    }
  }

  async generateSummary(
    templateIdOrOptions?:
      | SummaryTemplateId
      | { templateId?: SummaryTemplateId; sessionId?: string | null }
  ): Promise<void> {
    const templateId =
      typeof templateIdOrOptions === 'string' || templateIdOrOptions === undefined
        ? templateIdOrOptions
        : templateIdOrOptions.templateId;
    const sessionId =
      typeof templateIdOrOptions === 'object' && templateIdOrOptions !== null
        ? (templateIdOrOptions.sessionId ?? null)
        : null;
    const sessionSnapshot = sessionId
      ? useTranscriptStore.getState().sessions[sessionId]
      : undefined;
    const sessionStore = this.ports.getTranscriptSessionStore();
    const segments = sessionSnapshot?.segments ?? sessionStore.segments;
    const historyId = sessionSnapshot
      ? sessionSnapshot.sourceHistoryId
      : sessionStore.sourceHistoryId;

    await this.retrySummaryTranscriptJob({
      segments,
      historyId,
      sessionId,
      templateId,
      priority: 'high',
    });
  }

  async retrySummaryTranscriptJob({
    segments,
    historyId,
    sessionId,
    templateId,
    config,
    priority,
    allowWhenSummaryDisabled,
  }: RetrySummaryTranscriptJobOptions): Promise<void> {
    await this.runSummaryTranscriptJob({
      segments,
      historyId,
      sessionId,
      templateId,
      config,
      priority,
      allowWhenSummaryDisabled,
    });
  }

  private getTargetInFlightKeys(target: TaskTarget): string[] {
    const keys = new Set<string>();
    const sidecarKey = resolveTargetSidecarKey(target);
    keys.add(sidecarKey);
    const latestHistoryId = resolveTargetHistoryId(target);
    if (latestHistoryId) {
      keys.add(latestHistoryId);
    }
    if (target.sessionId) {
      keys.add(resolveSidecarKey(target.sessionId, null));
    }
    return Array.from(keys);
  }

  private async runSummaryTranscriptJob({
    segments,
    historyId,
    sessionId,
    templateId,
    config: configOverride,
    priority,
    allowWhenSummaryDisabled,
  }: RetrySummaryTranscriptJobOptions): Promise<void> {
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = configOverride ?? this.ports.getEffectiveConfigSnapshot();

    if (config.summaryEnabled === false && !allowWhenSummaryDisabled) {
      throw new Error('Summary is disabled.');
    }

    if (!isSummaryLlmConfigComplete(config)) {
      throw new Error('LLM Service not fully configured.');
    }

    if (!segments || segments.length === 0) {
      return;
    }

    const target = captureTaskTarget({ sessionId, historyId });
    const candidateKeys = this.getTargetInFlightKeys(target);
    if (candidateKeys.some((key) => this.inFlightSummaryKeys.has(key))) {
      return;
    }
    const trackedKeys = new Set<string>(candidateKeys);
    for (const key of trackedKeys) {
      this.inFlightSummaryKeys.add(key);
    }

    const initialSidecarKey = resolveTargetSidecarKey(target);
    const normalizedHistoryId = normalizeHistoryId(historyId);
    const { projectId, pipelineSnapshot } = resolveTaskProjectContext({
      sessionId,
      historyId: normalizedHistoryId,
    });
    const pipeline =
      pipelineSnapshot ??
      resolveItemPipeline(projectId, useProjectStore.getState().projects, config);
    const candidateTemplateId =
      templateId ??
      sidecarStore.getSummaryState(initialSidecarKey).activeTemplateId ??
      pipeline.summaryTemplateId ??
      config.summaryTemplateId;
    const resolvedTemplate = resolveSummaryTemplate(
      candidateTemplateId,
      config.summaryCustomTemplates
    );
    const activeTemplateId = resolvedTemplate.id;

    try {
      await this.ports.runTranscriptLlmTaskJob({
        taskType: 'summary',
        segments,
        sourceHistoryId: historyId,
        originSessionId: sessionId ?? null,
        templateId: activeTemplateId,
        onStart: (sidecarKey) => {
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
            sidecarKey
          );
        },
        onProgress: (generationProgress, sidecarKey) => {
          this.ports.getTranscriptSidecarStore().updateSummaryState(
            {
              generationProgress,
            },
            sidecarKey
          );
        },
        onText: ({ text, isThought, reset }, sidecarKey) => {
          const currentSidecarStore = this.ports.getTranscriptSidecarStore();
          if (reset) {
            currentSidecarStore.updateSummaryState(
              {
                streamingContent: '',
                streamingThought: '',
              },
              sidecarKey
            );
            return;
          }
          if (isThought) {
            currentSidecarStore.updateSummaryState(
              {
                streamingThought: text,
              },
              sidecarKey
            );
            return;
          }
          currentSidecarStore.updateSummaryState(
            {
              streamingContent: text,
            },
            sidecarKey
          );
        },
        runTask: async (taskId, _initialKey, ctx) => {
          if (ctx.isCancelled()) {
            throw new LlmJobCancelledError();
          }

          const sentJobHistoryId = ctx.resolveHistoryId();
          const result = await this.ports.runTranscriptLlmJob(
            this.buildRequest(taskId, sentJobHistoryId, resolvedTemplate, segments, config),
            {
              priority: priority ?? 'normal',
              taskId,
              cancelCheck: ctx.isCancelled,
              onStart: ctx.markRunning,
            }
          );
          const summary = result.summary;
          const summaryRecord = summary?.record;

          if (!summary || !summaryRecord) {
            throw new Error('Summary job did not return a summary record.');
          }

          if (ctx.isCancelled()) {
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

          const completedHistoryId = ctx.resolveHistoryId();
          if (!sentJobHistoryId && completedHistoryId && ctx.target.sessionId) {
            useTranscriptStore
              .getState()
              .rekeySessionSidecar(ctx.target.sessionId, completedHistoryId);
          }

          const latestSidecarKey = ctx.resolveSidecarKey();
          this.ports.getTranscriptSidecarStore().updateSummaryState(
            {
              activeTemplateId: resultTemplateId,
              record,
              streamingContent: undefined,
              streamingThought: undefined,
            },
            latestSidecarKey
          );

          if (completedHistoryId) {
            trackedKeys.add(completedHistoryId);
            this.inFlightSummaryKeys.add(completedHistoryId);
          }
          if (!sentJobHistoryId && completedHistoryId) {
            await this.persistSummary(completedHistoryId);
          }
        },
        onError: (sidecarKey) => {
          this.ports.getTranscriptSidecarStore().updateSummaryState(
            {
              generationProgress: 0,
            },
            sidecarKey
          );
        },
        onFinally: (sidecarKey) => {
          this.ports.getTranscriptSidecarStore().updateSummaryState(
            {
              isGenerating: false,
              generationProgress: 0,
            },
            sidecarKey
          );
        },
      });
    } finally {
      for (const key of this.getTargetInFlightKeys(target)) {
        trackedKeys.add(key);
      }
      for (const key of trackedKeys) {
        this.inFlightSummaryKeys.delete(key);
      }
    }
  }

  private buildRequest(
    taskId: string,
    jobHistoryId: string | null,
    template: ResolvedSummaryTemplate,
    segments: TranscriptSegment[],
    config: AppConfig
  ): SummaryTranscriptLlmJobRequest {
    const summaryLlmConfig = getFeatureLlmConfig(config, 'summary');
    if (!summaryLlmConfig) {
      throw new Error('LLM Service not fully configured.');
    }
    return {
      taskId,
      taskType: 'summary',
      jobHistoryId,
      config: summaryLlmConfig,
      template,
      segments,
    };
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
