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
}

export interface StartDownloadParams {
  modelId: string;
  modelName: string;
  modelType?: ModelDownloadType;
  downloadId: string;
  onRetry?: () => Promise<void> | void;
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
  completeDownload: (modelId: string) => void;
  failDownload: (modelId: string, error: string) => void;
  cancelDownload: (modelId: string) => Promise<void>;
  dismissDownload: (modelId: string) => void;
  clearCompleted: () => void;
  clearAllNonActive: () => void;
}

let unlistenProgress: UnlistenFn | null = null;
let lastProgressTime = 0;
const lastBytesMap = new Map<string, number>();

export async function ensureModelDownloadListener(): Promise<void> {
  const isTauri =
    typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window);
  if (!isTauri || unlistenProgress !== null) return;

  try {
    unlistenProgress = await listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
      const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);
      if (!id || total <= 0) return;

      const now = Date.now();
      const prevDownloaded = lastBytesMap.get(id) ?? 0;
      const timeDiff = now - lastProgressTime;

      let speedStr: string | undefined;
      if (timeDiff > 400) {
        const bytesDiff = Math.max(0, downloaded - prevDownloaded);
        const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
        speedStr =
          speedBytesPerSec > 1024 * 1024
            ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
            : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

        lastBytesMap.set(id, downloaded);
        lastProgressTime = now;
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

  startDownload: ({ modelId, modelName, modelType = 'asr', downloadId, onRetry }) => {
    lastBytesMap.set(downloadId, 0);
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
        },
      },
    }));
  },

  updateProgress: (identifier, { progress, downloadedBytes, totalBytes, speed }) => {
    set((state) => {
      // Find item by modelId or downloadId
      let targetModelId: string | null = null;
      if (state.downloads[identifier]) {
        targetModelId = identifier;
      } else {
        for (const [mid, item] of Object.entries(state.downloads)) {
          if (item.downloadId === identifier) {
            targetModelId = mid;
            break;
          }
        }
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

  completeDownload: (modelId) => {
    set((state) => {
      const current = state.downloads[modelId];
      if (!current) return state;
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

  failDownload: (modelId, error) => {
    set((state) => {
      const current = state.downloads[modelId];
      if (!current) return state;
      return {
        downloads: {
          ...state.downloads,
          [modelId]: {
            ...current,
            status: 'failed',
            error,
            timestamp: Date.now(),
          },
        },
      };
    });
  },

  cancelDownload: async (modelId) => {
    const item = get().downloads[modelId];
    if (item && item.status === 'downloading') {
      try {
        await tauriCancelDownload(item.downloadId);
      } catch (err) {
        console.error(`Failed to cancel download for ${modelId}:`, err);
      }
      set((state) => {
        const current = state.downloads[modelId];
        if (!current) return state;
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
    }
  },

  dismissDownload: (modelId) => {
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
