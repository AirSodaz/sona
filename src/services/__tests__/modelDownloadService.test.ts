import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createModelDownloadService } from '../modelDownloadService';
import { parseDownloadProgressPayload } from '../modelDownloadService';
import type { ModelCatalogModel, ModelInfo } from '../modelService';

const i18nMocks = vi.hoisted(() => ({
  t: vi.fn((key: string, params?: Record<string, unknown>) => {
    if (key === 'settings.model_download_status.done') return 'Done';
    if (key === 'settings.model_download_status.extracting') return 'Extracting';
    if (key === 'settings.model_download_status.extracting_file') return `Extracting ${params?.filename}`;
    if (key === 'settings.model_download_status.downloading_only') return `Downloading ${params?.label}`;
    if (key === 'settings.model_download_status.downloading_from_mirror') return `Mirror ${params?.label}`;
    if (key === 'settings.model_download_status.downloading') return `${params?.downloadedMB}/${params?.totalMB} ${params?.speed}`;
    if (key === 'settings.model_download_status.download_label') return 'Downloading';
    return key;
  }),
}));

vi.mock('../../i18n', () => ({
  default: i18nMocks,
}));

function makeModel(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'model-a',
    name: 'Model A',
    description: '',
    url: 'https://example.com/model-a.tar.bz2',
    type: 'sensevoice',
    modes: ['streaming'],
    language: 'en',
    size: '1 MB',
    isArchive: true,
    engine: 'sherpa-onnx',
    ...overrides,
  };
}

function makeCatalogModel(overrides: Partial<ModelCatalogModel> = {}): ModelCatalogModel {
  return {
    ...makeModel(),
    installPath: '/models/model-a',
    downloadPath: '/models/downloads/model-a.tar.bz2',
    isInstalled: false,
    rules: {
      requiresVad: false,
      requiresPunctuation: false,
    },
    ...overrides,
  };
}

describe('modelDownloadService', () => {
  const downloadFile = vi.fn();
  const extractTarBz2 = vi.fn();
  const cancelDownload = vi.fn();
  const remove = vi.fn();
  const listen = vi.fn();
  const join = vi.fn((...parts: string[]) => Promise.resolve(parts.join('/')));
  const getModelsDir = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    downloadFile.mockResolvedValue(undefined);
    extractTarBz2.mockResolvedValue(undefined);
    cancelDownload.mockResolvedValue(undefined);
    remove.mockResolvedValue(undefined);
    listen.mockResolvedValue(vi.fn());
    getModelsDir.mockResolvedValue('/models');
  });

  it('downloads and extracts catalog models using catalog paths', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    const onProgress = vi.fn();

    const result = await service.downloadModel({
      modelId: 'catalog-model',
      model: makeCatalogModel({
        id: 'catalog-model',
        installPath: '/catalog/install',
        downloadPath: '/catalog/download.tar.bz2',
      }),
      modelsDir: '/catalog',
      onProgress,
    });

    expect(result).toBe('/catalog/install');
    expect(downloadFile).toHaveBeenCalledWith(expect.objectContaining({
      url: 'https://example.com/model-a.tar.bz2',
      outputPath: '/catalog/download.tar.bz2',
    }));
    expect(extractTarBz2).toHaveBeenCalledWith({
      archivePath: '/catalog/download.tar.bz2',
      targetDir: '/catalog',
    });
    expect(remove).toHaveBeenCalledWith('/catalog/download.tar.bz2');
    expect(onProgress).toHaveBeenCalledWith(100, 'Done', true);
  });

  it('passes expected sha256 for archive model downloads before extraction', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });

    await service.downloadModel({
      modelId: 'model-a',
      model: makeModel({
        sha256: '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6',
      }),
    });

    expect(downloadFile).toHaveBeenCalledWith(expect.objectContaining({
      outputPath: '/models/model-a.tar.bz2',
      expectedSha256: '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6',
    }));
  });

  it('returns the downloaded file path for non-archive models', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    const model = makeModel({
      id: 'speaker.onnx',
      isArchive: false,
      filename: 'speaker.onnx',
      type: 'speaker-embedding',
      url: 'https://example.com/speaker.onnx',
    });

    await expect(service.downloadModel({
      modelId: 'speaker.onnx',
      model,
      onProgress: vi.fn(),
    })).resolves.toBe('/models/speaker.onnx');

    expect(extractTarBz2).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
  });

  it('passes expected sha256 to backend downloads for single-file models', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    const model = makeModel({
      id: 'speaker.onnx',
      isArchive: false,
      filename: 'speaker.onnx',
      type: 'speaker-embedding',
      url: 'https://example.com/speaker.onnx',
      sha256: '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6',
    });

    await service.downloadModel({
      modelId: 'speaker.onnx',
      model,
    });

    expect(downloadFile).toHaveBeenCalledWith(expect.objectContaining({
      outputPath: '/models/speaker.onnx',
      expectedSha256: '9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6',
    }));
  });

  it('retries the same mirror up to 3 times on failure', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    downloadFile
      .mockRejectedValueOnce(new Error('attempt 1 failed'))
      .mockRejectedValueOnce(new Error('attempt 2 failed'))
      .mockRejectedValueOnce(new Error('attempt 3 failed'));

    await expect(service.downloadModel({
      modelId: 'model-a',
      model: makeModel(),
      mirror: 'ghproxy',
    })).rejects.toThrow('attempt 3 failed');

    expect(downloadFile).toHaveBeenCalledTimes(3);
    expect(downloadFile).toHaveBeenNthCalledWith(1, expect.objectContaining({
      url: 'https://mirror.ghproxy.com/https://example.com/model-a.tar.bz2',
    }));
    expect(downloadFile).toHaveBeenNthCalledWith(2, expect.objectContaining({
      url: 'https://mirror.ghproxy.com/https://example.com/model-a.tar.bz2',
    }));
    expect(downloadFile).toHaveBeenNthCalledWith(3, expect.objectContaining({
      url: 'https://mirror.ghproxy.com/https://example.com/model-a.tar.bz2',
    }));
  });

  it('stops retrying once an attempt succeeds', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    downloadFile
      .mockRejectedValueOnce(new Error('attempt 1 failed'))
      .mockResolvedValueOnce(undefined);

    await expect(service.downloadModel({
      modelId: 'model-a',
      model: makeModel(),
      mirror: 'ghnet',
    })).resolves.toBe('/models/model-a');

    expect(downloadFile).toHaveBeenCalledTimes(2);
    expect(downloadFile).toHaveBeenNthCalledWith(2, expect.objectContaining({
      url: 'https://ghproxy.net/https://example.com/model-a.tar.bz2',
    }));
  });

  it('cancels the active backend download when the abort signal fires', async () => {
    const service = createModelDownloadService({
      downloadFile,
      extractTarBz2,
      cancelDownload,
      remove,
      listen,
      join,
      getModelsDir,
    });
    const controller = new AbortController();

    downloadFile.mockImplementation(async () => {
      controller.abort();
      throw new Error('cancelled by backend');
    });

    await expect(service.downloadModel({
      modelId: 'model-a',
      model: makeModel(),
      signal: controller.signal,
    })).rejects.toThrow('Download cancelled');

    expect(cancelDownload).toHaveBeenCalledTimes(1);
  });

  it('parses legacy and named download progress payloads', () => {
    expect(parseDownloadProgressPayload([10, 20, 'download-a'])).toEqual({
      downloaded: 10,
      total: 20,
      id: 'download-a',
    });
    expect(parseDownloadProgressPayload({ 0: 1, 1: 2, 2: 'download-b' })).toEqual({
      downloaded: 1,
      total: 2,
      id: 'download-b',
    });
    expect(parseDownloadProgressPayload({ downloaded: 3, total: 4, id: 'download-c' })).toEqual({
      downloaded: 3,
      total: 4,
      id: 'download-c',
    });
  });
});
