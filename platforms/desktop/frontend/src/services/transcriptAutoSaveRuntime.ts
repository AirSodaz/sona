import i18next from 'i18next';
import { v4 as uuidv4 } from 'uuid';
import { registerAutoSaveLifecycleHooks } from '../stores/autoSaveHooks';
import { addHistoryTranscriptUpdateListener, useHistoryStore } from '../stores/historyStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import {
  type SessionData,
  type TranscriptStore,
  useTranscriptStore,
} from '../stores/transcriptStore';
import type { TranscriptSegment } from '../types/transcript';
import { logger } from '../utils/logger';
import { computeSegmentsFingerprint } from '../utils/segmentUtils';
import { documentMutationCoordinator } from './task/documentMutationCoordinator';

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

function findSessionByHistoryId(state: TranscriptStore, historyId: string): SessionData | null {
  const direct = state.sessions[historyId];
  if (direct && (!direct.sourceHistoryId || direct.sourceHistoryId === historyId)) {
    return direct;
  }
  for (const session of Object.values(state.sessions)) {
    if (session.sourceHistoryId === historyId) {
      return session;
    }
  }
  return direct ?? null;
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

  private handleSessionUpdate(
    historyId: string,
    segments: TranscriptSegment[],
    isLive: boolean,
    sessionId?: string | null
  ) {
    const currentFingerprint = computeSegmentsFingerprint(segments);
    const lastFingerprint = this.lastFingerprints.get(historyId);
    if (currentFingerprint === lastFingerprint) return;

    const llmState = useTranscriptSidecarStore.getState().llmStates[historyId];
    if (
      llmState?.isPolishing ||
      llmState?.isTranslating ||
      llmState?.isRetranscribing ||
      documentMutationCoordinator.isLocked(sessionId, historyId)
    ) {
      this.lastFingerprints.set(historyId, currentFingerprint);
      return;
    }

    this.lastFingerprints.set(historyId, currentFingerprint);
    const delayMs = isLive ? LIVE_DRAFT_AUTO_SAVE_DELAY_MS : DEFAULT_AUTO_SAVE_DELAY_MS;
    this.queueSave(historyId, segments, delayMs);
  }

  /**
   * Cancels and discards all pending debounce timers, queued saves, edit baselines, and auto-save status
   * for the specified history IDs (used on delete / purge / backup evict so evicted sessions never
   * trigger a stale `commitTranscriptEdit`).
   */
  cancelPending(historyIds: string[]): void {
    const normalizedIds = Array.from(new Set(historyIds.filter(Boolean)));
    if (normalizedIds.length === 0) return;

    for (const historyId of normalizedIds) {
      this.clearTimeout(historyId);
      this.pendingByHistoryId.delete(historyId);
      const editSessionId = this.editSessionIds.get(historyId);
      if (editSessionId) {
        this.baselinesBySessionId.delete(editSessionId);
        this.conflictedSessionIds.delete(editSessionId);
        this.editSessionIds.delete(historyId);
      }
      this.lastFingerprints.delete(historyId);
      useTranscriptSidecarStore.getState().clearAutoSaveState(historyId);
    }
    const idSet = new Set(normalizedIds);
    this.pendingOrder = this.pendingOrder.filter((id) => !idSet.has(id));
  }

  cancelAllPending(): void {
    const allIds = Array.from(
      new Set([
        ...this.timeouts.keys(),
        ...this.pendingByHistoryId.keys(),
        ...this.editSessionIds.keys(),
        ...this.lastFingerprints.keys(),
      ])
    );
    this.cancelPending(allIds);
  }

  async flushPendingHistoryIds(historyIds: string[]): Promise<void> {
    const unique = Array.from(new Set(historyIds.filter(Boolean)));
    for (const id of unique) {
      await this.flushPending(id);
    }
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
        findSessionByHistoryId(storeState, historyId)?.segments ??
        (storeState.activeSessionId === historyId &&
        storeState.sessions[storeState.activeSessionId]?.sourceHistoryId === historyId
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
          findSessionByHistoryId(storeState, pendingHistId) ??
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

  hasPendingWork(historyId: string): boolean {
    return this.timeouts.has(historyId) || this.pendingByHistoryId.has(historyId);
  }

  resetBaselineIfTracked(historyId: string, segments: TranscriptSegment[]): void {
    this.lastFingerprints.set(historyId, computeSegmentsFingerprint(segments));
    if (this.editSessionIds.has(historyId) && !this.hasPendingWork(historyId)) {
      const oldSessionId = this.editSessionIds.get(historyId);
      if (oldSessionId) {
        this.conflictedSessionIds.delete(oldSessionId);
        this.baselinesBySessionId.delete(oldSessionId);
      }
      this.beginSession(historyId, segments);
    }
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
      const processedHistoryIds = new Set<string>();

      // Flush pending debounce when active session navigates away from a tracked document
      if (state.activeSessionId !== prevState.activeSessionId) {
        const prevActiveSession = prevState.sessions[prevState.activeSessionId];
        const prevActiveHistoryId = prevActiveSession?.sourceHistoryId;
        if (prevActiveHistoryId && this.timeouts.has(prevActiveHistoryId)) {
          this.clearTimeout(prevActiveHistoryId);
          if (prevActiveSession?.segments) {
            this.enqueueSave(prevActiveHistoryId, prevActiveSession.segments);
          }
        }
      }

      // Track all mounted history-backed sessions (both foreground and background hosts)
      for (const [sessionId, prevSession] of Object.entries(prevState.sessions)) {
        const prevHistoryId = prevSession?.sourceHistoryId;
        if (!prevHistoryId) continue;

        const currSession = state.sessions[sessionId];
        const currHistoryId = currSession?.sourceHistoryId;
        if (prevHistoryId !== currHistoryId) {
          if (this.timeouts.has(prevHistoryId)) {
            this.clearTimeout(prevHistoryId);
            if (prevSession.segments) {
              this.enqueueSave(prevHistoryId, prevSession.segments);
            }
          }
        }
      }

      for (const [sessionId, currSession] of Object.entries(state.sessions)) {
        const currHistoryId = currSession?.sourceHistoryId;
        if (!currHistoryId) continue;

        const prevSession = prevState.sessions[sessionId];
        const prevHistoryId = prevSession?.sourceHistoryId;

        if (prevHistoryId !== currHistoryId) {
          if (!this.editSessionIds.has(currHistoryId)) {
            this.beginSession(currHistoryId, currSession.segments);
          }
          this.lastFingerprints.set(
            currHistoryId,
            computeSegmentsFingerprint(currSession.segments)
          );
          processedHistoryIds.add(currHistoryId);
        } else if (currSession.segments !== prevSession?.segments) {
          const isLive = state.isRecording
            ? currHistoryId === state.recordingSessionId || sessionId === state.recordingSessionId
            : state.mode === 'create' && state.ingestionFocus === 'live';
          this.handleSessionUpdate(currHistoryId, currSession.segments, isLive, sessionId);
          processedHistoryIds.add(currHistoryId);
        }
      }

      // Track active recording session (even before/across draft metadata binding)
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

      if (
        currRecId &&
        currRecHistoryId &&
        !processedHistoryIds.has(currRecHistoryId) &&
        currRecSession &&
        currRecSession.segments !== prevRecSession?.segments
      ) {
        this.handleSessionUpdate(currRecHistoryId, currRecSession.segments, true, currRecId);
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

registerAutoSaveLifecycleHooks({
  cancelPending: (historyIds) => transcriptAutoSaveRuntime.cancelPending(historyIds),
  rebaseline: (historyId, segments) => transcriptAutoSaveRuntime.rebaseline(historyId, segments),
});
