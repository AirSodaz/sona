import i18n from '../i18n';
import { logger } from '../utils/logger';
import { extractErrorMessage } from '../utils/errorUtils';
import { retryWithBackoff } from '../utils/retryWithBackoff';
import { TauriEvent } from './tauri/events';
import type { ModelCatalogModel, ModelInfo, ProgressCallback } from './modelService';

interface DownloadProgressPayloadObject {
  0?: number;
  1?: number;
  2?: string;
  downloaded?: number;
  total?: number;
  id?: string;
}

type DownloadFile = (input: { url: string; outputPath: string; id: string; expectedSha256?: string }) => Promise<void>;
type ExtractTarBz2 = (input: { archivePath: string; targetDir: string }) => Promise<void>;
type Listen = <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>;

interface ModelDownloadServicePorts {
  downloadFile: DownloadFile;
  extractTarBz2: ExtractTarBz2;
  cancelDownload: (id: string) => Promise<void>;
  remove: (path: string) => Promise<void>;
  listen: Listen;
  join: (...paths: string[]) => Promise<string>;
  getModelsDir: () => Promise<string>;
}

interface DownloadModelInput {
  modelId: string;
  model: ModelInfo | ModelCatalogModel;
  modelsDir?: string;
  onProgress?: ProgressCallback;
  signal?: AbortSignal;
  mirror?: string;
}

export function parseDownloadProgressPayload(payload: unknown): { downloaded: number; total: number; id: string } {
  if (Array.isArray(payload)) {
    const [downloaded, total, id] = payload;
    return {
      downloaded: typeof downloaded === 'number' ? downloaded : 0,
      total: typeof total === 'number' ? total : 0,
      id: typeof id === 'string' ? id : '',
    };
  }

  if (typeof payload === 'object' && payload !== null) {
    const value = payload as DownloadProgressPayloadObject;
    const downloaded = typeof value[0] === 'number'
      ? value[0]
      : typeof value.downloaded === 'number'
        ? value.downloaded
        : 0;
    const total = typeof value[1] === 'number'
      ? value[1]
      : typeof value.total === 'number'
        ? value.total
        : 0;
    const id = typeof value[2] === 'string'
      ? value[2]
      : typeof value.id === 'string'
        ? value.id
        : '';

    return { downloaded, total, id };
  }

  return { downloaded: 0, total: 0, id: '' };
}

function isCatalogModel(model: ModelInfo | ModelCatalogModel): model is ModelCatalogModel {
  return 'installPath' in model && 'downloadPath' in model;
}

class ModelDownloadService {
  constructor(private readonly ports: ModelDownloadServicePorts) {}

  async downloadModel({
    modelId,
    model,
    modelsDir,
    onProgress,
    signal,
    mirror,
  }: DownloadModelInput): Promise<string> {
    const targetModelsDir = modelsDir ?? await this.ports.getModelsDir();
    const targetFilename = model.filename || `${modelId}.tar.bz2`;
    const tempFilePath = isCatalogModel(model)
      ? model.downloadPath
      : await this.ports.join(targetModelsDir, targetFilename);

    const expectedSha256 = model.sha256;
    await this.downloadFile(model.url, tempFilePath, onProgress, signal, 'Downloading', expectedSha256, mirror);

    if (model.isArchive === false) {
      onProgress?.(100, i18n.t('settings.model_download_status.done'), true);
      return tempFilePath;
    }

    if (signal?.aborted) {
      throw new Error('Download cancelled');
    }

    onProgress?.(100, i18n.t('settings.model_download_status.extracting'), false);

    let extractUnlisten: (() => void) | undefined;
    if (onProgress) {
      extractUnlisten = await this.ports.listen<string>(TauriEvent.app.extractProgress, (event) => {
        const filename = event.payload;
        const displayFilename = filename.length > 30 ? '...' + filename.slice(-27) : filename;
        onProgress(100, i18n.t('settings.model_download_status.extracting_file', {
          filename: displayFilename,
        }), false);
      });
    }

    try {
      logger.info('Starting extraction...');
      await this.extractArchive(tempFilePath, targetModelsDir, signal);
    } catch (error) {
      throw Object.assign(new Error(`Extraction failed: ${extractErrorMessage(error)}`), { cause: error });
    } finally {
      if (extractUnlisten) {
        extractUnlisten();
      }
    }

    await this.ports.remove(tempFilePath);

    onProgress?.(100, i18n.t('settings.model_download_status.done'), true);

    if (isCatalogModel(model)) {
      return model.installPath;
    }
    if (model.filename) {
      return await this.ports.join(targetModelsDir, model.filename);
    }
    if (model.type === 'punctuation') {
      return await this.ports.join(targetModelsDir, modelId);
    }
    if (model.type === 'vad') {
      return tempFilePath;
    }
    return await this.ports.join(targetModelsDir, modelId);
  }

  private async downloadFile(
    url: string,
    outputPath: string,
    onProgress?: ProgressCallback,
    signal?: AbortSignal,
    label: string = i18n.t('settings.model_download_status.download_label'),
    expectedSha256?: string,
    mirrorKey: string = 'direct',
  ): Promise<void> {
    const mirrorMap: Record<string, string> = {
      direct: '',
      ghproxy: 'https://mirror.ghproxy.com/',
      ghnet: 'https://ghproxy.net/',
    };

    const mirrorPrefix = mirrorMap[mirrorKey] ?? '';
    const downloadUrl = `${mirrorPrefix}${url}`;

    let lastError: unknown = null;
    let unlisten: (() => void) | undefined;
    let lastDownloaded = 0;
    let lastTime = Date.now();
    const downloadId = Math.random().toString(36).substring(7);

    if (signal) {
      signal.addEventListener('abort', async () => {
        try {
          await this.ports.cancelDownload(downloadId);
        } catch (error) {
          logger.error('Failed to cancel download:', error);
        }
      });
    }

    if (onProgress) {
      unlisten = await this.ports.listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
        const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);

        if (id && id !== downloadId) return;

        const now = Date.now();
        const timeDiff = now - lastTime;

        if (timeDiff > 500 || total === downloaded) {
          const bytesDiff = downloaded - lastDownloaded;
          const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
          const speedStr = speedBytesPerSec > 1024 * 1024
            ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
            : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

          lastDownloaded = downloaded;
          lastTime = now;

          if (total > 0) {
            const percentage = Math.round((downloaded / total) * 100);
            const downloadedMB = Math.round(downloaded / 1024 / 1024);
            const totalMB = Math.round(total / 1024 / 1024);
            onProgress(percentage, i18n.t('settings.model_download_status.downloading', {
              label,
              downloadedMB,
              totalMB,
              speed: speedStr,
            }));
          }
        }
      });
    }

    try {
      try {
        await retryWithBackoff({
          attempts: 3,
          abortError: () => new Error('Download cancelled'),
          run: async () => {
            if (onProgress) {
              onProgress(0, i18n.t(
                mirrorPrefix
                  ? 'settings.model_download_status.downloading_from_mirror'
                  : 'settings.model_download_status.downloading_only',
                { label },
              ));
            }

            logger.info(`Attempting download from: ${downloadUrl} with ID: ${downloadId}`);
            await this.ports.downloadFile({
              url: downloadUrl,
              outputPath,
              id: downloadId,
              ...(expectedSha256 ? { expectedSha256 } : {}),
            });
          },
          onFailedAttempt: (error, { attempt }) => {
            logger.warn(`Download attempt ${attempt} failed via ${mirrorPrefix || 'direct'}:`, error);
            lastError = error;
          },
          shouldRetry: (error) => {
            if (signal?.aborted || extractErrorMessage(error).includes('cancelled')) {
              throw Object.assign(new Error('Download cancelled'), { cause: error });
            }
            return true;
          },
          signal,
        });
      } catch (error) {
        if (extractErrorMessage(error) === 'Download cancelled') {
          throw error;
        }
        const cause = lastError ?? error;
        const lastErrorMessage = extractErrorMessage(cause);
        throw Object.assign(
          new Error(`Download failed after all attempts. Last error: ${lastErrorMessage}`),
          { cause },
        );
      }
    } finally {
      if (unlisten) {
        unlisten();
      }
    }
  }

  private async extractArchive(
    archivePath: string,
    targetDir: string,
    signal?: AbortSignal,
  ): Promise<void> {
    logger.info('[ModelService] Attempting extraction via Rust backend (extract_tar_bz2)...');

    if (signal) {
      signal.addEventListener('abort', () => {
        logger.warn('Extraction cancellation requested, but not supported via Rust backend yet.');
      });
    }

    try {
      await this.ports.extractTarBz2({
        archivePath,
        targetDir,
      });
    } catch (error) {
      throw Object.assign(new Error(`Extraction failed: ${extractErrorMessage(error)}`), { cause: error });
    }
  }
}

export function createModelDownloadService(ports: ModelDownloadServicePorts): ModelDownloadService {
  return new ModelDownloadService(ports);
}
