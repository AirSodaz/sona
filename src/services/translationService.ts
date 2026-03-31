import { invoke } from '@tauri-apps/api/core';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from './historyService';
import { logger } from '../utils/logger';
import { normalizeError } from '../utils/errorUtils';
import { getActiveLlmConfig, isLlmConfigComplete } from './llmConfig';
import {
  createLlmTaskId,
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  TranslatedSegment,
  TranslateSegmentsRequest,
  TranslateTaskChunkPayload,
} from './llmTaskService';

class TranslationService {
  async translateCurrentTranscript() {
    const store = useTranscriptStore.getState();
    const config = store.config;
    const llm = getActiveLlmConfig(config);

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
      useTranscriptStore.getState().updateLlmState({
        translationProgress: Math.round((completedChunks / Math.max(totalChunks, 1)) * 100),
      }, jobHistoryId);
    });
    let receivedChunkEvent = false;
    const unlistenChunk = await listenToLlmTaskChunks<TranslateTaskChunkPayload>(
      taskId,
      'translate',
      async ({ items }) => {
        receivedChunkEvent = true;
        await this.applyTranslations(items, jobHistoryId);
      },
    );

    store.updateLlmState({ isTranslating: true, translationProgress: 0 }, jobHistoryId);

    try {
      const translations = await invoke<TranslatedSegment[]>('translate_transcript_segments', {
        request: this.buildRequest(taskId),
      });

      if (!receivedChunkEvent) {
        await this.applyTranslations(translations, jobHistoryId);
      }
      useTranscriptStore.getState().updateLlmState({ translationProgress: 100 }, jobHistoryId);
    } catch (error) {
      useTranscriptStore.getState().updateLlmState({ translationProgress: 0 }, jobHistoryId);
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenChunk();
      unlistenProgress();

      const currentStore = useTranscriptStore.getState();
      currentStore.updateLlmState({ isTranslating: false }, jobHistoryId);

      if (!currentStore.getLlmState(jobHistoryId).isTranslationVisible) {
        currentStore.updateLlmState({ isTranslationVisible: true }, jobHistoryId);
      }
    }
  }

  private buildRequest(taskId: string): TranslateSegmentsRequest {
    const store = useTranscriptStore.getState();

    return {
      taskId,
      config: getActiveLlmConfig(store.config),
      segments: store.segments.map(({ id, text }) => ({ id, text })),
      targetLanguage: store.config.translationLanguage || 'zh',
    };
  }

  private async applyTranslations(translations: TranslatedSegment[], jobHistoryId: string) {
    const currentStore = useTranscriptStore.getState();
    const currentHistoryId = currentStore.sourceHistoryId || 'current';

    if (currentHistoryId === jobHistoryId) {
      translations.forEach(({ id, translation }) => {
        currentStore.updateSegment(id, { translation });
      });
      return;
    }

    if (jobHistoryId === 'current') {
      return;
    }

    try {
      const bgSegments = await historyService.loadTranscript(`${jobHistoryId}.json`);
      if (!bgSegments) {
        return;
      }

      const translationMap = new Map<string, TranslatedSegment>();
      for (let i = 0; i < translations.length; i++) {
        translationMap.set(translations[i].id, translations[i]);
      }

      for (let i = 0; i < bgSegments.length; i++) {
        const translated = translationMap.get(bgSegments[i].id);
        if (translated) {
          bgSegments[i].translation = translated.translation;
        }
      }

      await historyService.updateTranscript(jobHistoryId, bgSegments);
    } catch (error) {
      logger.error('[TranslationService] Failed to update background record segments:', error);
    }
  }
}

export const translationService = new TranslationService();
