import i18next from 'i18next';
import { v4 as uuidv4 } from 'uuid';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { TranscriptSegment } from '../types/transcript';
import { logger } from '../utils/logger';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';

const DEFAULT_AUTO_SAVE_DELAY_MS = 2000;
const LIVE_DRAFT_AUTO_SAVE_DELAY_MS = 500;

type PendingSave = {
  historyId: string;
  editSessionId: string;
  segments: TranscriptSegment[];
};

function cloneSegments(segments: TranscriptSegment[]): TranscriptSegment[] {
  return structuredClone(segments);
}
function getI18nText(key: string, defaultValue: string): string {
  if (i18next.isInitialized) {
    const translated = i18next.t(key, { defaultValue });
    if (translated) return translated;
  }
  return defaultValue;
}

class TranscriptAutoSaveRuntime {
  private timeout: ReturnType<typeof setTimeout> | null = null;

  private lastFingerprint = '';

  private pendingByHistoryId = new Map<string, PendingSave>();

  private pendingOrder: string[] = [];

  private baselinesBySessionId = new Map<string, TranscriptSegment[]>();

  private editSessionIds = new Map<string, string>();

  private conflictedSessionIds = new Set<string>();

  private drainPromise: Promise<void> | null = null;

  private unsubscribe: (() => void) | null = null;

  private beginSession(historyId: string, segments: TranscriptSegment[]) {
    const editSessionId = uuidv4();
    this.editSessionIds.set(historyId, editSessionId);
    this.baselinesBySessionId.set(editSessionId, cloneSegments(segments));
  }

  private enqueueSave(historyId: string, segments: TranscriptSegment[]) {
    if (!this.editSessionIds.has(historyId)) this.beginSession(historyId, segments);
    const editSessionId = this.editSessionIds.get(historyId)!;
    if (this.conflictedSessionIds.has(editSessionId)) {
      const conflictMsg = getI18nText(
        'editor.autosave_conflict',
        'Conflict detected: transcript was modified externally'
      );
      useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error', conflictMsg);
      return;
    }
    if (!this.pendingByHistoryId.has(historyId)) {
      this.pendingOrder.push(historyId);
    }
    this.pendingByHistoryId.set(historyId, {
      historyId,
      editSessionId,
      segments: cloneSegments(segments),
    });
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saving');
    void this.ensureDrain();
  }

  private ensureDrain(): Promise<void> {
    if (!this.drainPromise) {
      this.drainPromise = this.drain().finally(() => {
        this.drainPromise = null;
        if (this.pendingOrder.length > 0) void this.ensureDrain();
      });
    }
    return this.drainPromise;
  }

  private async drain() {
    while (this.pendingOrder.length > 0) {
      const historyId = this.pendingOrder.shift();
      if (!historyId) continue;
      const pending = this.pendingByHistoryId.get(historyId);
      this.pendingByHistoryId.delete(historyId);
      if (!pending || this.conflictedSessionIds.has(pending.editSessionId)) continue;

      const baseline =
        this.baselinesBySessionId.get(pending.editSessionId) ?? cloneSegments(pending.segments);
      try {
        logger.info('[AutoSave] Committing transcript edit...', historyId);
        const result = await useHistoryStore
          .getState()
          .commitTranscriptEdit(historyId, pending.editSessionId, baseline, pending.segments);
        if (result.status === 'conflict') {
          const currentFingerprint = computeSegmentsFingerprint(result.currentSegments);
          const pendingFingerprint = computeSegmentsFingerprint(pending.segments);
          if (currentFingerprint === pendingFingerprint) {
            this.baselinesBySessionId.set(pending.editSessionId, cloneSegments(pending.segments));
            this.conflictedSessionIds.delete(pending.editSessionId);
            if (
              this.editSessionIds.get(historyId) === pending.editSessionId &&
              !this.pendingByHistoryId.has(historyId)
            ) {
              useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
            }
            continue;
          }

          this.conflictedSessionIds.delete(pending.editSessionId);
          this.baselinesBySessionId.delete(pending.editSessionId);

          if (this.editSessionIds.get(historyId) === pending.editSessionId) {
            this.beginSession(historyId, result.currentSegments);

            if (this.pendingByHistoryId.get(historyId)?.editSessionId === pending.editSessionId) {
              this.pendingByHistoryId.delete(historyId);
              this.pendingOrder = this.pendingOrder.filter((id) => id !== historyId);
            }
            const conflictMsg = getI18nText(
              'editor.autosave_conflict',
              'Conflict detected: transcript was modified externally'
            );
            useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error', conflictMsg);
          }
          continue;
        }

        this.baselinesBySessionId.set(pending.editSessionId, cloneSegments(pending.segments));
        if (
          this.editSessionIds.get(historyId) === pending.editSessionId &&
          !this.pendingByHistoryId.has(historyId)
        ) {
          useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
        }
      } catch (error) {
        logger.error('[AutoSave] Failed to save:', error);
        if (this.editSessionIds.get(historyId) === pending.editSessionId) {
          const errorMessage =
            error instanceof Error
              ? error.message
              : typeof error === 'string'
                ? error
                : getI18nText('editor.autosave_error', 'Save failed');
          useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error', errorMessage);
        }
      }
    }
  }

  private queueSave(historyId: string, segments: TranscriptSegment[], delayMs: number) {
    if (this.timeout) clearTimeout(this.timeout);
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saving');
    this.timeout = setTimeout(() => {
      this.timeout = null;
      this.enqueueSave(historyId, segments);
    }, delayMs);
  }

  async flushPending(
    historyId?: string | null,
    segments?: TranscriptSegment[] | null
  ): Promise<void> {
    if (this.timeout) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }

    const targetHistoryId = historyId ?? useTranscriptSessionStore.getState().sourceHistoryId;
    const targetSegments = segments ?? useTranscriptSessionStore.getState().segments;
    if (targetHistoryId && targetSegments) {
      this.enqueueSave(targetHistoryId, targetSegments);
    }
    await this.ensureDrain();
  }

  rebaseline(historyId: string, segments: TranscriptSegment[]): void {
    if (this.timeout) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }
    this.pendingByHistoryId.delete(historyId);
    this.pendingOrder = this.pendingOrder.filter((id) => id !== historyId);

    const oldSessionId = this.editSessionIds.get(historyId);
    if (oldSessionId) {
      this.conflictedSessionIds.delete(oldSessionId);
      this.baselinesBySessionId.delete(oldSessionId);
    }

    this.beginSession(historyId, segments);
    this.lastFingerprint = computeSegmentsFingerprint(segments);
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
  }

  start() {
    if (this.unsubscribe) return;

    const initial = useTranscriptSessionStore.getState();
    this.lastFingerprint = computeSegmentsFingerprint(initial.segments);
    if (initial.sourceHistoryId) this.beginSession(initial.sourceHistoryId, initial.segments);

    this.unsubscribe = useTranscriptSessionStore.subscribe((state, prevState) => {
      const currentId = state.sourceHistoryId;
      const prevId = prevState.sourceHistoryId;

      if (prevId !== currentId) {
        if (this.timeout && prevId) {
          clearTimeout(this.timeout);
          this.timeout = null;
          this.enqueueSave(prevId, prevState.segments);
        }
        if (currentId) this.beginSession(currentId, state.segments);
        this.lastFingerprint = computeSegmentsFingerprint(state.segments);
        return;
      }

      if (!currentId || state.segments === prevState.segments) return;
      const currentFingerprint = computeSegmentsFingerprint(state.segments);
      if (currentFingerprint === this.lastFingerprint) return;

      const llmState = useTranscriptSidecarStore.getState().llmStates[currentId];
      if (llmState?.isPolishing || llmState?.isTranslating || llmState?.isRetranscribing) {
        this.lastFingerprint = currentFingerprint;
        return;
      }

      this.lastFingerprint = currentFingerprint;
      const delayMs =
        useTranscriptRuntimeStore.getState().mode === 'live'
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
