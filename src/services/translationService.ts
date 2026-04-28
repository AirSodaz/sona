import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { historyService } from './historyService';
import { logger } from '../utils/logger';
import { normalizeError } from '../utils/errorUtils';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llm/runtime';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment } from '../types/transcript';
import {
  createLlmTaskId,
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  TranslatedSegment,
  TranslateSegmentsRequest,
  TranslateTaskChunkPayload,
} from './llmTaskService';

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
    const llm = getFeatureLlmConfig(config, 'translation');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    if (!segments || segments.length === 0) {
      return [];
    }

    const taskId = taskIdOverride || createLlmTaskId('translate');
    let receivedChunkEvent = false;
    const unlistenChunk = onChunkTranslated
      ? await listenToLlmTaskChunks<TranslateTaskChunkPayload>(
        taskId,
        'translate',
        async ({ items }) => {
          receivedChunkEvent = true;
          await onChunkTranslated(items);
        },
      )
      : () => undefined;

    try {
      const translations = await invoke<TranslatedSegment[]>('translate_transcript_segments', {
        request: this.buildRequest(taskId, config, segments),
      });

      // Some providers still complete in one buffered response. Reuse the same consumer
      // path so callers do not need separate streamed vs non-streamed handling.
      if (onChunkTranslated && !receivedChunkEvent) {
        await onChunkTranslated(translations);
      }

      return translations;
    } catch (error) {
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenChunk();
    }
  }

  async translateCurrentTranscript() {
    const store = useTranscriptStore.getState();
    const config = store.config;
    const llm = getFeatureLlmConfig(config, 'translation');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    const segments = store.segments;
    if (!segments || segments.length === 0) {
      return;
    }

    const jobHistoryId = store.sourceHistoryId || 'current';
    const taskId = createLlmTaskId('translate');
    const unlistenProgress = await listenToLlmTaskProgress(taskId, 'translate', ({ completedChunks, totalChunks }) => {
      // Progress belongs to the job's original record even if the user navigates away
      // before the backend finishes, so keep writing against the captured history id.
      useTranscriptStore.getState().updateLlmState({
        translationProgress: Math.round((completedChunks / Math.max(totalChunks, 1)) * 100),
      }, jobHistoryId);
    });
    let receivedChunkEvent = false;

    store.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);

    try {
      const translations = await this.translateSegmentsWithConfig(
        config,
        segments,
        async (items) => {
          receivedChunkEvent = true;
          await this.applyTranslations(items, jobHistoryId);
        },
        taskId,
      );

      if (!receivedChunkEvent) {
        await this.applyTranslations(translations, jobHistoryId);
      }
      useTranscriptStore.getState().updateLlmState({ translationProgress: 100 }, jobHistoryId);
    } catch (error) {
      useTranscriptStore.getState().updateLlmState({ translationProgress: 0 }, jobHistoryId);
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenProgress();

      const currentStore = useTranscriptStore.getState();
      currentStore.updateLlmState({ isTranslating: false }, jobHistoryId);

      if (!currentStore.getLlmState(jobHistoryId).isTranslationVisible) {
        currentStore.updateLlmState({ isTranslationVisible: true }, jobHistoryId);
      }
    }
  }

  applyTranslationsInMemory(
    segments: TranscriptSegment[],
    translations: TranslatedSegment[],
  ): TranscriptSegment[] {
    return applyTranslatedChunkToSegments(segments, buildTranslationMap(translations));
  }

  private buildRequest(
    taskId: string,
    config: Pick<AppConfig, 'llmSettings' | 'translationLanguage'>,
    segments: TranscriptSegment[],
  ): TranslateSegmentsRequest {
    return {
      taskId,
      config: getFeatureLlmConfig(config, 'translation')!,
      segments: segments.map(({ id, text }) => ({ id, text })),
      targetLanguage: config.translationLanguage || 'zh',
    };
  }

  private async applyTranslations(translations: TranslatedSegment[], jobHistoryId: string) {
    const currentStore = useTranscriptStore.getState();
    const currentHistoryId = currentStore.sourceHistoryId || 'current';

    if (currentHistoryId === jobHistoryId) {
      this.applyTranslationsToCurrentTranscript(translations, currentStore);
      return;
    }

    // If the job started from an unsaved "current" transcript and the user later moved
    // elsewhere, there is no durable record to patch in the background.
    if (jobHistoryId === 'current') {
      return;
    }

    try {
      const bgSegments = await historyService.loadTranscript(`${jobHistoryId}.json`);
      if (!bgSegments) {
        return;
      }

      const translationMap = buildTranslationMap(translations);

      // Background writeback preserves the original job target when the user switches to
      // another transcript mid-run, so finished chunks do not leak into the new screen.
      await useHistoryStore.getState().updateTranscript(
        jobHistoryId,
        applyTranslatedChunkToSegments(bgSegments, translationMap),
      );
    } catch (error) {
      logger.error('[TranslationService] Failed to update background record segments:', error);
    }
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
