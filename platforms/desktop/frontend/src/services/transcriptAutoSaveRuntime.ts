import { v4 as uuidv4 } from 'uuid';
import type { TranscriptSegment } from '../types/transcript';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';
import { logger } from '../utils/logger';

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
      useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error');
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

      const baseline = this.baselinesBySessionId.get(pending.editSessionId)
        ?? cloneSegments(pending.segments);
      try {
        logger.info('[AutoSave] Committing transcript edit...', historyId);
        const result = await useHistoryStore.getState().commitTranscriptEdit(
          historyId,
          pending.editSessionId,
          baseline,
          pending.segments,
        );
        if (result.status === 'conflict') {
          this.conflictedSessionIds.add(pending.editSessionId);
          if (this.pendingByHistoryId.get(historyId)?.editSessionId === pending.editSessionId) {
            this.pendingByHistoryId.delete(historyId);
            this.pendingOrder = this.pendingOrder.filter((id) => id !== historyId);
          }
          if (this.editSessionIds.get(historyId) === pending.editSessionId) {
            useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error');
          }
          continue;
        }

        this.baselinesBySessionId.set(pending.editSessionId, cloneSegments(pending.segments));
        if (
          this.editSessionIds.get(historyId) === pending.editSessionId
          && !this.pendingByHistoryId.has(historyId)
        ) {
          useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
        }
      } catch (error) {
        logger.error('[AutoSave] Failed to save:', error);
        if (this.editSessionIds.get(historyId) === pending.editSessionId) {
          useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'error');
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

  async flushPending(historyId?: string | null, segments?: TranscriptSegment[] | null): Promise<void> {
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
