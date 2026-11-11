import type { TranscriptTextReplacementRuleSet } from '../bindings';
import { LANGUAGE_OPTIONS } from '../constants/languages';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useProjectStore } from '../stores/projectStore';
import {
  patchSegmentTranslationsForHistory,
  patchSegmentTranslationsForSession,
} from '../stores/transcriptCoordinator';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { resolveSidecarKey } from '../stores/transcriptSidecarState';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import { getActiveLlmConfig, getFeatureLlmConfig, isLlmConfigComplete } from './llm/configUtils';
import { LlmJobCancelledError, type LlmJobPriority } from './llm/llmJobScheduler';
import { runConfiguredSegmentTask, runTranscriptSegmentTaskJob } from './llm/segmentTask';
import { runTranscriptLlmJob, translateTranscriptSegments } from './llmOperations';
import { listenToTranscriptLlmJobUpdates } from './llmTaskEvents';
import type {
  TranscriptLlmJobResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from './llmTaskTypes';
import { resolveItemPipeline } from './projectPipeline';
import { resolveTaskProjectContext } from './task/taskProjectContext';
import {
  buildLocalNmtPostProcessReplacements,
  createTaskTarget,
  mergeUpdatedSegmentsById,
  normalizeHistoryId,
  resolveLatestSegmentsForTarget,
  resolveTargetHistoryId,
  resolveTargetSidecarKey,
  runWithDocumentLock,
  type TaskTarget,
} from './task/taskTarget';
import { toggleLiveTranslation } from './tauri/recognizer';
import { transcriptAutoSaveRuntime } from './transcriptAutoSaveRuntime';

interface RetryTranslateTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  sessionId?: string | null;
  targetLanguage?: string;
  priority?: LlmJobPriority;
}

interface TranslateSegmentExecutionOptions {
  priority?: LlmJobPriority;
  signal?: AbortSignal;
  cancelCheck?: () => boolean;
  onStart?: () => void | Promise<void>;
  postProcessReplacements?: TranscriptTextReplacementRuleSet[];
}

function buildTranslationMap(translations: TranslatedSegment[]): Map<string, TranslatedSegment> {
  const translationMap = new Map<string, TranslatedSegment>();
  for (let index = 0; index < translations.length; index += 1) {
    translationMap.set(translations[index].id, translations[index]);
  }
  return translationMap;
}

function applyTranslatedChunkToSegments(
  segments: TranscriptSegment[],
  translationMap: ReadonlyMap<string, TranslatedSegment>
): TranscriptSegment[] {
  // Merge by id so translated text reuses the existing timeline, speaker metadata, and
  // other segment fields instead of replacing transcript rows wholesale.
  // Typography and replacement rules are applied canonically in Rust.
  return segments.map((segment) => {
    const translated = translationMap.get(segment.id);
    return translated ? { ...segment, translation: translated.translation } : segment;
  });
}

export interface TranslationServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  getActiveSessionSnapshot?: () => { sessionId: string; sourceHistoryId: string | null };
  getSessionSnapshot?: (sessionId: string) => { sourceHistoryId: string | null } | null;
  patchSegmentTranslationsForSession?: typeof patchSegmentTranslationsForSession;
  patchSegmentTranslationsForHistory: typeof patchSegmentTranslationsForHistory;
  commitSystemTranscriptUpdate?: (id: string, segments: TranscriptSegment[]) => Promise<unknown>;
  rekeySessionSidecar?: (sessionId: string, nextHistoryId: string) => void;
  runConfiguredSegmentTask: typeof runConfiguredSegmentTask;
  runTranscriptSegmentTaskJob: typeof runTranscriptSegmentTaskJob;
  runTranscriptLlmJob: typeof runTranscriptLlmJob;
  translateTranscriptSegments: typeof translateTranscriptSegments;
  listenToTranscriptLlmJobUpdates: typeof listenToTranscriptLlmJobUpdates;
  toggleLiveTranslation?: typeof toggleLiveTranslation;
  rebaselineTranscriptAutoSave?: (historyId: string, segments: TranscriptSegment[]) => void;
}

export class TranslationService {
  constructor(private readonly ports: TranslationServicePorts) {}

  async translateSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
    onChunkTranslated?: (translatedChunk: TranslatedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
    executionOptions?: TranslateSegmentExecutionOptions
  ): Promise<TranslatedSegment[]> {
    return this.ports.runConfiguredSegmentTask({
      feature: 'translation',
      taskType: 'translate',
      config,
      segments,
      onChunk: onChunkTranslated,
      taskIdOverride,
      priority: executionOptions?.priority,
      signal: executionOptions?.signal,
      cancelCheck: executionOptions?.cancelCheck,
      onStart: executionOptions?.onStart,
      buildRequest: ({ taskId, llmConfig, segments: inputSegments }) =>
        this.buildRequest(
          taskId,
          llmConfig,
          config,
          inputSegments,
          executionOptions?.postProcessReplacements
        ),
    });
  }

  async translateText(
    text: string,
    targetLanguage: string,
    options?: { sourceLanguage?: string; signal?: AbortSignal }
  ): Promise<string> {
    const trimmed = text.trim();
    if (!trimmed) {
      return '';
    }
    const config = this.ports.getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'translation') ?? getActiveLlmConfig(config);
    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }
    const langOpt = LANGUAGE_OPTIONS.find((l) => l.code === targetLanguage);
    const taskId = `translate-text-${Date.now()}`;
    const results = await this.ports.translateTranscriptSegments(
      {
        taskId,
        config: llm,
        segments: [{ id: '1', text: trimmed }],
        targetLanguage,
        targetLanguageName: langOpt ? langOpt.englishName : undefined,
        sourceLanguage: options?.sourceLanguage,
      },
      {
        priority: 'high',
        taskId,
        signal: options?.signal,
      }
    );
    return results[0]?.translation ?? trimmed;
  }

  async translateCurrentTranscript(
    targetLanguageOverride?: string,
    options?: { sessionId?: string | null }
  ) {
    const requestedSessionId =
      options?.sessionId ?? this.ports.getActiveSessionSnapshot?.().sessionId ?? 'default';
    const isRecording = useTranscriptRuntimeStore.getState().isRecording;
    const recordingSessionId = useTranscriptStore.getState().recordingSessionId;
    const sessionSnapshot = options?.sessionId
      ? useTranscriptStore.getState().sessions[options.sessionId]
      : undefined;
    const sessionStore = this.ports.getTranscriptSessionStore();
    const sourceHistoryId = sessionSnapshot
      ? sessionSnapshot.sourceHistoryId
      : sessionStore.sourceHistoryId;
    const segments = sessionSnapshot?.segments ?? sessionStore.segments;

    const isLiveSession =
      isRecording &&
      (recordingSessionId
        ? requestedSessionId === recordingSessionId || sourceHistoryId === recordingSessionId
        : !normalizeHistoryId(sourceHistoryId));

    if (isLiveSession) {
      const sidecarKey = resolveSidecarKey(requestedSessionId, sourceHistoryId);
      const sidecarStore = this.ports.getTranscriptSidecarStore();
      sidecarStore.updateLlmState({ isTranslationVisible: true }, sidecarKey);
      const config = this.ports.getEffectiveConfigSnapshot();
      const targetLang =
        targetLanguageOverride ||
        config.realtimeTranslationTargetLanguage ||
        config.translationLanguage ||
        'zh';
      const sourceLang = config.realtimeTranslationSourceLanguage || 'auto';
      const toggleFn = this.ports.toggleLiveTranslation ?? toggleLiveTranslation;
      await toggleFn({
        enabled: true,
        sourceLang,
        targetLang,
        catchupSegments: segments.filter((s) => s.isFinal),
      });
      return;
    }

    await this.retryTranslateTranscriptJob({
      segments,
      historyId: sourceHistoryId,
      sessionId: requestedSessionId,
      targetLanguage: targetLanguageOverride,
      priority: 'high',
    });
  }

  async retryTranslateTranscriptJob({
    segments,
    historyId,
    sessionId,
    targetLanguage,
    priority,
  }: RetryTranslateTranscriptJobOptions): Promise<void> {
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'translation') ?? getActiveLlmConfig(config);

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    const { projectId: initialProjectId, pipelineSnapshot: initialSnapshot } =
      resolveTaskProjectContext({
        sessionId,
        historyId,
      });
    const initialPipeline =
      initialSnapshot ??
      resolveItemPipeline(initialProjectId, useProjectStore.getState().projects, config);
    const resolvedTargetLanguage =
      targetLanguage || initialPipeline.targetLanguage || config.translationLanguage || 'zh';

    await this.ports.runTranscriptSegmentTaskJob({
      taskType: 'translate',
      segments,
      sourceHistoryId: historyId,
      originSessionId: sessionId ?? null,
      targetLanguage: resolvedTargetLanguage,
      onStart: (sidecarKey) => {
        sidecarStore.updateLlmState({ isTranslating: true, translationProgress: 0 }, sidecarKey);
      },
      onProgress: (translationProgress, sidecarKey) => {
        // Progress belongs to the job's original record even if the user navigates away
        // before the backend finishes, so keep writing against the resolved target key.
        this.ports.getTranscriptSidecarStore().updateLlmState({ translationProgress }, sidecarKey);
      },
      runTask: async (taskId, _initialSidecarKey, ctx) => {
        await runWithDocumentLock(ctx.target, { taskType: 'translate' }, async () => {
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
          const effectiveTargetLanguage =
            targetLanguage || pipeline.targetLanguage || config.translationLanguage || 'zh';
          const keywords =
            pipeline.customTerms && pipeline.customTerms.length > 0
              ? pipeline.customTerms.join('\n')
              : undefined;
          const postProcessReplacements =
            llm.strategy === 'local_nmt'
              ? buildLocalNmtPostProcessReplacements(pipeline)
              : undefined;

          const unlistenJobUpdates = await this.ports.listenToTranscriptLlmJobUpdates(
            taskId,
            'translate',
            (payload) => {
              if (ctx.isCancelled()) {
                return;
              }
              this.applyTranscriptJobUpdate(payload, ctx.target);
            }
          );
          try {
            const langOpt = LANGUAGE_OPTIONS.find((l) => l.code === effectiveTargetLanguage);
            const result = await this.ports.runTranscriptLlmJob(
              {
                taskId,
                taskType: 'translate',
                jobHistoryId: sentJobHistoryId,
                config: llm,
                segments: latestSegments,
                targetLanguage: effectiveTargetLanguage,
                targetLanguageName: langOpt ? langOpt.englishName : undefined,
                keywords,
                postProcessReplacements,
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
              const merged = mergeUpdatedSegmentsById(latestBase, result.segments, 'translation');
              const commitUpdate =
                this.ports.commitSystemTranscriptUpdate ??
                useHistoryStore.getState().commitSystemTranscriptUpdate;
              await commitUpdate(completedHistoryId, merged);
              this.ports
                .getTranscriptSidecarStore()
                .updateLlmState({ isTranslationVisible: true }, completedHistoryId);
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
      onSuccess: (sidecarKey) => {
        this.ports
          .getTranscriptSidecarStore()
          .updateLlmState({ translationProgress: 100 }, sidecarKey);
      },
      onError: (sidecarKey) => {
        this.ports
          .getTranscriptSidecarStore()
          .updateLlmState({ translationProgress: 0 }, sidecarKey);
      },
      onFinally: (sidecarKey) => {
        const currentSidecarStore = this.ports.getTranscriptSidecarStore();
        currentSidecarStore.updateLlmState({ isTranslating: false }, sidecarKey);

        if (!currentSidecarStore.getLlmState(sidecarKey).isTranslationVisible) {
          currentSidecarStore.updateLlmState({ isTranslationVisible: true }, sidecarKey);
        }
      },
    });
  }

  applyTranslationsInMemory(
    segments: TranscriptSegment[],
    translations: TranslatedSegment[]
  ): TranscriptSegment[] {
    return applyTranslatedChunkToSegments(segments, buildTranslationMap(translations));
  }

  private buildRequest(
    taskId: string,
    llmConfig: NonNullable<ReturnType<typeof getFeatureLlmConfig>>,
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
    postProcessReplacements?: TranscriptTextReplacementRuleSet[]
  ): TranslateSegmentsRequest {
    const targetCode = config.translationLanguage || 'zh';
    const langOpt = LANGUAGE_OPTIONS.find((l) => l.code === targetCode);
    return {
      taskId,
      config: llmConfig,
      segments: segments.map(({ id, text }) => ({ id, text })),
      targetLanguage: targetCode,
      targetLanguageName: langOpt ? langOpt.englishName : undefined,
      ...(postProcessReplacements ? { postProcessReplacements } : {}),
    };
  }

  private applyTranscriptJobUpdate(
    payload: TranscriptLlmJobResult,
    targetOrOriginSession: TaskTarget | string | null = null
  ) {
    if (!payload.segments) {
      return;
    }

    const updates = payload.segments
      .filter((seg): seg is typeof seg & { translation: string } => Boolean(seg.translation))
      .map((seg) => ({ id: seg.id, translation: seg.translation }));

    const payloadHistoryId = normalizeHistoryId(payload.jobHistoryId);
    if (payloadHistoryId) {
      if (updates.length > 0) {
        this.ports.patchSegmentTranslationsForHistory(payloadHistoryId, updates);
      }
      this.ports
        .getTranscriptSidecarStore()
        .updateLlmState({ isTranslationVisible: true }, payloadHistoryId);
      return;
    }

    const target =
      typeof targetOrOriginSession === 'string' || targetOrOriginSession === null
        ? createTaskTarget({ sessionId: targetOrOriginSession })
        : targetOrOriginSession;

    const latestHistoryId = resolveTargetHistoryId(target);
    if (latestHistoryId) {
      if (updates.length > 0) {
        this.ports.patchSegmentTranslationsForHistory(latestHistoryId, updates);
      }
      this.ports
        .getTranscriptSidecarStore()
        .updateLlmState({ isTranslationVisible: true }, latestHistoryId);
      return;
    }

    if (target.sessionId) {
      const originSession =
        this.ports.getSessionSnapshot?.(target.sessionId) ??
        useTranscriptStore.getState().sessions[target.sessionId];
      if (!originSession || normalizeHistoryId(originSession.sourceHistoryId)) {
        return;
      }
      if (updates.length > 0) {
        if (this.ports.patchSegmentTranslationsForSession) {
          this.ports.patchSegmentTranslationsForSession(target.sessionId, updates);
        } else {
          patchSegmentTranslationsForSession(target.sessionId, updates);
        }
      }
      this.ports
        .getTranscriptSidecarStore()
        .updateLlmState({ isTranslationVisible: true }, resolveTargetSidecarKey(target));
      return;
    }

    const sessionStore = this.ports.getTranscriptSessionStore();
    if (sessionStore.sourceHistoryId) {
      return;
    }
    if (updates.length > 0) {
      sessionStore.patchSegmentTranslations(updates);
    }
    this.ports
      .getTranscriptSidecarStore()
      .updateLlmState({ isTranslationVisible: true }, resolveSidecarKey('default', null));
  }
}

export function createTranslationService(ports: TranslationServicePorts): TranslationService {
  return new TranslationService(ports);
}

export const translationService = createTranslationService({
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
  patchSegmentTranslationsForSession,
  patchSegmentTranslationsForHistory,
  commitSystemTranscriptUpdate: (id, segments) =>
    useHistoryStore.getState().commitSystemTranscriptUpdate(id, segments),
  rekeySessionSidecar: (sessionId, nextHistoryId) =>
    useTranscriptStore.getState().rekeySessionSidecar(sessionId, nextHistoryId),
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
  runTranscriptLlmJob,
  translateTranscriptSegments,
  listenToTranscriptLlmJobUpdates,
  toggleLiveTranslation,
  rebaselineTranscriptAutoSave: (historyId, segments) =>
    transcriptAutoSaveRuntime.rebaseline(historyId, segments),
});
