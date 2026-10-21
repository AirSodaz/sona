import { create } from 'zustand';
import { parseDownloadProgressPayload } from '../services/modelDownloadService';
import { cancelDownload as tauriCancelDownload } from '../services/tauri/app';
import { TauriEvent } from '../services/tauri/events';
import { listen, type UnlistenFn } from '../services/tauri/platform/events';

export type ModelDownloadType = 'asr' | 'llm' | 'companion' | 'other';
export type ModelDownloadStatus = 'downloading' | 'completed' | 'failed' | 'cancelled';

export interface ModelDownloadItem {
  id: string; // downloadId or modelId
  modelId: string;
  modelName: string;
  modelType: ModelDownloadType;
  status: ModelDownloadStatus;
  progress: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: string;
  error?: string;
  timestamp: number;
  downloadId: string;
  onRetry?: () => Promise<void> | void;
  onCancel?: () => void;
}

export interface StartDownloadParams {
  modelId: string;
  modelName: string;
  modelType?: ModelDownloadType;
  downloadId: string;
  onRetry?: () => Promise<void> | void;
  onCancel?: () => void;
}

interface ModelDownloadState {
  downloads: Record<string, ModelDownloadItem>;
  startDownload: (params: StartDownloadParams) => void;
  updateProgress: (
    identifier: string,
    update: {
      progress: number;
      downloadedBytes?: number;
      totalBytes?: number;
      speed?: string;
    }
  ) => void;
  completeDownload: (modelId: string, downloadId?: string) => void;
  failDownload: (modelId: string, error?: string, downloadId?: string) => void;
  cancelDownload: (modelId: string, downloadId?: string) => Promise<boolean>;
  dismissDownload: (modelId: string) => void;
  clearCompleted: () => void;
  clearAllNonActive: () => void;
}

let unlistenProgress: UnlistenFn | null = null;
let listenerPromise: Promise<void> | null = null;
let listenerGeneration = 0;
interface ProgressSample {
  lastTime: number;
  lastDownloadedBytes: number;
}
const progressSamples = new Map<string, ProgressSample>();

export async function ensureModelDownloadListener(): Promise<void> {
  const isTauri =
    typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
  if (!isTauri || unlistenProgress !== null) return;
  if (listenerPromise !== null) return listenerPromise;

  const currentGen = ++listenerGeneration;

  listenerPromise = (async () => {
    try {
      const unlisten = await listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
        const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);
        if (!id || total <= 0) return;

        const currentDownloads = useModelDownloadStore.getState().downloads;
        const isActive = Object.values(currentDownloads).some(
          (item) => (item.downloadId === id || item.modelId === id) && item.status === 'downloading'
        );
        if (!isActive) {
          progressSamples.delete(id);
          return;
        }

        const now = Date.now();
        const sample = progressSamples.get(id);

        let speedStr: string | undefined;
        if (!sample) {
          progressSamples.set(id, { lastTime: now, lastDownloadedBytes: downloaded });
        } else {
          const timeDiff = now - sample.lastTime;
          if (timeDiff > 400) {
            const bytesDiff = Math.max(0, downloaded - sample.lastDownloadedBytes);
            const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
            speedStr =
              speedBytesPerSec > 1024 * 1024
                ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
                : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

            progressSamples.set(id, { lastTime: now, lastDownloadedBytes: downloaded });
          }
        }

        const progress = Math.min(100, Math.round((downloaded / total) * 100));
        useModelDownloadStore.getState().updateProgress(id, {
          progress,
          downloadedBytes: downloaded,
          totalBytes: total,
          speed: speedStr,
        });
      });

      if (currentGen !== listenerGeneration) {
        unlisten();
        unlistenProgress = null;
      } else {
        unlistenProgress = unlisten;
      }
    } catch (err) {
      console.error('Failed to initialize model download progress listener:', err);
    } finally {
      if (currentGen === listenerGeneration) {
        listenerPromise = null;
      }
    }
  })();

  return listenerPromise;
}

export function cleanupModelDownloadListener(): void {
  listenerGeneration++;
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  listenerPromise = null;
  progressSamples.clear();
}

// Auto-initialize listener if running in browser/tauri
if (typeof window !== 'undefined') {
  void ensureModelDownloadListener();
}

export const useModelDownloadStore = create<ModelDownloadState>((set, get) => ({
  downloads: {},

  startDownload: ({ modelId, modelName, modelType = 'asr', downloadId, onRetry, onCancel }) => {
    void ensureModelDownloadListener();
    progressSamples.delete(downloadId);
    progressSamples.delete(modelId);
    set((state) => ({
      downloads: {
        ...state.downloads,
        [modelId]: {
          id: downloadId,
          modelId,
          modelName,
          modelType,
          status: 'downloading',
          progress: 0,
          timestamp: Date.now(),
          downloadId,
          onRetry,
          onCancel,
        },
      },
    }));
  },

  updateProgress: (identifier, { progress, downloadedBytes, totalBytes, speed }) => {
    set((state) => {
      let targetModelId: string | null = null;
      for (const [mid, item] of Object.entries(state.downloads)) {
        if (item.downloadId === identifier) {
          targetModelId = mid;
          break;
        }
      }
      if (!targetModelId && state.downloads[identifier]) {
        targetModelId = identifier;
      }

      if (!targetModelId) return state;

      const current = state.downloads[targetModelId];
      if (current?.status !== 'downloading') return state;

      return {
        downloads: {
          ...state.downloads,
          [targetModelId]: {
            ...current,
            progress,
            downloadedBytes: downloadedBytes ?? current.downloadedBytes,
            totalBytes: totalBytes ?? current.totalBytes,
            speed: speed ?? current.speed,
          },
        },
      };
    });
  },

  completeDownload: (modelId, downloadId) => {
    set((state) => {
      const current = state.downloads[modelId];
      if (
        !current ||
        (downloadId && current.downloadId !== downloadId) ||
        current.status !== 'downloading'
      ) {
        return state;
      }
      progressSamples.delete(current.downloadId);
      progressSamples.delete(modelId);
      return {
        downloads: {
          ...state.downloads,
          [modelId]: {
            ...current,
            status: 'completed',
            progress: 100,
            speed: undefined,
            onCancel: undefined,
            onRetry: undefined,
            timestamp: Date.now(),
          },
        },
      };
    });
  },

  failDownload: (modelId, error, downloadId) => {
    set((state) => {
      const current = state.downloads[modelId];
      if (
        !current ||
        (downloadId && current.downloadId !== downloadId) ||
        current.status !== 'downloading'
      ) {
        return state;
      }
      progressSamples.delete(current.downloadId);
      progressSamples.delete(modelId);
      return {
        downloads: {
          ...state.downloads,
          [modelId]: {
            ...current,
            status: 'failed',
            error: error ?? current.error,
            speed: undefined,
            onCancel: undefined,
            timestamp: Date.now(),
          },
        },
      };
    });
  },

  cancelDownload: async (modelId, downloadId) => {
    const item = get().downloads[modelId];
    if (!item || (downloadId && item.downloadId !== downloadId)) {
      return false;
    }
    const targetDownloadId = downloadId ?? item.downloadId;
    if (item.status === 'downloading') {
      const onCancelCallback = item.onCancel;
      progressSamples.delete(targetDownloadId);
      progressSamples.delete(modelId);
      set((state) => {
        const current = state.downloads[modelId];
        if (
          !current ||
          current.downloadId !== targetDownloadId ||
          current.status !== 'downloading'
        ) {
          return state;
        }
        return {
          downloads: {
            ...state.downloads,
            [modelId]: {
              ...current,
              status: 'cancelled',
              speed: undefined,
              onCancel: undefined,
              timestamp: Date.now(),
            },
          },
        };
      });

      try {
        onCancelCallback?.();
      } catch (err) {
        console.warn(`Error during onCancel for ${modelId}:`, err);
      }
      try {
        await tauriCancelDownload(targetDownloadId);
      } catch (err) {
        console.warn(`Backend cancelDownload returned error for ${modelId}:`, err);
      }
      return true;
    }
    return false;
  },

  dismissDownload: (modelId) => {
    const item = get().downloads[modelId];
    if (item) {
      progressSamples.delete(item.downloadId);
    }
    progressSamples.delete(modelId);
    set((state) => {
      const next = { ...state.downloads };
      delete next[modelId];
      return { downloads: next };
    });
  },

  clearCompleted: () => {
    set((state) => {
      const next: Record<string, ModelDownloadItem> = {};
      for (const [mid, item] of Object.entries(state.downloads)) {
        if (item.status !== 'completed') {
          next[mid] = item;
        }
      }
      return { downloads: next };
    });
  },

  clearAllNonActive: () => {
    set((state) => {
      const next: Record<string, ModelDownloadItem> = {};
      for (const [mid, item] of Object.entries(state.downloads)) {
        if (item.status === 'downloading') {
          next[mid] = item;
        }
      }
      return { downloads: next };
    });
  },
}));
