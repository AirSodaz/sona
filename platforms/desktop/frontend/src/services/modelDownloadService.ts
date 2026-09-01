import i18n from '../i18n';
import { logger } from '../utils/logger';
import { extractErrorMessage } from '../utils/errorUtils';
import { TauriEvent } from './tauri/events';
import type { ModelCatalogModel, ModelInfo, ProgressCallback } from '../types/modelCatalog';
import { downloadCandidates, modelscopeMirrorUrl } from '../utils/mirrorCandidates';

interface DownloadProgressPayloadObject {
  0?: number;
  1?: number;
  2?: string;
  downloaded?: number;
  total?: number;
  id?: string;
}

type DownloadFile = (input: { url: string; outputPath: string; id: string; expectedSha256?: string }) => Promise<void>;
type DownloadPresetModel = (input: { modelId: string; downloadId: string; mirror?: string }) => Promise<string>;
type ExtractTarBz2 = (input: { archivePath: string; targetDir: string }) => Promise<void>;
type Listen = <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>;

interface ModelDownloadServicePorts {
  downloadFile: DownloadFile;
  downloadPresetModel?: DownloadPresetModel;
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
    if (this.ports.downloadPresetModel) {
      return await this.downloadPresetModel(modelId, onProgress, signal, mirror);
    }
    const targetModelsDir = modelsDir ?? await this.ports.getModelsDir();
    const artifacts = model.artifacts ?? [];
    if (artifacts.length > 1) {
      return await this.downloadPresetModel(modelId, onProgress, signal, mirror);
    }
    const primaryArtifact = artifacts[0];
    if (!primaryArtifact) {
      throw new Error(`Model '${modelId}' has no download artifacts`);
    }
    const targetFilename = model.filename || `${modelId}.tar.bz2`;
    const tempFilePath = isCatalogModel(model)
      ? model.downloadPath
      : await this.ports.join(targetModelsDir, targetFilename);

    const expectedSha256 = primaryArtifact.sha256;
    const alternateUrl = modelscopeMirrorUrl(model.id, primaryArtifact.filename);
    const candidates = downloadCandidates(primaryArtifact.url, mirror ?? 'auto', alternateUrl);

    let lastError: unknown = null;
    for (const [candidateIndex, candidateUrl] of candidates.entries()) {
      if (candidateIndex > 0) {
        // Never resume a partial file across different sources; the Rust
        // downloader keeps the in-flight file at `<outputPath>.download`
        // (see `temporary_download_path`).
        await this.ports.remove(`${tempFilePath}.download`).catch(() => undefined);
      }
      try {
        await this.downloadFile(candidateUrl, tempFilePath, onProgress, signal, 'Downloading', expectedSha256, candidateIndex > 0);
        lastError = null;
        break;
      } catch (error) {
        if (signal?.aborted || extractErrorMessage(error).includes('cancelled')) {
          throw Object.assign(new Error('Download cancelled'), { cause: error });
        }
        lastError = error;
      }
    }
    if (lastError !== null) {
      throw lastError;
    }

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

  private async downloadPresetModel(
    modelId: string,
    onProgress?: ProgressCallback,
    signal?: AbortSignal,
    mirror?: string,
  ): Promise<string> {
    if (!this.ports.downloadPresetModel) {
      throw new Error('Preset model downloads are unavailable in this host');
    }

    const downloadId = Math.random().toString(36).substring(7);
    const abort = async () => {
      try {
        await this.ports.cancelDownload(downloadId);
      } catch (error) {
        logger.error('Failed to cancel multi-file model download:', error);
      }
    };
    signal?.addEventListener('abort', abort, { once: true });

    let lastDownloaded = 0;
    let uiLastDownloaded = 0;
    let lastTime = Date.now();
    const unlisten = await this.ports.listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
      const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);
      if (id !== downloadId || total <= 0) return;

      if (downloaded < lastDownloaded) {
        uiLastDownloaded = 0;
      }
      lastDownloaded = downloaded;

      const now = Date.now();
      const timeDiff = now - lastTime;
      if (onProgress && timeDiff > 500) {
        const bytesDiff = Math.max(0, downloaded - uiLastDownloaded);
        const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
        const speedStr = speedBytesPerSec > 1024 * 1024
          ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
          : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

        uiLastDownloaded = downloaded;
        lastTime = now;
        onProgress(Math.round((downloaded / total) * 100), i18n.t('settings.model_download_status.downloading', {
          label: 'Downloading',
          downloadedMB: Math.round(downloaded / 1024 / 1024),
          totalMB: Math.round(total / 1024 / 1024),
          speed: speedStr,
        }));
      }
    });

    try {
      onProgress?.(0, i18n.t('settings.model_download_status.downloading_only', {
        label: 'Downloading',
      }));
      const path = await this.ports.downloadPresetModel({ modelId, downloadId, mirror });
      onProgress?.(100, i18n.t('settings.model_download_status.done'), true);
      return path;
    } catch (error) {
      if (signal?.aborted || extractErrorMessage(error).includes('cancelled')) {
        throw Object.assign(new Error('Download cancelled'), { cause: error });
      }
      throw error;
    } finally {
      signal?.removeEventListener('abort', abort);
      unlisten();
    }
  }

  private async downloadFile(
    url: string,
    outputPath: string,
    onProgress?: ProgressCallback,
    signal?: AbortSignal,
    label: string = i18n.t('settings.model_download_status.download_label'),
    expectedSha256?: string,
    fromMirror: boolean = false,
  ): Promise<void> {
    let lastError: unknown = null;
    let lastDownloaded = 0;
    let uiLastDownloaded = 0;
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

    const unlisten = await this.ports.listen<unknown>(TauriEvent.app.downloadProgress, (event) => {
      const { downloaded, total, id } = parseDownloadProgressPayload(event.payload);

      if (id && id !== downloadId) return;

      if (downloaded < lastDownloaded) {
        uiLastDownloaded = 0;
      }
      lastDownloaded = downloaded;

      const now = Date.now();
      const timeDiff = now - lastTime;

      if (onProgress && (timeDiff > 500 || total === downloaded)) {
        const bytesDiff = Math.max(0, downloaded - uiLastDownloaded);
        const speedBytesPerSec = bytesDiff / (timeDiff / 1000);
        const speedStr = speedBytesPerSec > 1024 * 1024
          ? `${(speedBytesPerSec / 1024 / 1024).toFixed(1)} MB/s`
          : `${Math.round(speedBytesPerSec / 1024)} KB/s`;

        uiLastDownloaded = downloaded;
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

    try {
      let consecutiveFailures = 0;
      const maxConsecutiveFailures = 3;
      let attempt = 0;

      while (consecutiveFailures < maxConsecutiveFailures) {
        attempt++;
        const downloadedAtStartOfAttempt = lastDownloaded;

        try {
          if (onProgress) {
            onProgress(0, i18n.t(
              fromMirror
                ? 'settings.model_download_status.downloading_from_mirror'
                : 'settings.model_download_status.downloading_only',
              { label },
            ));
          }

          logger.info(`Attempting download from: ${url} with ID: ${downloadId}`);
          await this.ports.downloadFile({
            url,
            outputPath,
            id: downloadId,
            ...(expectedSha256 ? { expectedSha256 } : {}),
          });

          // Success, exit the loop
          break;
        } catch (error) {
          if (signal?.aborted || extractErrorMessage(error).includes('cancelled')) {
            throw Object.assign(new Error('Download cancelled'), { cause: error });
          }

          if (lastDownloaded > downloadedAtStartOfAttempt) {
            // We made progress! Reset consecutive failures to 1 (this was a failed attempt but fruitful)
            consecutiveFailures = 1;
            logger.warn(`Download attempt ${attempt} failed via ${fromMirror ? 'mirror' : 'direct'}, but progress was made. Resetting consecutive failures.`, error);
          } else {
            consecutiveFailures++;
            logger.warn(`Download attempt ${attempt} failed via ${fromMirror ? 'mirror' : 'direct'}. Consecutive failures: ${consecutiveFailures}`, error);
          }

          lastError = error;

          if (consecutiveFailures >= maxConsecutiveFailures) {
            // Re-throw to be caught by the outer catch
            throw error;
          }

          // Small delay before retrying might be beneficial, but keeping it simple for now as per previous logic
        }
      }
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
