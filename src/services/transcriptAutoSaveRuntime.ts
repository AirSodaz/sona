import type { TranscriptSegment } from '../types/transcript';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';
import { logger } from '../utils/logger';

const DEFAULT_AUTO_SAVE_DELAY_MS = 2000;
const LIVE_DRAFT_AUTO_SAVE_DELAY_MS = 500;

class TranscriptAutoSaveRuntime {
  private isSaving = false;

  private timeout: ReturnType<typeof setTimeout> | null = null;

  private lastFingerprint = '';

  private pendingHistoryId: string | null = null;

  private pendingSegments: TranscriptSegment[] | null = null;

  private unsubscribe: (() => void) | null = null;

  private async saveToHistory(historyId: string, segments: TranscriptSegment[]) {
    if (this.isSaving) {
      return;
    }

    try {
      this.isSaving = true;
      logger.info('[AutoSave] Saving transcript...', historyId);
      await useHistoryStore.getState().updateTranscript(historyId, segments);
      useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
    } catch (error) {
      logger.error('[AutoSave] Failed to save:', error);
      useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error');
    } finally {
      this.isSaving = false;
    }
  }

  private queueSave(historyId: string, segments: TranscriptSegment[], delayMs: number) {
    if (this.timeout) {
      clearTimeout(this.timeout);
    }

    this.pendingHistoryId = historyId;
    this.pendingSegments = [...segments];
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saving');
    this.timeout = setTimeout(() => {
      this.timeout = null;
      const queuedHistoryId = this.pendingHistoryId;
      const queuedSegments = this.pendingSegments;
      this.pendingHistoryId = null;
      this.pendingSegments = null;
      if (!queuedHistoryId || !queuedSegments) {
        return;
      }
      logger.info('[AutoSave] Debounce triggered for:', queuedHistoryId);
      void this.saveToHistory(queuedHistoryId, queuedSegments);
    }, delayMs);
  }

  async flushPending(historyId?: string | null, segments?: TranscriptSegment[] | null): Promise<void> {
    const targetHistoryId = historyId || this.pendingHistoryId;
    const targetSegments = segments ? [...segments] : this.pendingSegments;

    if (!targetHistoryId || !targetSegments) {
      return;
    }

    if (this.timeout) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }

    if (this.pendingHistoryId === targetHistoryId) {
      this.pendingHistoryId = null;
      this.pendingSegments = null;
    }

    useTranscriptSidecarStore.getState().setAutoSaveState(targetHistoryId, 'saving');
    await this.saveToHistory(targetHistoryId, targetSegments);
  }

  start() {
    if (this.unsubscribe) {
      return;
    }

    this.lastFingerprint = computeSegmentsFingerprint(useTranscriptSessionStore.getState().segments);

    this.unsubscribe = useTranscriptSessionStore.subscribe((state, prevState) => {
      const currentId = state.sourceHistoryId;
      const prevId = prevState.sourceHistoryId;

      if (prevId && prevId !== currentId) {
        if (this.timeout) {
          logger.info('[AutoSave] Switching items, flushing save for:', prevId);
          void this.flushPending(prevId, prevState.segments);
        }

        this.lastFingerprint = computeSegmentsFingerprint(state.segments);
        return;
      }

      if (!currentId || state.segments === prevState.segments) {
        return;
      }

      const currentFingerprint = computeSegmentsFingerprint(state.segments);
      if (currentFingerprint === this.lastFingerprint) {
        return;
      }

      this.lastFingerprint = currentFingerprint;
      const delayMs = useTranscriptRuntimeStore.getState().mode === 'live'
        ? LIVE_DRAFT_AUTO_SAVE_DELAY_MS
        : DEFAULT_AUTO_SAVE_DELAY_MS;
      this.queueSave(currentId, state.segments, delayMs);
    });
  }

  stop() {
    this.unsubscribe?.();
    this.unsubscribe = null;

    if (this.timeout) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }
  }
}

export const transcriptAutoSaveRuntime = new TranscriptAutoSaveRuntime();
