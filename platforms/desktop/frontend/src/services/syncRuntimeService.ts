import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useSyncStatusStore } from '../stores/syncStatusStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import type { SyncStatusSnapshot } from '../types/sync';
import { logger } from '../utils/logger';
import { getSyncStatus, runSyncNow } from './tauri/sync';
import { subscribeToSyncLocalChanges } from './tauri/syncLocalChangeBus';

export const LOCAL_CHANGE_DEBOUNCE_MS = 5_000;
export const PERIODIC_SYNC_INTERVAL_ACTIVE_MS = 5 * 60 * 1_000;
export const PERIODIC_SYNC_INTERVAL_BACKGROUND_MS = 15 * 60 * 1_000;
export const MIN_FOREGROUND_SYNC_INTERVAL_MS = 30_000;
export const HEARTBEAT_INTERVAL_MS = 10_000;

class SyncRuntimeService {
  private started = false;
  private running = false;
  private queued = false;
  private lastSyncAtMs = 0;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
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
      const requestForegroundSync = () => {
        if (Date.now() - this.lastSyncAtMs >= MIN_FOREGROUND_SYNC_INTERVAL_MS) {
          this.requestSync(0);
        }
      };
      const requestVisibleSync = () => {
        if (document.visibilityState === 'visible') {
          requestForegroundSync();
        }
      };
      const handleOnline = () => this.requestSync(0);
      window.addEventListener('focus', requestForegroundSync);
      window.addEventListener('online', handleOnline);
      document.addEventListener('visibilitychange', requestVisibleSync);
      this.unsubscribers.push(() => {
        window.removeEventListener('focus', requestForegroundSync);
        window.removeEventListener('online', handleOnline);
        document.removeEventListener('visibilitychange', requestVisibleSync);
      });
    }

    this.heartbeatTimer = setInterval(() => {
      this.checkPeriodicSync();
    }, HEARTBEAT_INTERVAL_MS);
    void this.refreshStatus().then((snapshot) => {
      if (snapshot) {
        this.requestSync(0);
      }
    });
  }

  dispose(): void {
    if (this.syncTimer) {
      clearTimeout(this.syncTimer);
      this.syncTimer = null;
    }
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    this.unsubscribers.splice(0).forEach((unsubscribe) => unsubscribe());
    this.started = false;
    this.running = false;
    this.queued = false;
    this.lastSyncAtMs = 0;
  }

  async refreshStatus(): Promise<SyncStatusSnapshot | null> {
    try {
      const snapshot = await getSyncStatus();
      useSyncStatusStore.getState().setSnapshot(snapshot);
      if (snapshot.lastSuccessAtMs && this.lastSyncAtMs === 0) {
        this.lastSyncAtMs = snapshot.lastSuccessAtMs;
      }
      this.scheduleRetry(snapshot);
      return snapshot;
    } catch (error) {
      logger.warn('[Sync] Failed to load status:', error);
      useSyncStatusStore.setState({ isLoaded: true });
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
      this.syncTimer = null;
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
      this.lastSyncAtMs = Date.now();
      useSyncStatusStore.getState().setLastRunResult(result);
    } catch (error) {
      logger.warn('[Sync] Scheduled run failed:', error);
    } finally {
      this.running = false;
      const refreshed = await this.refreshStatus();
      if (refreshed?.lastSuccessAtMs) {
        this.lastSyncAtMs = Math.max(this.lastSyncAtMs, refreshed.lastSuccessAtMs);
      }
      if (this.queued || (refreshed?.pendingOperationCount ?? 0) > 0) {
        this.requestSync(LOCAL_CHANGE_DEBOUNCE_MS);
      }
    }
  }

  private checkPeriodicSync(): void {
    if (this.running || this.syncTimer !== null || this.isBusinessBusy() || !this.isOnline()) {
      return;
    }
    const snapshot = useSyncStatusStore.getState().snapshot;
    if (!this.canRun(snapshot)) {
      return;
    }

    const now = Date.now();
    // If an error occurred with a retry timestamp that has arrived, retry now
    if (snapshot.state === 'error') {
      if (snapshot.nextRetryAtMs && now >= snapshot.nextRetryAtMs) {
        this.requestSync(0);
      }
      return;
    }

    // If pending operations exist and not running/scheduled, sync
    if ((snapshot.pendingOperationCount ?? 0) > 0) {
      this.requestSync(LOCAL_CHANGE_DEBOUNCE_MS);
      return;
    }

    // Periodic pull: check if interval has elapsed since last sync
    const interval = this.isDocumentVisible()
      ? PERIODIC_SYNC_INTERVAL_ACTIVE_MS
      : PERIODIC_SYNC_INTERVAL_BACKGROUND_MS;

    if (now - this.lastSyncAtMs >= interval) {
      this.requestSync(0);
    }
  }

  private scheduleRetry(snapshot: SyncStatusSnapshot): void {
    if (
      snapshot.state === 'error'
      && snapshot.lastError?.retryable
      && snapshot.nextRetryAtMs
    ) {
      const delay = Math.max(0, snapshot.nextRetryAtMs - Date.now());
      if (!this.syncTimer) {
        this.armSyncTimer(delay);
      }
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

  private isDocumentVisible(): boolean {
    return typeof document === 'undefined' || document.visibilityState !== 'hidden';
  }
}

export const syncRuntimeService = new SyncRuntimeService();
