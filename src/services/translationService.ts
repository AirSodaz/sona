import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/configUtils';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranscriptLlmJobResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from './llmTaskTypes';
import { listenToTranscriptLlmJobUpdates } from './llmTaskEvents';
import {
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from './llm/segmentTask';
import { runTranscriptLlmJob } from './tauri/llm';
import {
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
} from './taskLedgerBuilders';

interface RetryTranslateTranscriptJobOptions {
  segments: TranscriptSegment[];
  historyId: string | null;
  targetLanguage?: string;
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
  translationMap: ReadonlyMap<string, TranslatedSegment>,
): TranscriptSegment[] {
  // Merge by id so translated text reuses the existing timeline, speaker metadata, and
  // other segment fields instead of replacing transcript rows wholesale.
  return segments.map((segment) => {
    const translated = translationMap.get(segment.id);
    return translated ? { ...segment, translation: translated.translation } : segment;
  });
}

export interface TranslationServicePorts {
  getEffectiveConfigSnapshot: typeof getEffectiveConfigSnapshot;
  getTranscriptSessionStore: typeof useTranscriptSessionStore.getState;
  getTranscriptSidecarStore: typeof useTranscriptSidecarStore.getState;
  runConfiguredSegmentTask: typeof runConfiguredSegmentTask;
  runTranscriptSegmentTaskJob: typeof runTranscriptSegmentTaskJob;
  runTranscriptLlmJob: typeof runTranscriptLlmJob;
  listenToTranscriptLlmJobUpdates: typeof listenToTranscriptLlmJobUpdates;
}

export class TranslationService {
  constructor(private readonly ports: TranslationServicePorts) {}

  async translateSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
    onChunkTranslated?: (translatedChunk: TranslatedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
  ): Promise<TranslatedSegment[]> {
    return this.ports.runConfiguredSegmentTask({
      feature: 'translation',
      taskType: 'translate',
      config,
      segments,
      onChunk: onChunkTranslated,
      taskIdOverride,
      buildRequest: ({ taskId, llmConfig, segments: inputSegments }) => this.buildRequest(
        taskId,
        llmConfig,
        config,
        inputSegments,
      ),
    });
  }

  async translateCurrentTranscript() {
    const sessionStore = this.ports.getTranscriptSessionStore();
    await this.retryTranslateTranscriptJob({
      segments: sessionStore.segments,
      historyId: sessionStore.sourceHistoryId,
    });
  }

  async retryTranslateTranscriptJob({
    segments,
    historyId,
    targetLanguage,
  }: RetryTranslateTranscriptJobOptions): Promise<void> {
    const sidecarStore = this.ports.getTranscriptSidecarStore();
    const config = this.ports.getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'translation');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    const resolvedTargetLanguage = targetLanguage || config.translationLanguage || 'zh';
    await this.ports.runTranscriptSegmentTaskJob({
      taskType: 'translate',
      segments,
      sourceHistoryId: historyId,
      targetLanguage: resolvedTargetLanguage,
      onStart: (jobHistoryId) => {
        sidecarStore.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);
      },
      onProgress: (translationProgress, jobHistoryId) => {
        // Progress belongs to the job's original record even if the user navigates away
        // before the backend finishes, so keep writing against the captured history id.
        this.ports.getTranscriptSidecarStore().updateLlmState({ translationProgress }, jobHistoryId);
      },
      runTask: async (taskId, jobHistoryId) => {
        const unlistenJobUpdates = await this.ports.listenToTranscriptLlmJobUpdates(
          taskId,
          'translate',
          (payload) => {
            if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
              return;
            }
            this.applyTranscriptJobUpdate(payload);
          },
        );
        try {
          const result = await this.ports.runTranscriptLlmJob({
            taskId,
            taskType: 'translate',
            jobHistoryId: jobHistoryId === 'current' ? null : jobHistoryId,
            config: llm!,
            segments,
            targetLanguage: resolvedTargetLanguage,
          });
          if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
            return;
          }
          this.applyTranscriptJobUpdate(result);
        } finally {
          unlistenJobUpdates();
        }
      },
      onSuccess: (jobHistoryId) => {
        this.ports.getTranscriptSidecarStore().updateLlmState({ translationProgress: 100 }, jobHistoryId);
      },
      onError: (jobHistoryId) => {
        this.ports.getTranscriptSidecarStore().updateLlmState({ translationProgress: 0 }, jobHistoryId);
      },
      onFinally: (jobHistoryId) => {
        const currentSidecarStore = this.ports.getTranscriptSidecarStore();
        currentSidecarStore.updateLlmState({ isTranslating: false }, jobHistoryId);

        if (!currentSidecarStore.getLlmState(jobHistoryId).isTranslationVisible) {
          currentSidecarStore.updateLlmState({ isTranslationVisible: true }, jobHistoryId);
        }
      },
    });
  }

  applyTranslationsInMemory(
    segments: TranscriptSegment[],
    translations: TranslatedSegment[],
  ): TranscriptSegment[] {
    return applyTranslatedChunkToSegments(segments, buildTranslationMap(translations));
  }

  private buildRequest(
    taskId: string,
    llmConfig: NonNullable<ReturnType<typeof getFeatureLlmConfig>>,
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
  ): TranslateSegmentsRequest {
    return {
      taskId,
      config: llmConfig,
      segments: segments.map(({ id, text }) => ({ id, text })),
      targetLanguage: config.translationLanguage || 'zh',
    };
  }

  private applyTranscriptJobUpdate(payload: TranscriptLlmJobResult) {
    if (!payload.segments) {
      return;
    }

    const currentHistoryId = this.ports.getTranscriptSessionStore().sourceHistoryId || 'current';
    const payloadHistoryId = payload.jobHistoryId || 'current';
    if (currentHistoryId !== payloadHistoryId) {
      return;
    }

    this.ports.getTranscriptSessionStore().setSegments(payload.segments);
  }
}

export function createTranslationService(ports: TranslationServicePorts): TranslationService {
  return new TranslationService(ports);
}

export const translationService = createTranslationService({
  getEffectiveConfigSnapshot,
  getTranscriptSessionStore: useTranscriptSessionStore.getState,
  getTranscriptSidecarStore: useTranscriptSidecarStore.getState,
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
  runTranscriptLlmJob,
  listenToTranscriptLlmJobUpdates,
});
