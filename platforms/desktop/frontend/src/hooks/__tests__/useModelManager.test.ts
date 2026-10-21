import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { modelService } from '../../services/modelService';
import { cleanInterruptedDownload } from '../../services/tauri/app';
import { useConfigStore } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import { useModelDownloadStore } from '../../stores/modelDownloadStore';
import { setTestConfig } from '../../test-utils/configTestUtils';
import { useModelManager } from '../useModelManager';

const SENSEVOICE_INT8_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const SENSEVOICE_FP32_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17';
const SILERO_VAD_ID = 'silero-vad';
const catalogModel = {
  id: 'preset-a',
  name: 'Preset A',
  description: 'settings.descriptions.preset_a',
  url: 'https://example.com/preset-a.tar.bz2',
  type: 'sensevoice',
  modes: ['streaming', 'batch'],
  languages: ['en', 'zh'],
  languageMode: 'selectable',
  size: '1 MB',
  engine: 'sherpa-onnx',
  rules: { requiresVad: false, requiresPunctuation: false },
  installPath: '/models/preset-a',
  downloadPath: '/models/preset-a.tar.bz2',
  isInstalled: true,
} as any;
const modelCatalogSnapshot = {
  modelsDir: '/models',
  models: [catalogModel],
  sections: [
    {
      type: 'asr',
      groups: [
        {
          key: 'preset-a',
          models: [catalogModel],
        },
      ],
    },
  ],
  selectionOptions: {
    streaming: [
      {
        id: 'preset-a',
        label: 'Preset A',
        installPath: '/models/preset-a',
        isInstalled: true,
      },
    ],
    batch: [
      {
        id: 'preset-a',
        label: 'Preset A',
        installPath: '/models/preset-a',
        isInstalled: true,
      },
    ],
    speakerSegmentation: [],
    speakerEmbedding: [],
  },
  modelPathById: {
    'preset-a': '/models/preset-a',
  },
  modelIdByNormalizedPath: {
    '/models/preset-a': 'preset-a',
  },
  pathMatchTokens: [{ id: 'preset-a', token: 'preset-a' }],
  dependencyRequestsByModelId: {},
  restoreDefaults: {
    punctuationModelPath: '',
    speakerSegmentationModelPath: '',
    speakerEmbeddingModelPath: '',
    enableITN: true,
    vadBufferSize: 5,
    maxConcurrent: 2,
  },
} as any;

function buildInstalledCatalogSnapshot(installedIds: string[]): any {
  const models = [SENSEVOICE_INT8_ID, SENSEVOICE_FP32_ID, SILERO_VAD_ID].map((id) => ({
    ...catalogModel,
    id,
    installPath: `/models/${id}`,
    downloadPath: `/models/${id}.tar.bz2`,
    isInstalled: installedIds.includes(id),
  }));

  return {
    modelsDir: '/models',
    models,
    sections: [
      {
        type: 'asr',
        groups: models.map((model) => ({ key: model.id, models: [model] })),
      },
    ],
    selectionOptions: {
      streaming: models.map((model) => ({
        id: model.id,
        label: model.name,
        installPath: model.installPath,
        isInstalled: model.isInstalled,
      })),
      batch: models.map((model) => ({
        id: model.id,
        label: model.name,
        installPath: model.installPath,
        isInstalled: model.isInstalled,
      })),
      speakerSegmentation: [],
      speakerEmbedding: [],
    },
    modelPathById: Object.fromEntries(models.map((model) => [model.id, model.installPath])),
    modelIdByNormalizedPath: Object.fromEntries(
      models.map((model) => [model.installPath.toLowerCase(), model.id])
    ),
    pathMatchTokens: models.map((model) => ({ id: model.id, token: model.id.toLowerCase() })),
    dependencyRequestsByModelId: {},
    restoreDefaults: {
      streamingModelPath: installedIds.includes(SENSEVOICE_INT8_ID)
        ? `/models/${SENSEVOICE_INT8_ID}`
        : installedIds.includes(SENSEVOICE_FP32_ID)
          ? `/models/${SENSEVOICE_FP32_ID}`
          : undefined,
      batchModelPath: installedIds.includes(SENSEVOICE_INT8_ID)
        ? `/models/${SENSEVOICE_INT8_ID}`
        : installedIds.includes(SENSEVOICE_FP32_ID)
          ? `/models/${SENSEVOICE_FP32_ID}`
          : undefined,
      vadModelPath: installedIds.includes(SILERO_VAD_ID) ? `/models/${SILERO_VAD_ID}` : undefined,
      punctuationModelPath: '',
      speakerSegmentationModelPath: '',
      speakerEmbeddingModelPath: '',
      enableITN: true,
      vadBufferSize: 5,
      maxConcurrent: 2,
    },
  };
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../services/tauri/app', () => ({
  cleanInterruptedDownload: vi.fn().mockResolvedValue(true),
  checkInterruptedDownload: vi.fn().mockResolvedValue(null),
  cancelDownload: vi.fn().mockResolvedValue(true),
}));

vi.mock('../../services/modelService', () => ({
  PRESET_MODELS: [{ id: 'preset-a', name: 'Preset A', type: 'sensevoice' }],
  PRESET_MODELS_MAP: {},
  modelService: {
    isModelInstalled: vi.fn(),
    getModelPath: vi.fn(),
    downloadModel: vi.fn(),
    checkHardware: vi.fn(),
    deleteModel: vi.fn(),
    getModelRules: vi.fn(),
    getModelCatalogSnapshot: vi.fn(),
    resolveModelCatalogSelectedIds: vi.fn(),
    resolveAsrSelectedModelIdsFromSnapshot: vi.fn(),
    resolveScenarioSelectedModelIdsFromSnapshot: vi.fn(),
  },
}));

function createScenarioIdResolver() {
  return (snapshot: any, config: Record<string, string | undefined>) => {
    const sectionOptionsByType: Record<string, Array<{ id: string }>> = {};
    for (const section of snapshot.sections) {
      sectionOptionsByType[section.type] = section.groups.flatMap((group: any) => group.models);
    }

    const resolve = (path: string | undefined, options: Array<{ id: string }>) => {
      if (!path?.trim()) return null;
      const normalizedPath = path.replace(/\\/g, '/').toLowerCase();
      const exact = snapshot.modelIdByNormalizedPath[normalizedPath];
      if (exact && options.some((option) => option.id === exact)) return exact;
      const tokenMatch = options.find((option) => {
        const token = snapshot.pathMatchTokens.find((item: any) => item.id === option.id);
        return token?.token && normalizedPath.includes(token.token);
      });
      return tokenMatch?.id ?? null;
    };

    return {
      liveSpeakerSegmentation: resolve(
        config.liveSpeakerSegmentationModelPath,
        snapshot.selectionOptions.speakerSegmentation
      ),
      batchSpeakerSegmentation: resolve(
        config.batchSpeakerSegmentationModelPath,
        snapshot.selectionOptions.speakerSegmentation
      ),
      liveSpeakerEmbedding: resolve(
        config.liveSpeakerEmbeddingModelPath,
        snapshot.selectionOptions.speakerEmbedding
      ),
      batchSpeakerEmbedding: resolve(
        config.batchSpeakerEmbeddingModelPath,
        snapshot.selectionOptions.speakerEmbedding
      ),
      livePunctuation: resolve(
        config.livePunctuationModelPath,
        sectionOptionsByType.punctuation ?? []
      ),
      batchPunctuation: resolve(
        config.batchPunctuationModelPath,
        sectionOptionsByType.punctuation ?? []
      ),
      liveVad: resolve(config.liveVadModelPath, sectionOptionsByType.vad ?? []),
      batchVad: resolve(config.batchVadModelPath, sectionOptionsByType.vad ?? []),
      liveAlignment: resolve(config.liveAlignmentModelPath, sectionOptionsByType.alignment ?? []),
      batchAlignment: resolve(config.batchAlignmentModelPath, sectionOptionsByType.alignment ?? []),
    };
  };
}

describe('useModelManager restoreDefaultModelSettings', () => {
  beforeEach(() => {
    vi.mocked(modelService.isModelInstalled).mockReset();
    vi.mocked(modelService.getModelPath).mockReset();
    vi.mocked(modelService.downloadModel).mockReset();
    vi.mocked(modelService.checkHardware).mockReset();
    vi.mocked(modelService.deleteModel).mockReset();
    vi.mocked(modelService.getModelRules).mockReset();
    vi.mocked(modelService.getModelCatalogSnapshot).mockReset();
    vi.mocked(modelService.resolveModelCatalogSelectedIds).mockReset();
    vi.mocked(modelService.resolveAsrSelectedModelIdsFromSnapshot).mockReset();
    vi.mocked(modelService.resolveScenarioSelectedModelIdsFromSnapshot).mockReset();

    setTestConfig({
      streamingModelPath: '/current/live',
      batchModelPath: '/current/batch',
      liveVadModelPath: '/current/vad',
      livePunctuationModelPath: '/current/punctuation',
      liveSpeakerSegmentationModelPath: '/current/speaker-segmentation',
      liveSpeakerEmbeddingModelPath: '/current/speaker-embedding',
      enableITN: false,
      batchVadEnabled: false,
      liveVadBufferSize: 9,
      maxConcurrent: 4,
      asr: {
        providers: {
          online: {
            'volcengine-doubao': {
              apiKey: 'volc-test-key',
              streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
              streamingResourceId: 'volc.seedasr.sauc.duration',
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit',
              batchResourceId: 'volc.seedasr.auc',
            },
          },
        },
      },
    });

    useDialogStore.setState({
      isOpen: false,
      options: null,
      resolveRef: null,
      confirm: vi.fn().mockResolvedValue(true),
      showError: vi.fn().mockResolvedValue(undefined),
    });

    vi.mocked(modelService.getModelPath).mockImplementation(async (id: string) => `/models/${id}`);
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(modelCatalogSnapshot);
    vi.mocked(modelService.resolveModelCatalogSelectedIds).mockResolvedValue({
      streaming: null,
      batch: null,
      speakerSegmentation: null,
      speakerEmbedding: null,
    });
    vi.mocked(modelService.resolveAsrSelectedModelIdsFromSnapshot).mockImplementation(
      (snapshot, paths) => {
        const resolve = (path: string, options: Array<{ id: string }>) => {
          if (!path?.trim()) return null;
          const normalizedPath = path.replace(/\\/g, '/').toLowerCase();
          const exact = snapshot.modelIdByNormalizedPath[normalizedPath];
          if (exact && options.some((option) => option.id === exact)) return exact;
          const tokenMatch = options.find((option) => {
            const token = snapshot.pathMatchTokens.find((item: any) => item.id === option.id);
            return token?.token && normalizedPath.includes(token.token);
          });
          return tokenMatch?.id ?? null;
        };

        return {
          streaming: resolve(paths.streamingModelPath, snapshot.selectionOptions.streaming),
          batch: resolve(paths.batchModelPath, snapshot.selectionOptions.batch),
        };
      }
    );
    vi.mocked(modelService.resolveScenarioSelectedModelIdsFromSnapshot).mockImplementation(
      createScenarioIdResolver() as any
    );
  });

  it('does not load the model catalog while inactive for settings tab prewarm', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

    renderHook(() => useModelManager(false));

    await act(async () => {
      await Promise.resolve();
    });

    expect(modelService.getModelCatalogSnapshot).not.toHaveBeenCalled();
    expect(modelService.resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
    expect(modelService.isModelInstalled).not.toHaveBeenCalled();
  });

  it('loads the model catalog snapshot when the model pane becomes active', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

    const { result, rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
      initialProps: { isOpen: false },
    });

    rerender({ isOpen: true });

    await waitFor(() => {
      expect(modelService.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
      expect(result.current.installedModels.has('preset-a')).toBe(true);
    });
    await waitFor(() => {
      expect(result.current.catalogLoadState).toBe('ready');
    });
    expect(modelService.resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
    expect(modelService.isModelInstalled).not.toHaveBeenCalled();
  });

  it('defers model catalog loading until after the active frame', async () => {
    vi.mocked(modelService.isModelInstalled).mockResolvedValue(false);

    const { rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
      initialProps: { isOpen: false },
    });

    rerender({ isOpen: true });

    expect(modelService.getModelCatalogSnapshot).not.toHaveBeenCalled();
    expect(modelService.resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
    expect(modelService.isModelInstalled).not.toHaveBeenCalled();

    await waitFor(() => {
      expect(modelService.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
      expect(modelService.resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
    });
    expect(modelService.isModelInstalled).not.toHaveBeenCalled();
  });

  it('does not apply a stale catalog snapshot after the model pane is deactivated', async () => {
    let resolveSnapshot: (snapshot: typeof modelCatalogSnapshot) => void = () => undefined;
    vi.mocked(modelService.getModelCatalogSnapshot).mockReturnValue(
      new Promise((resolve) => {
        resolveSnapshot = resolve;
      })
    );

    const { result, rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
      initialProps: { isOpen: false },
    });

    rerender({ isOpen: true });

    await waitFor(() => {
      expect(result.current.catalogLoadState).toBe('loading');
    });

    rerender({ isOpen: false });

    await act(async () => {
      resolveSnapshot(modelCatalogSnapshot);
      await Promise.resolve();
    });

    expect(result.current.catalogLoadState).toBe('idle');
    expect(result.current.installedModels.has('preset-a')).toBe(false);
  });

  it('resolves selected model ids from the loaded snapshot without backend fallback', async () => {
    setTestConfig({
      streamingModelPath: '/models/preset-a',
      batchModelPath: 'D:\\portable\\preset-a',
      liveSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
    });

    const { result, rerender } = renderHook(({ isOpen }) => useModelManager(isOpen), {
      initialProps: { isOpen: false },
    });

    expect(result.current.catalogLoadState).toBe('idle');

    rerender({ isOpen: true });

    await waitFor(() => {
      expect(result.current.catalogLoadState).toBe('ready');
      expect(result.current.selectedModelIds).toMatchObject({
        streaming: 'preset-a',
        batch: 'preset-a',
      });
    });

    expect(modelService.getModelCatalogSnapshot).toHaveBeenCalledTimes(1);
    expect(modelService.resolveModelCatalogSelectedIds).not.toHaveBeenCalled();
  });

  it('restores SenseVoice Int8 and Silero VAD when both are installed', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([SENSEVOICE_INT8_ID, SILERO_VAD_ID])
    );

    const { result } = renderHook(() => useModelManager(false));

    await act(async () => {
      await result.current.restoreDefaultModelSettings();
    });

    expect(useConfigStore.getState().config).toMatchObject({
      streamingModelPath: `/models/${SENSEVOICE_INT8_ID}`,
      batchModelPath: `/models/${SENSEVOICE_INT8_ID}`,
      liveVadModelPath: `/models/${SILERO_VAD_ID}`,
      batchVadModelPath: `/models/${SILERO_VAD_ID}`,
      livePunctuationModelPath: '',
      batchPunctuationModelPath: '',
      liveSpeakerSegmentationModelPath: '',
      batchSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
      batchSpeakerEmbeddingModelPath: '',
      enableITN: true,
      batchVadEnabled: true,
      liveVadBufferSize: 5,
      batchVadBufferSize: 5,
      maxConcurrent: 2,
      asr: {
        providers: {
          online: {
            'volcengine-doubao': {
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
              batchResourceId: 'volc.bigasr.auc_turbo',
            },
          },
        },
      },
    });
  });

  it('falls back to SenseVoice Fp32 when Int8 is unavailable', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([SENSEVOICE_FP32_ID])
    );

    const { result } = renderHook(() => useModelManager(false));

    await act(async () => {
      await result.current.restoreDefaultModelSettings();
    });

    expect(useConfigStore.getState().config).toMatchObject({
      streamingModelPath: `/models/${SENSEVOICE_FP32_ID}`,
      batchModelPath: `/models/${SENSEVOICE_FP32_ID}`,
      liveVadModelPath: '',
      batchVadModelPath: '',
      livePunctuationModelPath: '',
      batchPunctuationModelPath: '',
      liveSpeakerSegmentationModelPath: '',
      batchSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
      batchSpeakerEmbeddingModelPath: '',
      enableITN: true,
      batchVadEnabled: true,
      liveVadBufferSize: 5,
      batchVadBufferSize: 5,
      maxConcurrent: 2,
    });
  });

  it('keeps the current ASR models and clears scenario VAD paths when no defaults resolve', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );

    const { result } = renderHook(() => useModelManager(false));

    await act(async () => {
      await result.current.restoreDefaultModelSettings();
    });

    expect(useConfigStore.getState().config).toMatchObject({
      streamingModelPath: '/current/live',
      batchModelPath: '/current/batch',
      liveVadModelPath: '',
      batchVadModelPath: '',
      livePunctuationModelPath: '',
      batchPunctuationModelPath: '',
      liveSpeakerSegmentationModelPath: '',
      batchSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
      batchSpeakerEmbeddingModelPath: '',
      enableITN: true,
      batchVadEnabled: true,
      liveVadBufferSize: 5,
      batchVadBufferSize: 5,
      maxConcurrent: 2,
    });
  });

  it('clears scenario VAD paths to the catalog default when Silero VAD is not installed', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([SENSEVOICE_INT8_ID])
    );

    const { result } = renderHook(() => useModelManager(false));

    await act(async () => {
      await result.current.restoreDefaultModelSettings();
    });

    expect(useConfigStore.getState().config).toMatchObject({
      streamingModelPath: `/models/${SENSEVOICE_INT8_ID}`,
      batchModelPath: `/models/${SENSEVOICE_INT8_ID}`,
      liveVadModelPath: '',
      batchVadModelPath: '',
      livePunctuationModelPath: '',
      batchPunctuationModelPath: '',
      liveSpeakerSegmentationModelPath: '',
      batchSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
      batchSpeakerEmbeddingModelPath: '',
      enableITN: true,
      batchVadEnabled: true,
      liveVadBufferSize: 5,
      batchVadBufferSize: 5,
      maxConcurrent: 2,
    });
  });
});

describe('useModelManager download lifecycle and errors (H1-H8)', () => {
  beforeEach(() => {
    useModelDownloadStore.setState({ downloads: {} });
    vi.mocked(modelService.downloadModel).mockReset();
    vi.mocked(modelService.checkHardware).mockReset();
    vi.mocked(modelService.getModelCatalogSnapshot).mockReset();
    useDialogStore.setState({
      alert: vi.fn(),
      confirm: vi.fn(),
      checkboxConfirm: vi.fn(),
      showError: vi.fn(),
    });
  });

  it('passes unified downloadId to downloadModel and registers onCancel (H1)', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });
    let passedDownloadId = '';
    let passedSignal: AbortSignal | undefined;
    const { promise, resolve: resolveDownload } = Promise.withResolvers<string>();
    vi.mocked(modelService.downloadModel).mockImplementation(
      async (_id, _cb, sig, _mirror, downloadId) => {
        passedDownloadId = downloadId ?? '';
        passedSignal = sig;
        return promise;
      }
    );

    const { result } = renderHook(() => useModelManager(true));

    let downloadPromise: Promise<void> | undefined;
    await act(async () => {
      downloadPromise = result.current.handleDownload(catalogModel);
    });

    expect(passedDownloadId).toBeTruthy();
    expect(passedDownloadId).toContain(catalogModel.id);
    expect(modelService.downloadModel).toHaveBeenCalledTimes(1);

    const item = useModelDownloadStore.getState().downloads[catalogModel.id];
    expect(item).toBeDefined();
    expect(item?.onCancel).toBeDefined();
    expect(passedSignal?.aborted).toBe(false);
    item?.onCancel?.();
    expect(passedSignal?.aborted).toBe(true);

    await act(async () => {
      resolveDownload('/models/preset-a');
      await downloadPromise;
    });
  });

  it('cancels active background download via store when local activeDownloadsRef is empty', async () => {
    const onCancel = vi.fn();
    useModelDownloadStore.getState().startDownload({
      modelId: catalogModel.id,
      modelName: catalogModel.name,
      downloadId: 'dl-bg-123',
      onCancel,
    });
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );

    const { result } = renderHook(() => useModelManager(true));

    await act(async () => {
      result.current.handleCancelDownload(catalogModel.id);
    });

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(useModelDownloadStore.getState().downloads[catalogModel.id]?.status).toBe('cancelled');
  });

  it('guards against concurrent download triggers before running prompts (H6)', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });
    let resolveDownload: (path: string) => void = () => {};
    vi.mocked(modelService.downloadModel).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveDownload = resolve;
        })
    );

    const { result } = renderHook(() => useModelManager(true));

    // First download starts
    act(() => {
      void result.current.handleDownload(catalogModel);
    });

    // Check hardware was called once
    expect(modelService.checkHardware).toHaveBeenCalledTimes(1);

    // Second download triggered concurrently for same model
    await act(async () => {
      await result.current.handleDownload(catalogModel);
    });

    // In-flight guard must prevent second download and prompt from running
    expect(modelService.checkHardware).toHaveBeenCalledTimes(1);
    expect(modelService.downloadModel).toHaveBeenCalledTimes(1);

    // Finish first download
    await act(async () => {
      resolveDownload('/models/preset-a');
    });
  });

  it('halts and shows error when cleanInterruptedDownload rejects in handleDiscardDownload (H3)', async () => {
    const { cleanInterruptedDownload } = await import('../../services/tauri/app');
    vi.mocked(cleanInterruptedDownload).mockRejectedValue(new Error('File is locked'));

    const showErrorMock = vi.fn();
    const confirmMock = vi.fn().mockResolvedValue(true);
    useDialogStore.setState({
      showError: showErrorMock,
      confirm: confirmMock,
    });

    const interruptedModel = {
      ...catalogModel,
      isInstalled: false,
      interruptedDownload: {
        modelId: catalogModel.id,
        downloadedBytes: 500,
        totalBytes: 1000,
        percentage: 50,
      },
    };

    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue({
      ...modelCatalogSnapshot,
      models: [interruptedModel],
      sections: [{ type: 'asr', groups: [{ key: 'preset-a', models: [interruptedModel] }] }],
    });

    const { result } = renderHook(() => useModelManager(true));

    await act(async () => {
      await result.current.handleDiscardDownload(interruptedModel);
    });

    expect(confirmMock).toHaveBeenCalled();
    expect(cleanInterruptedDownload).toHaveBeenCalledWith(catalogModel.id);
    expect(showErrorMock).toHaveBeenCalledTimes(1);
  });

  it('resumes download directly without prompt when model has interruptedDownload', async () => {
    const { cleanInterruptedDownload } = await import('../../services/tauri/app');
    const interruptedModel = {
      ...catalogModel,
      isInstalled: false,
      interruptedDownload: {
        modelId: catalogModel.id,
        downloadedBytes: 500,
        totalBytes: 1000,
        percentage: 50,
      },
    };

    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue({
      ...modelCatalogSnapshot,
      models: [interruptedModel],
      sections: [{ type: 'asr', groups: [{ key: 'preset-a', models: [interruptedModel] }] }],
    });
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });

    const { result } = renderHook(() => useModelManager(true));

    await act(async () => {
      await result.current.handleDownload(interruptedModel);
    });

    expect(cleanInterruptedDownload).not.toHaveBeenCalled();
    expect(modelService.downloadModel).toHaveBeenCalledWith(
      catalogModel.id,
      expect.any(Function),
      expect.any(AbortSignal),
      expect.anything(),
      expect.any(String)
    );
  });

  it('does not flip download to failed when post-download snapshot refresh throws (H8)', async () => {
    let callCount = 0;
    vi.mocked(modelService.getModelCatalogSnapshot).mockImplementation(async () => {
      callCount++;
      if (callCount > 1) {
        throw new Error('Post-download refresh network error');
      }
      return buildInstalledCatalogSnapshot([]);
    });
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });
    vi.mocked(modelService.downloadModel).mockResolvedValue('/models/preset-a');

    const { useModelDownloadStore } = await import('../../stores/modelDownloadStore');

    const { result } = renderHook(() => useModelManager(true));

    await act(async () => {
      await result.current.handleDownload(catalogModel);
    });

    // The download succeeded, so store item must be 'completed', NOT 'failed'
    const item = useModelDownloadStore.getState().downloads[catalogModel.id];
    expect(item).toBeDefined();
    expect(item.status).toBe('completed');
  });

  it('refreshes catalog snapshot when download is cancelled (H4)', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });

    let abortControllerSignal: AbortSignal | undefined;
    const downloadStarted = new Promise<void>((resolve) => {
      vi.mocked(modelService.downloadModel).mockImplementation(async (_id, _cb, sig) => {
        abortControllerSignal = sig;
        resolve();
        return new Promise((_, reject) => {
          sig?.addEventListener('abort', () => reject(new Error('Download cancelled')));
        });
      });
    });

    const { result } = renderHook(() => useModelManager(true));

    await waitFor(() => {
      expect(modelService.getModelCatalogSnapshot).toHaveBeenCalled();
    });
    vi.mocked(modelService.getModelCatalogSnapshot).mockClear();

    void result.current.handleDownload(catalogModel);
    await downloadStarted;

    // Cancel download
    await act(async () => {
      result.current.handleCancelDownload(catalogModel.id);
    });

    expect(abortControllerSignal?.aborted).toBe(true);
    // Snapshot must be refreshed after cancellation so resume chip appears
    await waitFor(() => {
      expect(modelService.getModelCatalogSnapshot).toHaveBeenCalled();
    });
  });

  it('synchronizes store downloading status into hook downloads return value', async () => {
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );
    useModelDownloadStore.getState().startDownload({
      modelId: catalogModel.id,
      modelName: catalogModel.name,
      downloadId: 'dl-store-sync',
    });
    useModelDownloadStore.getState().updateProgress('dl-store-sync', { progress: 45 });

    const { result } = renderHook(() => useModelManager(true));

    expect(result.current.downloads[catalogModel.id]).toBeDefined();
    expect(result.current.downloads[catalogModel.id]?.progress).toBe(45);
  });

  it('prevents handleDiscardDownload while download cancellation is actively unwinding', async () => {
    vi.mocked(cleanInterruptedDownload).mockClear();
    vi.mocked(modelService.getModelCatalogSnapshot).mockResolvedValue(
      buildInstalledCatalogSnapshot([])
    );
    vi.mocked(modelService.checkHardware).mockResolvedValue({ compatible: true });

    let finishAbort: () => void = () => {};
    const downloadStarted = new Promise<void>((resolve) => {
      vi.mocked(modelService.downloadModel).mockImplementation(async (_id, _cb, sig) => {
        resolve();
        return new Promise((_, reject) => {
          sig?.addEventListener('abort', () => {
            finishAbort = () => reject(new Error('Download cancelled'));
          });
        });
      });
    });

    const { result } = renderHook(() => useModelManager(true));
    void result.current.handleDownload(catalogModel);
    await downloadStarted;

    act(() => {
      result.current.handleCancelDownload(catalogModel.id);
    });

    await act(async () => {
      await result.current.handleDiscardDownload({
        ...catalogModel,
        interruptedDownload: { downloadedBytes: 50, totalBytes: 100, percentage: 50 },
      });
    });

    expect(cleanInterruptedDownload).not.toHaveBeenCalled();

    await act(async () => {
      finishAbort();
    });
  });
});
