import { invokeTauri } from './tauri/invoke';
import { TauriCommand } from './tauri/commands';
import type { CudaAddonInspection } from '../bindings';
import { listen, type UnlistenFn } from './tauri/platform/events';

export interface CudaAddonDownloadProgress {
  downloadedBytes: number;
  totalBytes: number;
  progressPercent: number;
}

export const cudaAddonService = {
  async getStatus(): Promise<CudaAddonInspection> {
    return invokeTauri(TauriCommand.cudaAddon.getStatus);
  },

  async activate(): Promise<CudaAddonInspection> {
    return invokeTauri(TauriCommand.cudaAddon.activate);
  },

  async downloadAndInstall(options?: {
    mirror?: string;
    version?: string;
    customUrl?: string;
    expectedSha256?: string;
    onProgress?: (progress: CudaAddonDownloadProgress) => void;
  }): Promise<CudaAddonInspection> {
    const downloadId = `cuda-addon-${Date.now()}`;
    let unlisten: UnlistenFn | null = null;

    if (options?.onProgress) {
      unlisten = await listen<[number, number, string]>('download-progress', (event) => {
        const [downloadedBytes, totalBytes, eventId] = event.payload;
        if (eventId === downloadId && options.onProgress) {
          const progressPercent =
            totalBytes > 0
              ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
              : 0;
          options.onProgress({
            downloadedBytes,
            totalBytes,
            progressPercent,
          });
        }
      });
    }

    try {
      return await invokeTauri(TauriCommand.cudaAddon.download, {
        downloadId,
        mirror: options?.mirror,
        version: options?.version,
        customUrl: options?.customUrl,
        expectedSha256: options?.expectedSha256,
      });
    } finally {
      if (unlisten) {
        unlisten();
      }
    }
  },

  async cancelDownload(downloadId: string): Promise<void> {
    await invokeTauri(TauriCommand.app.cancelDownload, { id: downloadId });
  },
};
