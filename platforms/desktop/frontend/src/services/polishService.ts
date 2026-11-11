import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import { setSegmentsForHistory, setSegmentsForSession } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AppConfig } from '../types/config';
import type { PolishMode } from '../types/llmTask';
import type { TranscriptSegment } from '../types/transcript';
import { resolvePolishPreset } from '../utils/polishPresets';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/configUtils';
import { LlmJobCancelledError, type LlmJobPriority } from './llm/llmJobScheduler';
import { runConfiguredSegmentTask, runTranscriptSegmentTaskJob } from './llm/segmentTask';
import { runTranscriptLlmJob } from './llmOperations';
import { listenToTranscriptLlmJobUpdates } from './llmTaskEvents';
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  TranscriptLlmJobResult,
} from './llmTaskTypes';
import { resolveItemPipeline } from './projectPipeline';
import { resolveTaskProjectContext } from './task/taskProjectContext';
import {
  createTaskTarget,
  mergeUpdatedSegmentsById,
  normalizeHistoryId,
  resolveLatestSegmentsForTarget,
  resolveTargetHistoryId,
  runWithDocumentLock,
  type TaskTarget,
} from './task/taskTarget';
import { transcriptAutoSaveRuntime } from './transcriptAutoSaveRuntime';

interface RetryPolishTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  sessionId?: string | null;
  polishPresetId?: string;
  priority?: LlmJobPriority;
}

interface SegmentExecutionOptions {
  priority?: LlmJobPriority;
  signal?: AbortSignal;
  cancelCheck?: () => boolean;
  onStart?: () => void | Promise<void>;
}

function buildPolishedSegmentMap(polishedChunk: PolishedSegment[]): Map<string, PolishedSegment> {
  const polishedMap = new Map<string, PolishedSegment>();
  for (let index = 0; index < polishedChunk.length; index += 1) {
    polishedMap.set(polishedChunk[index].id, polishedChunk[index]);
  }
  return polishedMap;
}

function applyPolishedChunkToSegments(
  segments: TranscriptSegment[],
  polishedMap: ReadonlyMap<string, PolishedSegment>
): TranscriptSegment[] {
  // Merge by id so polishing only rewrites transcript text and keeps each segment's
  // existing timing, speaker, and translation fields intact.
  return segments.map((segment) => {
    const polished = polishedMap.get(segment.id);
    return polished ? { ...segment, text: polished.text } : segment;
  });
}

export interface PolishServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  getActiveSessionSnapshot?: () => { sessionId: string; sourceHistoryId: string | null };
  getSessionSnapshot?: (sessionId: string) => { sourceHistoryId: string | null } | null;
  setSegmentsForSession?: typeof setSegmentsForSession;
  setSegmentsForHistory: typeof setSegmentsForHistory;
  commitSystemTranscriptUpdate?: (id: string, segments: TranscriptSegment[]) => Promise<unknown>;
  rekeySessionSidecar?: (sessionId: string, nextHistoryId: string) => void;
  runConfiguredSegmentTask: typeof runConfiguredSegmentTask;
  runTranscriptSegmentTaskJob: typeof runTranscriptSegmentTaskJob;
  runTranscriptLlmJob: typeof runTranscriptLlmJob;
  listenToTranscriptLlmJobUpdates: typeof listenToTranscriptLlmJobUpdates;
  rebaselineTranscriptAutoSave?: (historyId: string, segments: TranscriptSegment[]) => void;
}

export class PolishService {
  constructor(private readonly ports: PolishServicePorts) {}

  async polishSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets'>,
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
    executionOptions?: SegmentExecutionOptions
  ): Promise<PolishedSegment[]> {
    return this.ports.runConfiguredSegmentTask({
      feature: 'polish',
      taskType: 'polish',
      config,
      segments,
      onChunk: onChunkPolished,
      taskIdOverride,
      priority: executionOptions?.priority,
      signal: executionOptions?.signal,
      cancelCheck: executionOptions?.cancelCheck,
      onStart: executionOptions?.onStart,
      buildRequest: ({ taskId, llmConfig, segments: inputSegments }) =>
        this.buildRequest(taskId, llmConfig, config, inputSegments),
    });
  }

  async polishSegments(
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
    executionOptions?: SegmentExecutionOptions
  ): Promise<PolishedSegment[]> {
    return this.polishSegmentsWithConfig(
      this.ports.getEffectiveConfigSnapshot(),
      segments,
      onChunkPolished,
      undefined,
      executionOptions
    );
  }

  async polishTranscript(options?: { sessionId?: string | null }) {
    const requestedSessionId =
      options?.sessionId ?? this.ports.getActiveSessionSnapshot?.().sessionId ?? null;
    const sessionSnapshot = requestedSessionId
      ? useTranscriptStore.getState().sessions[requestedSessionId]
      : undefined;
    const sessionStore = this.ports.getTranscriptSessionStore();
    const segments = sessionSnapshot?.segments ?? sessionStore.segments;
    const historyId = sessionSnapshot
      ? sessionSnapshot.sourceHistoryId
      : sessionStore.sourceHistoryId;

    await this.retryPolishTranscriptJob({
      segments,
      historyId,
      sessionId: requestedSessionId,
      priority: 'high',
    });
  }

  async retryPolishTranscriptJob(options: RetryPolishTranscriptJobOptions): Promise<void> {
    const { segments, historyId, sessionId, polishPresetId, priority } = options;
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'polish');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    await this.ports.runTranscriptSegmentTaskJob({
      taskType: 'polish',
      segments,
      sourceHistoryId: historyId,
      originSessionId: sessionId ?? null,
      onStart: (sidecarKey) => {
        sidecarStore.updateLlmState({ isPolishing: true, polishProgress: 0 }, sidecarKey);
      },
      onProgress: (polishProgress, sidecarKey) => {
        // Progress tracks the transcript that launched the job, not whichever record the
        // user is viewing by the time a late chunk event arrives.
        this.ports.getTranscriptSidecarStore().updateLlmState({ polishProgress }, sidecarKey);
      },
      runTask: async (taskId, _initialSidecarKey, ctx) => {
        await runWithDocumentLock(ctx.target, { taskType: 'polish' }, async () => {
          if (ctx.isCancelled()) {
            throw new LlmJobCancelledError();
          }

          const latestSegments = await resolveLatestSegmentsForTarget(ctx.target, segments);
          const sentJobHistoryId = ctx.resolveHistoryId();
          const { projectId, pipelineSnapshot } = resolveTaskProjectContext({
            sessionId: sessionId ?? ctx.target.sessionId,
            historyId: sentJobHistoryId,
          });
          const pipeline =
            pipelineSnapshot ??
            resolveItemPipeline(projectId, useProjectStore.getState().projects, config);
          const resolvedPresetId =
            polishPresetId || pipeline.polishPresetId || config.polishPresetId;
          const preset = resolvePolishPreset(resolvedPresetId, config.polishCustomPresets);
          const unlistenJobUpdates = await this.ports.listenToTranscriptLlmJobUpdates(
            taskId,
            'polish',
            (payload) => {
              if (ctx.isCancelled()) {
                return;
              }
              this.applyTranscriptJobUpdate(payload, ctx.target);
            }
          );
          try {
            const mode: PolishMode =
              preset.id === 'verbatim' || preset.id === 'formal' ? preset.id : 'clean';
            const result = await this.ports.runTranscriptLlmJob(
              {
                taskId,
                taskType: 'polish',
                jobHistoryId: sentJobHistoryId,
                config: llm,
                segments: latestSegments,
                context: preset.context || undefined,
                mode,
              },
              {
                priority: priority ?? 'normal',
                taskId,
                cancelCheck: ctx.isCancelled,
                onStart: ctx.markRunning,
              }
            );
            if (ctx.isCancelled()) {
              return;
            }

            const completedHistoryId = ctx.resolveHistoryId();
            if (!sentJobHistoryId && completedHistoryId && result.segments) {
              if (ctx.target.sessionId) {
                (
                  this.ports.rekeySessionSidecar ??
                  useTranscriptStore.getState().rekeySessionSidecar
                )(ctx.target.sessionId, completedHistoryId);
              }
              const latestBase = await resolveLatestSegmentsForTarget(ctx.target, result.segments);
              const merged = mergeUpdatedSegmentsById(latestBase, result.segments, 'text');
              const commitUpdate =
                this.ports.commitSystemTranscriptUpdate ??
                useHistoryStore.getState().commitSystemTranscriptUpdate;
              await commitUpdate(completedHistoryId, merged);
              return;
            }

            this.applyTranscriptJobUpdate(result, ctx.target);
            if (result.segments && completedHistoryId) {
              this.ports.rebaselineTranscriptAutoSave?.(completedHistoryId, result.segments);
            }
          } finally {
            unlistenJobUpdates();
          }
        });
      },
      onFinally: (sidecarKey) => {
        this.ports.getTranscriptSidecarStore().updateLlmState(
          {
            isPolishing: false,
            polishProgress: 0,
          },
          sidecarKey
        );
      },
    });
  }

  applyPolishedSegmentsInMemory(
    segments: TranscriptSegment[],
    polishedChunk: PolishedSegment[]
  ): TranscriptSegment[] {
    return applyPolishedChunkToSegments(segments, buildPolishedSegmentMap(polishedChunk));
  }

  private buildRequest(
    taskId: string,
    llmConfig: NonNullable<ReturnType<typeof getFeatureLlmConfig>>,
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets'>,
    segments: TranscriptSegment[]
  ): PolishSegmentsRequest {
    const preset = resolvePolishPreset(config.polishPresetId, config.polishCustomPresets);
    const mode: PolishMode =
      preset.id === 'verbatim' || preset.id === 'formal' ? preset.id : 'clean';

    return {
      taskId,
      config: llmConfig,
      segments: segments.map(({ id, text }) => ({ id, text })),
      context: preset.context || undefined,
      mode,
    };
  }

  private applyTranscriptJobUpdate(
    payload: TranscriptLlmJobResult,
    targetOrOriginSession: TaskTarget | string | null = null
  ) {
    if (!payload.segments) {
      return;
    }

    const payloadHistoryId = normalizeHistoryId(payload.jobHistoryId);
    if (payloadHistoryId) {
      this.ports.setSegmentsForHistory(payloadHistoryId, payload.segments);
      return;
    }

    const target =
      typeof targetOrOriginSession === 'string' || targetOrOriginSession === null
        ? createTaskTarget({ sessionId: targetOrOriginSession })
        : targetOrOriginSession;

    const latestHistoryId = resolveTargetHistoryId(target);
    if (latestHistoryId) {
      this.ports.setSegmentsForHistory(latestHistoryId, payload.segments);
      return;
    }

    if (target.sessionId) {
      const originSession =
        this.ports.getSessionSnapshot?.(target.sessionId) ??
        useTranscriptStore.getState().sessions[target.sessionId];
      if (!originSession || normalizeHistoryId(originSession.sourceHistoryId)) {
        return;
      }
      if (this.ports.setSegmentsForSession) {
        this.ports.setSegmentsForSession(target.sessionId, payload.segments);
      } else {
        setSegmentsForSession(target.sessionId, payload.segments);
      }
      return;
    }

    const sessionStore = this.ports.getTranscriptSessionStore();
    if (sessionStore.sourceHistoryId) {
      return;
    }
    sessionStore.setSegments(payload.segments);
  }
}

export function createPolishService(ports: PolishServicePorts): PolishService {
  return new PolishService(ports);
}

export const polishService = createPolishService({
  getEffectiveConfigSnapshot,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  getTranscriptSidecarStore: useTranscriptSidecarStore.getState,
  getActiveSessionSnapshot: () => {
    const state = useTranscriptStore.getState();
    return {
      sessionId: state.activeSessionId,
      sourceHistoryId: state.sessions[state.activeSessionId]?.sourceHistoryId ?? null,
    };
  },
  getSessionSnapshot: (sessionId) => {
    const session = useTranscriptStore.getState().sessions[sessionId];
    return session ? { sourceHistoryId: session.sourceHistoryId ?? null } : null;
  },
  setSegmentsForSession,
  setSegmentsForHistory,
  commitSystemTranscriptUpdate: (id, segments) =>
    useHistoryStore.getState().commitSystemTranscriptUpdate(id, segments),
  rekeySessionSidecar: (sessionId, nextHistoryId) =>
    useTranscriptStore.getState().rekeySessionSidecar(sessionId, nextHistoryId),
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
  runTranscriptLlmJob,
  listenToTranscriptLlmJobUpdates,
  rebaselineTranscriptAutoSave: (historyId, segments) =>
    transcriptAutoSaveRuntime.rebaseline(historyId, segments),
});
