import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/runtime';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranscriptLlmJobResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from './llmTaskService';
import { listenToTranscriptLlmJobUpdates } from './llmTaskService';
import {
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from './llm/segmentTask';
import { runTranscriptLlmJob } from './tauri/llm';
import {
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
} from './taskLedgerRuntime';

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

class TranslationService {
  async translateSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
    onChunkTranslated?: (translatedChunk: TranslatedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
  ): Promise<TranslatedSegment[]> {
    return runConfiguredSegmentTask({
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
    const sessionStore = useTranscriptSessionStore.getState();
    const sidecarStore = useTranscriptSidecarStore.getState();
    const config = getEffectiveConfigSnapshot();
    const llm = getFeatureLlmConfig(config, 'translation');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    const segments = sessionStore.segments;
    await runTranscriptSegmentTaskJob({
      taskType: 'translate',
      segments,
      sourceHistoryId: sessionStore.sourceHistoryId,
      targetLanguage: config.translationLanguage || 'zh',
      onStart: (jobHistoryId) => {
        sidecarStore.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);
      },
      onProgress: (translationProgress, jobHistoryId) => {
        // Progress belongs to the job's original record even if the user navigates away
        // before the backend finishes, so keep writing against the captured history id.
        useTranscriptSidecarStore.getState().updateLlmState({ translationProgress }, jobHistoryId);
      },
      runTask: async (taskId, jobHistoryId) => {
        const unlistenJobUpdates = await listenToTranscriptLlmJobUpdates(
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
          const result = await runTranscriptLlmJob({
            taskId,
            taskType: 'translate',
            jobHistoryId: jobHistoryId === 'current' ? null : jobHistoryId,
            config: llm!,
            segments,
            targetLanguage: config.translationLanguage || 'zh',
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
        useTranscriptSidecarStore.getState().updateLlmState({ translationProgress: 100 }, jobHistoryId);
      },
      onError: (jobHistoryId) => {
        useTranscriptSidecarStore.getState().updateLlmState({ translationProgress: 0 }, jobHistoryId);
      },
      onFinally: (jobHistoryId) => {
        const currentSidecarStore = useTranscriptSidecarStore.getState();
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

    const currentHistoryId = useTranscriptSessionStore.getState().sourceHistoryId || 'current';
    const payloadHistoryId = payload.jobHistoryId || 'current';
    if (currentHistoryId !== payloadHistoryId) {
      return;
    }

    useTranscriptSessionStore.getState().setSegments(payload.segments);
  }
}

export const translationService = new TranslationService();
