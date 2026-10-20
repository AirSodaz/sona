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
interface ProgressSample {
  lastTime: number;
  lastDownloadedBytes: number;
}
const progressSamples = new Map<string, ProgressSample>();

export async function ensureModelDownloadListener(): Promise<void> {
  const isTauri =
    typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
  if (!isTauri || unlistenProgress !== null) return;

  try {
    unlistenProgress = await listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
      const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);
      if (!id || total <= 0) return;

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
  } catch (err) {
    console.error('Failed to initialize model download progress listener:', err);
  }
}

// Auto-initialize listener if running in browser/tauri
if (typeof window !== 'undefined') {
  void ensureModelDownloadListener();
}

export const useModelDownloadStore = create<ModelDownloadState>((set, get) => ({
  downloads: {},

  startDownload: ({ modelId, modelName, modelType = 'asr', downloadId, onRetry, onCancel }) => {
    progressSamples.delete(downloadId);
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
    const item = get().downloads[modelId];
    if (item) {
      progressSamples.delete(item.downloadId);
    }
    set((state) => {
      const current = state.downloads[modelId];
      if (!current || (downloadId && current.downloadId !== downloadId)) return state;
      return {
        downloads: {
          ...state.downloads,
          [modelId]: {
            ...current,
            status: 'completed',
            progress: 100,
            timestamp: Date.now(),
          },
        },
      };
    });
  },

  failDownload: (modelId, error, downloadId) => {
    const item = get().downloads[modelId];
    if (item) {
      progressSamples.delete(item.downloadId);
    }
    set((state) => {
      const current = state.downloads[modelId];
      if (!current || (downloadId && current.downloadId !== downloadId)) return state;
      return {
        downloads: {
          ...state.downloads,
          [modelId]: {
            ...current,
            status: 'failed',
            error: error ?? current.error,
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
    if (item.status === 'downloading') {
      try {
        item.onCancel?.();
      } catch (err) {
        console.warn(`Error during onCancel for ${modelId}:`, err);
      }
      try {
        await tauriCancelDownload(item.downloadId);
      } catch (err) {
        console.warn(`Backend cancelDownload returned error for ${modelId}:`, err);
      }
      progressSamples.delete(item.downloadId);
      set((state) => {
        const current = state.downloads[modelId];
        if (!current || (downloadId && current.downloadId !== downloadId)) return state;
        return {
          downloads: {
            ...state.downloads,
            [modelId]: {
              ...current,
              status: 'cancelled',
              timestamp: Date.now(),
            },
          },
        };
      });
      return true;
    }
    return false;
  },

  dismissDownload: (modelId) => {
    const item = get().downloads[modelId];
    if (item) {
      progressSamples.delete(item.downloadId);
    }
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
