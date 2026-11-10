import i18next from 'i18next';
import { v4 as uuidv4 } from 'uuid';
import { addHistoryTranscriptUpdateListener, useHistoryStore } from '../stores/historyStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import { useTranscriptStore } from '../stores/transcriptStore';
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
  private timeouts = new Map<string, ReturnType<typeof setTimeout>>();

  private lastFingerprints = new Map<string, string>();

  private pendingByHistoryId = new Map<string, PendingSave>();

  private pendingOrder: string[] = [];

  private baselinesBySessionId = new Map<string, TranscriptSegment[]>();

  private editSessionIds = new Map<string, string>();

  private conflictedSessionIds = new Set<string>();

  private drainPromise: Promise<void> | null = null;

  private unsubscribe: (() => void) | null = null;
  private unsubscribeHistoryUpdate: (() => void) | null = null;

  private clearTimeout(historyId: string): void {
    const timer = this.timeouts.get(historyId);
    if (timer) {
      clearTimeout(timer);
      this.timeouts.delete(historyId);
    }
  }

  private clearAllTimeouts(): void {
    for (const timer of this.timeouts.values()) {
      clearTimeout(timer);
    }
    this.timeouts.clear();
  }

  private beginSession(historyId: string, segments: TranscriptSegment[]): string {
    const editSessionId = uuidv4();
    this.editSessionIds.set(historyId, editSessionId);
    this.baselinesBySessionId.set(editSessionId, cloneSegments(segments));
    return editSessionId;
  }

  private enqueueSave(historyId: string, segments: TranscriptSegment[]) {
    const editSessionId =
      this.editSessionIds.get(historyId) ?? this.beginSession(historyId, segments);
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
    this.clearTimeout(historyId);
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saving');
    const timer = setTimeout(() => {
      this.timeouts.delete(historyId);
      this.enqueueSave(historyId, segments);
    }, delayMs);
    this.timeouts.set(historyId, timer);
  }

  private handleSessionUpdate(historyId: string, segments: TranscriptSegment[], isLive: boolean) {
    const currentFingerprint = computeSegmentsFingerprint(segments);
    const lastFingerprint = this.lastFingerprints.get(historyId);
    if (currentFingerprint === lastFingerprint) return;

    const llmState = useTranscriptSidecarStore.getState().llmStates[historyId];
    if (llmState?.isPolishing || llmState?.isTranslating || llmState?.isRetranscribing) {
      this.lastFingerprints.set(historyId, currentFingerprint);
      return;
    }

    this.lastFingerprints.set(historyId, currentFingerprint);
    const delayMs = isLive ? LIVE_DRAFT_AUTO_SAVE_DELAY_MS : DEFAULT_AUTO_SAVE_DELAY_MS;
    this.queueSave(historyId, segments, delayMs);
  }

  async flushPending(
    historyId?: string | null,
    segments?: TranscriptSegment[] | null
  ): Promise<void> {
    const storeState = useTranscriptStore.getState();

    if (historyId) {
      this.clearTimeout(historyId);
      const targetSegments =
        segments ??
        storeState.sessions[historyId]?.segments ??
        (storeState.activeSessionId === historyId
          ? storeState.sessions[storeState.activeSessionId]?.segments
          : null);
      if (targetSegments) {
        this.enqueueSave(historyId, targetSegments);
      }
    } else {
      const flushedHistoryIds = new Set<string>();

      // Flush any pending debounce timeouts across all sessions
      for (const pendingHistId of Array.from(this.timeouts.keys())) {
        this.clearTimeout(pendingHistId);
        const session =
          storeState.sessions[pendingHistId] ??
          (storeState.activeSessionId === pendingHistId
            ? storeState.sessions[storeState.activeSessionId]
            : null);
        if (session?.segments) {
          this.enqueueSave(pendingHistId, session.segments);
          flushedHistoryIds.add(pendingHistId);
        }
      }

      // Check activeSessionId
      const activeSession = storeState.sessions[storeState.activeSessionId];
      const activeHistoryId = activeSession?.sourceHistoryId;
      if (activeHistoryId && !flushedHistoryIds.has(activeHistoryId) && activeSession.segments) {
        this.enqueueSave(activeHistoryId, activeSession.segments);
        flushedHistoryIds.add(activeHistoryId);
      }

      // Check recordingSessionId
      const recId = storeState.recordingSessionId;
      if (recId) {
        const recSession = storeState.sessions[recId];
        const recHistoryId = recSession?.sourceHistoryId || recId;
        if (!flushedHistoryIds.has(recHistoryId) && recSession?.segments) {
          this.enqueueSave(recHistoryId, recSession.segments);
          flushedHistoryIds.add(recHistoryId);
        }
      }
    }
    await this.ensureDrain();
  }

  rebaseline(historyId: string, segments: TranscriptSegment[]): void {
    this.clearTimeout(historyId);
    this.pendingByHistoryId.delete(historyId);
    this.pendingOrder = this.pendingOrder.filter((id) => id !== historyId);

    const oldSessionId = this.editSessionIds.get(historyId);
    if (oldSessionId) {
      this.conflictedSessionIds.delete(oldSessionId);
      this.baselinesBySessionId.delete(oldSessionId);
    }

    this.beginSession(historyId, segments);
    this.lastFingerprints.set(historyId, computeSegmentsFingerprint(segments));
    useTranscriptSidecarStore.getState().setAutoSaveState(historyId, 'saved');
  }

  start() {
    if (this.unsubscribe) return;

    const initial = useTranscriptStore.getState();
    const initialActive = initial.sessions[initial.activeSessionId];
    if (initialActive?.sourceHistoryId) {
      this.beginSession(initialActive.sourceHistoryId, initialActive.segments);
      this.lastFingerprints.set(
        initialActive.sourceHistoryId,
        computeSegmentsFingerprint(initialActive.segments)
      );
    }

    const initialRecId = initial.recordingSessionId;
    if (initialRecId && initialRecId !== initial.activeSessionId) {
      const initialRecSession = initial.sessions[initialRecId];
      const recHistoryId = initialRecSession?.sourceHistoryId || initialRecId;
      if (initialRecSession) {
        this.beginSession(recHistoryId, initialRecSession.segments);
        this.lastFingerprints.set(
          recHistoryId,
          computeSegmentsFingerprint(initialRecSession.segments)
        );
      }
    }

    this.unsubscribeHistoryUpdate = addHistoryTranscriptUpdateListener((historyId, segments) => {
      this.rebaseline(historyId, segments);
    });

    this.unsubscribe = useTranscriptStore.subscribe((state, prevState) => {
      const prevActiveSession = prevState.sessions[prevState.activeSessionId];
      const currActiveSession = state.sessions[state.activeSessionId];
      const prevActiveHistoryId = prevActiveSession?.sourceHistoryId;
      const currActiveHistoryId = currActiveSession?.sourceHistoryId;

      // A. Handle active session / history transition
      if (prevActiveHistoryId !== currActiveHistoryId) {
        if (prevActiveHistoryId && this.timeouts.has(prevActiveHistoryId)) {
          this.clearTimeout(prevActiveHistoryId);
          if (prevActiveSession?.segments) {
            this.enqueueSave(prevActiveHistoryId, prevActiveSession.segments);
          }
        }
        if (currActiveHistoryId && currActiveSession) {
          if (!this.editSessionIds.has(currActiveHistoryId)) {
            this.beginSession(currActiveHistoryId, currActiveSession.segments);
          }
          this.lastFingerprints.set(
            currActiveHistoryId,
            computeSegmentsFingerprint(currActiveSession.segments)
          );
        }
      } else if (
        currActiveHistoryId &&
        currActiveSession &&
        currActiveSession.segments !== prevActiveSession?.segments
      ) {
        const isLive = state.isRecording
          ? currActiveHistoryId === state.recordingSessionId
          : state.mode === 'create' && state.ingestionFocus === 'live';
        this.handleSessionUpdate(currActiveHistoryId, currActiveSession.segments, isLive);
      }

      // B. Handle recording session transition
      const prevRecId = prevState.recordingSessionId;
      const currRecId = state.recordingSessionId;
      const prevRecSession = prevRecId ? prevState.sessions[prevRecId] : null;
      const currRecSession = currRecId ? state.sessions[currRecId] : null;
      const prevRecHistoryId = prevRecSession?.sourceHistoryId || prevRecId;
      const currRecHistoryId = currRecSession?.sourceHistoryId || currRecId;

      if (prevRecHistoryId !== currRecHistoryId) {
        if (prevRecHistoryId && this.timeouts.has(prevRecHistoryId)) {
          this.clearTimeout(prevRecHistoryId);
          if (prevRecSession?.segments) {
            this.enqueueSave(prevRecHistoryId, prevRecSession.segments);
          }
        }
        if (currRecHistoryId && currRecSession) {
          if (!this.editSessionIds.has(currRecHistoryId)) {
            this.beginSession(currRecHistoryId, currRecSession.segments);
          }
          this.lastFingerprints.set(
            currRecHistoryId,
            computeSegmentsFingerprint(currRecSession.segments)
          );
        }
      }

      // C. Handle background recording updates
      if (currRecId && currRecId !== state.activeSessionId) {
        if (currRecSession && currRecSession.segments !== prevRecSession?.segments) {
          const recHistoryId = currRecSession.sourceHistoryId || currRecId;
          this.handleSessionUpdate(recHistoryId, currRecSession.segments, true);
        }
      }
    });
  }

  stop() {
    this.unsubscribe?.();
    this.unsubscribe = null;
    this.unsubscribeHistoryUpdate?.();
    this.unsubscribeHistoryUpdate = null;
    this.clearAllTimeouts();
  }
}

export const transcriptAutoSaveRuntime = new TranscriptAutoSaveRuntime();
