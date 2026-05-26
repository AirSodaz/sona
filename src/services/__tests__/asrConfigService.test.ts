import { describe, expect, it, vi } from 'vitest';
import type { AppConfig } from '../../types/config';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import onlineAsrProviderManifest from '../../shared/online-asr-providers.json';
import {
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
  VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
  createVolcengineDoubaoSelection,
  isAsrRequestConfigured,
  isVolcengineFlashBatchMode,
  resolveAsrTranscriptionRequest,
  syncLegacyAsrSelectionFields,
} from '../asrConfigService';

const mocks = vi.hoisted(() => {
  const presetModels = [
    {
      id: 'local-live',
      name: 'Local Live',
      description: '',
      url: '',
      type: 'zipformer',
      modes: ['streaming'],
      language: 'zh',
      size: '1 MB',
      engine: 'sherpa-onnx',
      rules: { requiresVad: true, requiresPunctuation: false },
      fileConfig: {
        encoder: 'encoder.onnx',
        decoder: 'decoder.onnx',
        joiner: 'joiner.onnx',
        tokens: 'tokens.txt',
      },
      filename: 'local-live',
    },
    {
      id: 'local-batch',
      name: 'Local Batch',
      description: '',
      url: '',
      type: 'sensevoice',
      modes: ['offline'],
      language: 'zh',
      size: '1 MB',
      engine: 'sherpa-onnx',
      rules: { requiresVad: true, requiresPunctuation: true },
      fileConfig: {
        model: 'model.onnx',
        tokens: 'tokens.txt',
      },
      filename: 'local-batch',
    },
  ] as any[];
  return { presetModels };
});

vi.mock('../modelService', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../modelService')>();
  return {
    ...actual,
    PRESET_MODELS: mocks.presetModels,
    PRESET_MODELS_MAP: new Map(mocks.presetModels.map((model) => [model.id, model])),
    modelService: {
      getModelRules: vi.fn((modelId: string) => ({
        requiresVad: true,
        requiresPunctuation: modelId === 'local-batch',
      })),
    },
  };
});

function buildAsrConfig(overrides: Partial<AppConfig> = {}): AppConfig {
  return buildTestConfig({
    asr: {
      selections: {
        live: {
          engine: 'local-sherpa',
          mode: 'streaming',
          modelId: 'local-live',
          modelPath: 'C:/models/local-live',
        },
        caption: {
          engine: 'local-sherpa',
          mode: 'streaming',
          modelId: 'local-live',
          modelPath: 'C:/models/local-live',
        },
        voiceTyping: {
          engine: 'local-sherpa',
          mode: 'streaming',
          modelId: 'local-live',
          modelPath: 'C:/models/local-live',
        },
        batch: {
          engine: 'local-sherpa',
          mode: 'offline',
          modelId: 'local-batch',
          modelPath: 'C:/models/local-batch',
        },
      },
    },
    streamingModelPath: 'C:/legacy/live',
    offlineModelPath: 'C:/legacy/batch',
    vadModelPath: 'C:/models/silero_vad.onnx',
    punctuationModelPath: 'C:/models/punct',
    vadBufferSize: 8,
    enableITN: true,
    language: 'zh',
    enableTimeline: true,
    hotwordSets: [
      {
        id: 'hotwords',
        name: 'Hotwords',
        enabled: true,
        rules: [
          { id: 'hw-1', text: 'Sona' },
          { id: 'hw-2', text: '  ' },
        ],
      },
    ],
    textReplacementSets: [
      {
        id: 'replace',
        name: 'Replace',
        enabled: true,
        ignoreCase: false,
        rules: [{ id: 'r-1', from: 'foo', to: 'bar' }],
      },
    ],
    ...overrides,
  });
}

describe('asrConfigService', () => {
  it('resolves a local sherpa streaming request from the ASR live selection', () => {
    const request = resolveAsrTranscriptionRequest(buildAsrConfig(), 'live');

    expect(request).toMatchObject({
      engine: 'local-sherpa',
      mode: 'streaming',
      modelId: 'local-live',
      modelPath: 'C:/models/local-live',
      modelType: 'zipformer',
      language: 'zh',
      enableItn: true,
      vadModel: 'C:/models/silero_vad.onnx',
      punctuationModel: null,
      vadBuffer: 8,
      hotwords: 'Sona',
      normalizationOptions: {
        enableTimeline: true,
      },
      postprocessOptions: {
        textReplacementSets: [
          {
            id: 'replace',
            name: 'Replace',
            enabled: true,
            ignoreCase: false,
            rules: [{ id: 'r-1', from: 'foo', to: 'bar' }],
          },
        ],
        dropFinalDotSegments: true,
      },
    });
    expect(request.fileConfig).toEqual({
      encoder: 'encoder.onnx',
      decoder: 'decoder.onnx',
      joiner: 'joiner.onnx',
      tokens: 'tokens.txt',
    });
  });

  it('resolves a local sherpa batch request with punctuation, VAD, and speaker settings', () => {
    const request = resolveAsrTranscriptionRequest(buildAsrConfig(), 'batch');

    expect(request).toMatchObject({
      engine: 'local-sherpa',
      mode: 'offline',
      modelId: 'local-batch',
      modelPath: 'C:/models/local-batch',
      modelType: 'sensevoice',
      vadModel: 'C:/models/silero_vad.onnx',
      punctuationModel: 'C:/models/punct',
      vadBuffer: 8,
    });
    expect(request.fileConfig).toEqual({
      model: 'model.onnx',
      tokens: 'tokens.txt',
    });
  });

  it('falls back to legacy model paths when ASR selections are missing', () => {
    const request = resolveAsrTranscriptionRequest(buildTestConfig({
      streamingModelPath: 'C:/legacy/live',
      offlineModelPath: 'C:/legacy/batch',
      vadBufferSize: 5,
      asr: undefined,
    }), 'voiceTyping');

    expect(request).toMatchObject({
      engine: 'local-sherpa',
      mode: 'streaming',
      modelPath: 'C:/legacy/live',
      modelType: 'sensevoice',
    });
  });

  it('updates new ASR selections and legacy fields together', () => {
    const config = buildAsrConfig();
    const patch = syncLegacyAsrSelectionFields(config, 'batch', {
      modelId: 'local-batch',
      modelPath: 'D:/models/local-batch',
    });

    expect(patch.offlineModelPath).toBe('D:/models/local-batch');
    expect(patch.streamingModelPath).toBeUndefined();
    expect(patch.asr?.selections.batch).toMatchObject({
      engine: 'local-sherpa',
      mode: 'offline',
      modelId: 'local-batch',
      modelPath: 'D:/models/local-batch',
    });
    expect(patch.asr?.selections.live.modelPath).toBe('C:/models/local-live');
  });

  it('normalizes legacy Volcengine Doubao selections into online provider requests without local model paths', () => {
    const config = buildAsrConfig({
      asr: {
        selections: {
          live: createVolcengineDoubaoSelection('streaming'),
          caption: createVolcengineDoubaoSelection('streaming'),
          voiceTyping: createVolcengineDoubaoSelection('streaming'),
          batch: createVolcengineDoubaoSelection('offline'),
        },
        providers: {
          online: {
            'volcengine-doubao': {
              apiKey: 'volc-test-key',
              streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
              streamingResourceId: 'volc.seedasr.sauc.duration',
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
              batchResourceId: 'volc.bigasr.auc_turbo',
            },
          },
        },
      },
    } as Partial<AppConfig>);

    const live = resolveAsrTranscriptionRequest(config, 'live');
    const batch = resolveAsrTranscriptionRequest(config, 'batch');

    expect(live).toMatchObject({
      engine: 'online',
      mode: 'streaming',
      modelId: null,
      modelPath: '',
      providerId: 'volcengine-doubao',
      profileId: 'volcengine-doubao-default',
      onlineProvider: {
        providerId: 'volcengine-doubao',
        profileId: 'volcengine-doubao-default',
        config: {
          apiKey: 'volc-test-key',
          streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
          streamingResourceId: 'volc.seedasr.sauc.duration',
        },
      },
    });
    expect(batch).toMatchObject({
      engine: 'online',
      mode: 'offline',
      modelPath: '',
      onlineProvider: {
        providerId: 'volcengine-doubao',
        config: {
          batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
          batchResourceId: 'volc.bigasr.auc_turbo',
        },
      },
    });
    expect('volcengine' in live).toBe(false);
    expect(isAsrRequestConfigured(live)).toBe(true);
    expect(isAsrRequestConfigured(batch)).toBe(true);
  });

  it('keeps the default Volcengine local batch provider on flash recognize mode', () => {
    const volcengineManifest = onlineAsrProviderManifest.providers.find(
      (provider) => provider.id === 'volcengine-doubao',
    );

    expect(volcengineManifest?.defaults.batchEndpoint).toBe(VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT);
    expect(volcengineManifest?.defaults.batchResourceId).toBe(VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID);
    expect(DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG).toMatchObject({
      batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
      batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
    });
    expect(isVolcengineFlashBatchMode(DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG)).toBe(true);
  });

  it('normalizes saved Volcengine async batch modes back to flash for local batch import', () => {
    const config = buildAsrConfig({
      asr: {
        selections: {
          live: createVolcengineDoubaoSelection('streaming'),
          caption: createVolcengineDoubaoSelection('streaming'),
          voiceTyping: createVolcengineDoubaoSelection('streaming'),
          batch: createVolcengineDoubaoSelection('offline'),
        },
        providers: {
          volcengineDoubao: {
            apiKey: 'volc-test-key',
            streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
            streamingResourceId: 'volc.seedasr.sauc.duration',
            batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/idle/submit',
            batchResourceId: 'volc.bigasr.auc_idle',
          },
        },
      },
    } as Partial<AppConfig>);

    const request = resolveAsrTranscriptionRequest(config, 'batch');

    expect(request.onlineProvider).toMatchObject({
      providerId: 'volcengine-doubao',
      config: {
        batchEndpoint: VOLCENGINE_DOUBAO_FLASH_BATCH_ENDPOINT,
        batchResourceId: VOLCENGINE_DOUBAO_FLASH_BATCH_RESOURCE_ID,
      },
    });
    expect(isAsrRequestConfigured(request)).toBe(true);
  });

  it('treats Volcengine Doubao as not ready until API key and resource ids are configured', () => {
    const config = buildAsrConfig({
      asr: {
        selections: {
          live: createVolcengineDoubaoSelection('streaming'),
          caption: createVolcengineDoubaoSelection('streaming'),
          voiceTyping: createVolcengineDoubaoSelection('streaming'),
          batch: createVolcengineDoubaoSelection('offline'),
        },
      },
    } as Partial<AppConfig>);

    expect(isAsrRequestConfigured(resolveAsrTranscriptionRequest(config, 'live'))).toBe(false);
    expect(isAsrRequestConfigured(resolveAsrTranscriptionRequest(config, 'batch'))).toBe(false);
    expect(isAsrRequestConfigured(resolveAsrTranscriptionRequest(buildAsrConfig(), 'live'))).toBe(true);
  });

  it('preserves Volcengine streaming provider config and keeps local batch on flash when updating a local legacy selection', () => {
    const config = buildAsrConfig({
      asr: {
        selections: {
          live: createVolcengineDoubaoSelection('streaming'),
          caption: createVolcengineDoubaoSelection('streaming'),
          voiceTyping: createVolcengineDoubaoSelection('streaming'),
          batch: {
            engine: 'local-sherpa',
            mode: 'offline',
            modelId: 'local-batch',
            modelPath: 'C:/models/local-batch',
          },
        },
        providers: {
          volcengineDoubao: {
            apiKey: 'volc-test-key',
            streamingEndpoint: 'wss://custom.example.com/stream',
            streamingResourceId: 'volc.seedasr.sauc.concurrent',
            batchEndpoint: 'https://custom.example.com/flash',
            batchResourceId: 'volc.bigasr.auc_turbo',
          },
        },
      },
    } as Partial<AppConfig>);

    const patch = syncLegacyAsrSelectionFields(config, 'batch', {
      modelId: 'local-batch',
      modelPath: 'D:/models/local-batch',
    });

    expect(patch.asr?.providers?.online?.['volcengine-doubao']).toMatchObject({
      apiKey: 'volc-test-key',
      streamingEndpoint: 'wss://custom.example.com/stream',
      streamingResourceId: 'volc.seedasr.sauc.concurrent',
      batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
      batchResourceId: 'volc.bigasr.auc_turbo',
    });
    expect(patch.asr?.selections.live.engine).toBe('online');
    expect(patch.offlineModelPath).toBe('D:/models/local-batch');
  });
});
