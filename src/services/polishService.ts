import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { TranscriptSegment } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { historyService } from './historyService';
import { logger } from '../utils/logger';
import { normalizeError } from '../utils/errorUtils';
import { getFeatureLlmConfig, isLlmConfigComplete } from './llmConfig';
import { resolvePolishPreset } from '../utils/polishPresets';
import { resolvePolishKeywords } from '../utils/polishKeywords';
import {
  createLlmTaskId,
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  PolishSegmentsRequest,
  PolishedSegment,
  PolishTaskChunkPayload,
} from './llmTaskService';

function applyPolishedChunkToSegments(
  segments: TranscriptSegment[],
  polishedChunk: PolishedSegment[],
): TranscriptSegment[] {
  const polishedMap = new Map<string, PolishedSegment>();
  for (let index = 0; index < polishedChunk.length; index += 1) {
    polishedMap.set(polishedChunk[index].id, polishedChunk[index]);
  }

  return segments.map((segment) => {
    const polished = polishedMap.get(segment.id);
    return polished ? { ...segment, text: polished.text } : segment;
  });
}

class PolishService {
  async polishSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets' | 'polishKeywordSets'>,
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
  ): Promise<PolishedSegment[]> {
    const llm = getFeatureLlmConfig(config, 'polish');

    if (!isLlmConfigComplete(llm)) {
      throw new Error('LLM Service not fully configured.');
    }

    if (!segments || segments.length === 0) {
      return [];
    }

    const taskId = taskIdOverride || createLlmTaskId('polish');
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
        request: this.buildRequest(taskId, config, segments),
      });

      if (onChunkPolished && !receivedChunkEvent) {
        await onChunkPolished(polishedSegments);
      }

      return polishedSegments;
    } catch (error) {
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenChunk?.();
    }
  }

  async polishSegments(
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
  ): Promise<PolishedSegment[]> {
    const store = useTranscriptStore.getState();
    return this.polishSegmentsWithConfig(store.config, segments, onChunkPolished);
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

    store.updateLlmState({ isPolishing: true, polishProgress: 0 }, jobHistoryId);

    try {
      const polishedSegments = await this.polishSegmentsWithConfig(
        store.config,
        segments,
        async (items) => {
          receivedChunkEvent = true;
          await this.applyPolishedSegments(items, jobHistoryId);
        },
        taskId,
      );

      if (!receivedChunkEvent) {
        await this.applyPolishedSegments(polishedSegments, jobHistoryId);
      }
    } catch (error) {
      throw new Error(normalizeError(error).message);
    } finally {
      unlistenProgress();
      useTranscriptStore.getState().updateLlmState({
        isPolishing: false,
        polishProgress: 0,
      }, jobHistoryId);
    }
  }

  applyPolishedSegmentsInMemory(
    segments: TranscriptSegment[],
    polishedChunk: PolishedSegment[],
  ): TranscriptSegment[] {
    return applyPolishedChunkToSegments(segments, polishedChunk);
  }

  private buildRequest(
    taskId: string,
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets' | 'polishKeywordSets'>,
    segments: TranscriptSegment[],
  ): PolishSegmentsRequest {
    const preset = resolvePolishPreset(config.polishPresetId, config.polishCustomPresets);

    return {
      taskId,
      config: getFeatureLlmConfig(config, 'polish')!,
      segments: segments.map(({ id, text }) => ({ id, text })),
      context: preset.context,
      keywords: resolvePolishKeywords(config.polishKeywordSets),
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

      await useHistoryStore.getState().updateTranscript(
        jobHistoryId,
        applyPolishedChunkToSegments(bgSegments, [...polishedMap.values()]),
      );
    } catch (error) {
      logger.error('[PolishService] Failed to update background record segments:', error);
    }
  }
}

export const polishService = new PolishService();
