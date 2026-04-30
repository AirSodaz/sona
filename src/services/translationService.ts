import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { historyService } from './historyService';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/runtime';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import type {
  TranslatedSegment,
  TranslateSegmentsRequest,
} from './llmTaskService';
import {
  applySegmentItemsToTranscriptJob,
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from './llm/segmentTask';

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
    const store = useTranscriptStore.getState();
    const config = store.config;
    const llm = getFeatureLlmConfig(config, 'translation');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    const segments = store.segments;
    await runTranscriptSegmentTaskJob({
      taskType: 'translate',
      segments,
      sourceHistoryId: store.sourceHistoryId,
      onStart: (jobHistoryId) => {
        store.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);
      },
      onProgress: (translationProgress, jobHistoryId) => {
        // Progress belongs to the job's original record even if the user navigates away
        // before the backend finishes, so keep writing against the captured history id.
        useTranscriptStore.getState().updateLlmState({ translationProgress }, jobHistoryId);
      },
      runTask: async (taskId, jobHistoryId) => {
        await this.translateSegmentsWithConfig(
          config,
          segments,
          async (items) => {
            await this.applyTranslations(items, jobHistoryId);
          },
          taskId,
        );
      },
      onSuccess: (jobHistoryId) => {
        useTranscriptStore.getState().updateLlmState({ translationProgress: 100 }, jobHistoryId);
      },
      onError: (jobHistoryId) => {
        useTranscriptStore.getState().updateLlmState({ translationProgress: 0 }, jobHistoryId);
      },
      onFinally: (jobHistoryId) => {
        const currentStore = useTranscriptStore.getState();
        currentStore.updateLlmState({ isTranslating: false }, jobHistoryId);

        if (!currentStore.getLlmState(jobHistoryId).isTranslationVisible) {
          currentStore.updateLlmState({ isTranslationVisible: true }, jobHistoryId);
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

  private async applyTranslations(translations: TranslatedSegment[], jobHistoryId: string) {
    await applySegmentItemsToTranscriptJob({
      jobHistoryId,
      items: translations,
      logLabel: 'TranslationService',
      getCurrentHistoryId: () => useTranscriptStore.getState().sourceHistoryId || 'current',
      applyToCurrentTranscript: (items) => {
        const currentStore = useTranscriptStore.getState();
        this.applyTranslationsToCurrentTranscript(items, currentStore);
      },
      loadTranscript: (filename) => historyService.loadTranscript(filename),
      updateTranscript: (historyId, segmentsToSave) => useHistoryStore.getState().updateTranscript(historyId, segmentsToSave),
      mergeIntoSegments: (segmentsToMerge, items) => {
        const translationMap = buildTranslationMap(items);
        return applyTranslatedChunkToSegments(segmentsToMerge, translationMap);
      },
    });
  }

  private applyTranslationsToCurrentTranscript(
    translations: TranslatedSegment[],
    store: ReturnType<typeof useTranscriptStore.getState>,
  ) {
    translations.forEach(({ id, translation }) => {
      store.updateSegment(id, { translation });
    });
  }
}

export const translationService = new TranslationService();
