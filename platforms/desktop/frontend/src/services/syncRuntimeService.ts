import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useSyncStatusStore } from '../stores/syncStatusStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import type { SyncStatusSnapshot } from '../types/sync';
import { logger } from '../utils/logger';
import { getSyncStatus, runSyncNow } from './tauri/sync';
import { subscribeToSyncLocalChanges } from './syncLocalChangeBus';

const LOCAL_CHANGE_DEBOUNCE_MS = 5_000;
const STATUS_POLL_INTERVAL_MS = 5_000;

class SyncRuntimeService {
  private started = false;
  private running = false;
  private queued = false;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private unsubscribers: Array<() => void> = [];

  init(): void {
    if (this.started) {
      return;
    }
    this.started = true;
    this.unsubscribers.push(
      subscribeToSyncLocalChanges(() => this.requestSync(LOCAL_CHANGE_DEBOUNCE_MS)),
      useTranscriptRuntimeStore.subscribe((state, previous) => {
        if (previous.isRecording && !state.isRecording) {
          this.flushQueuedSync();
        }
      }),
      useBatchQueueStore.subscribe((state, previous) => {
        if (this.isQueueBusy(previous) && !this.isQueueBusy(state)) {
          this.flushQueuedSync();
        }
      }),
    );

    if (typeof window !== 'undefined') {
      const requestForegroundSync = () => this.requestSync(0);
      const requestVisibleSync = () => {
        if (document.visibilityState === 'visible') {
          requestForegroundSync();
        }
      };
      window.addEventListener('focus', requestForegroundSync);
      window.addEventListener('online', requestForegroundSync);
      document.addEventListener('visibilitychange', requestVisibleSync);
      this.unsubscribers.push(() => {
        window.removeEventListener('focus', requestForegroundSync);
        window.removeEventListener('online', requestForegroundSync);
        document.removeEventListener('visibilitychange', requestVisibleSync);
      });
    }

    this.pollTimer = setInterval(() => {
      void this.pollStatus();
    }, STATUS_POLL_INTERVAL_MS);
    void this.refreshStatus().then((snapshot) => {
      if (snapshot) {
        this.requestSync(0);
      }
    });
  }

  dispose(): void {
    if (this.syncTimer) {
      clearTimeout(this.syncTimer);
    }
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
    }
    this.syncTimer = null;
    this.pollTimer = null;
    this.unsubscribers.splice(0).forEach((unsubscribe) => unsubscribe());
    this.started = false;
    this.running = false;
    this.queued = false;
  }

  async refreshStatus(): Promise<SyncStatusSnapshot | null> {
    try {
      const snapshot = await getSyncStatus();
      useSyncStatusStore.getState().setSnapshot(snapshot);
      this.scheduleRetry(snapshot);
      return snapshot;
    } catch (error) {
      logger.warn('[Sync] Failed to load status:', error);
      return null;
    }
  }

  requestSync(delayMs = LOCAL_CHANGE_DEBOUNCE_MS): void {
    this.queued = true;
    if (this.isBusinessBusy() || !this.isOnline()) {
      return;
    }
    const snapshot = useSyncStatusStore.getState().snapshot;
    if (snapshot.state === 'disabled' || snapshot.state === 'locked' || snapshot.state === 'paused') {
      return;
    }
    const retryDelay = snapshot.nextRetryAtMs
      ? Math.max(0, snapshot.nextRetryAtMs - Date.now())
      : 0;
    this.armSyncTimer(Math.max(delayMs, retryDelay));
  }

  private armSyncTimer(delayMs: number): void {
    if (this.syncTimer) {
      clearTimeout(this.syncTimer);
    }
    this.syncTimer = setTimeout(() => {
      this.syncTimer = null;
      void this.runScheduledSync();
    }, delayMs);
  }

  private async runScheduledSync(): Promise<void> {
    if (this.running || this.isBusinessBusy() || !this.isOnline()) {
      this.queued = true;
      return;
    }
    const snapshot = await this.refreshStatus();
    if (!snapshot || !this.canRun(snapshot)) {
      return;
    }
    if (snapshot.nextRetryAtMs && snapshot.nextRetryAtMs > Date.now()) {
      this.armSyncTimer(snapshot.nextRetryAtMs - Date.now());
      return;
    }

    this.running = true;
    this.queued = false;
    useSyncStatusStore.getState().setSnapshot({ ...snapshot, state: 'syncing' });
    try {
      const result = await runSyncNow();
      useSyncStatusStore.getState().setLastRunResult(result);
    } catch (error) {
      logger.warn('[Sync] Scheduled run failed:', error);
    } finally {
      this.running = false;
      const refreshed = await this.refreshStatus();
      if (this.queued || (refreshed?.pendingOperationCount ?? 0) > 0) {
        this.requestSync(LOCAL_CHANGE_DEBOUNCE_MS);
      }
    }
  }

  private async pollStatus(): Promise<void> {
    const previousPending = useSyncStatusStore.getState().snapshot.pendingOperationCount;
    const snapshot = await this.refreshStatus();
    if (snapshot && snapshot.pendingOperationCount > previousPending) {
      this.requestSync(LOCAL_CHANGE_DEBOUNCE_MS);
    }
  }

  private scheduleRetry(snapshot: SyncStatusSnapshot): void {
    if (
      snapshot.state === 'error'
      && snapshot.lastError?.retryable
      && snapshot.nextRetryAtMs
    ) {
      this.queued = true;
      this.armSyncTimer(Math.max(0, snapshot.nextRetryAtMs - Date.now()));
    }
  }

  private flushQueuedSync(): void {
    if (this.queued) {
      this.requestSync(0);
    }
  }

  private canRun(snapshot: SyncStatusSnapshot): boolean {
    return snapshot.state === 'idle' || (
      snapshot.state === 'error' && Boolean(snapshot.lastError?.retryable)
    );
  }

  private isBusinessBusy(): boolean {
    return useTranscriptRuntimeStore.getState().isRecording
      || this.isQueueBusy(useBatchQueueStore.getState());
  }

  private isQueueBusy(state: ReturnType<typeof useBatchQueueStore.getState>): boolean {
    return state.isQueueProcessing || state.queueItems.some(
      (item) => item.status === 'pending' || item.status === 'processing',
    );
  }

  private isOnline(): boolean {
    return typeof navigator === 'undefined' || navigator.onLine !== false;
  }
}

export const syncRuntimeService = new SyncRuntimeService();
