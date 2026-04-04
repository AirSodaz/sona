import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { TranscriptSegment } from '../types/transcript';
import { POLISH_SCENARIO_PROMPTS } from '../utils/polishPrompts';
import { historyService } from './historyService';
import { logger } from '../utils/logger';
import { normalizeError } from '../utils/errorUtils';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llmConfig';
import {
  createLlmTaskId,
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  PolishSegmentsRequest,
  PolishedSegment,
  PolishTaskChunkPayload,
} from './llmTaskService';

class PolishService {
  async polishSegments(
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
  ): Promise<void> {
    const store = useTranscriptStore.getState();
    const llm = getFeatureLlmConfig(store.config, 'polish');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    if (!segments || segments.length === 0) {
      return;
    }

    const taskId = createLlmTaskId('polish');
    let unlistenChunk: UnlistenFn | null = null;
    let receivedChunkEvent = false;

    try {
      if (onChunkPolished) {
        unlistenChunk = await listenToLlmTaskChunks<PolishTaskChunkPayload>(
          taskId,
          'polish',
          async ({ items }) => {
            receivedChunkEvent = true;
            await onChunkPolished(items);
          },
        );
      }

      const polishedSegments = await invoke<PolishedSegment[]>('polish_transcript_segments', {
        request: this.buildRequest(taskId, segments),
      });

      if (onChunkPolished && !receivedChunkEvent) {
        await onChunkPolished(polishedSegments);
      }
    } catch (error) {
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenChunk?.();
    }
  }

  async polishTranscript() {
    const store = useTranscriptStore.getState();
    const segments = store.segments;

    if (!segments || segments.length === 0) {
      return;
    }

    const jobHistoryId = store.sourceHistoryId || 'current';
    const taskId = createLlmTaskId('polish');
    let receivedChunkEvent = false;
    const unlistenProgress = await listenToLlmTaskProgress(taskId, 'polish', ({ completedChunks, totalChunks }) => {
      useTranscriptStore.getState().updateLlmState({
        polishProgress: Math.round((completedChunks / Math.max(totalChunks, 1)) * 100),
      }, jobHistoryId);
    });
    const unlistenChunk = await listenToLlmTaskChunks<PolishTaskChunkPayload>(
      taskId,
      'polish',
      async ({ items }) => {
        receivedChunkEvent = true;
        await this.applyPolishedSegments(items, jobHistoryId);
      },
    );

    store.updateLlmState({ isPolishing: true, polishProgress: 0 }, jobHistoryId);

    try {
      const polishedSegments = await invoke<PolishedSegment[]>('polish_transcript_segments', {
        request: this.buildRequest(taskId, segments),
      });

      if (!receivedChunkEvent) {
        await this.applyPolishedSegments(polishedSegments, jobHistoryId);
      }
    } catch (error) {
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenChunk();
      unlistenProgress();
      useTranscriptStore.getState().updateLlmState({
        isPolishing: false,
        polishProgress: 0,
      }, jobHistoryId);
    }
  }

  private buildRequest(taskId: string, segments: TranscriptSegment[]): PolishSegmentsRequest {
    const config = useConfigStore.getState().config;
    const scenario = config.polishScenario || 'custom';

    return {
      taskId,
      config: getFeatureLlmConfig(config, 'polish')!,
      segments: segments.map(({ id, text }) => ({ id, text })),
      context: scenario === 'custom' ? (config.polishContext || '') : '',
      keywords: config.polishKeywords || '',
      scenarioPrompt: scenario === 'custom' ? '' : (POLISH_SCENARIO_PROMPTS[scenario] || ''),
    };
  }

  private async applyPolishedSegments(polishedSegments: PolishedSegment[], jobHistoryId: string) {
    const currentStore = useTranscriptStore.getState();
    const currentHistoryId = currentStore.sourceHistoryId || 'current';

    if (currentHistoryId === jobHistoryId) {
      polishedSegments.forEach(({ id, text }) => {
        currentStore.updateSegment(id, { text });
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

      const polishedMap = new Map<string, PolishedSegment>();
      for (let i = 0; i < polishedSegments.length; i++) {
        polishedMap.set(polishedSegments[i].id, polishedSegments[i]);
      }

      for (let i = 0; i < bgSegments.length; i++) {
        const polished = polishedMap.get(bgSegments[i].id);
        if (polished) {
          bgSegments[i].text = polished.text;
        }
      }

      await historyService.updateTranscript(jobHistoryId, bgSegments);
    } catch (error) {
      logger.error('[PolishService] Failed to update background record segments:', error);
    }
  }
}

export const polishService = new PolishService();
