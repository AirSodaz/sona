import { describe, expect, it } from 'vitest';
import { DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG } from '../../services/onlineAsrProviders';
import type { ModelConfig } from '../../types/config';
import type { ModelCatalogSnapshot } from '../../types/modelCatalog';
import {
  buildAsrModelDropdownOptions,
  resolveActiveAsrModelId,
  toDropdownOptions,
} from '../asrModelOptions';

const mockCatalog: ModelCatalogSnapshot = {
  modelsDir: '/models',
  models: [],
  sections: [],
  selectionOptions: {
    streaming: [
      {
        id: 'sensevoice-live',
        label: 'SenseVoice Live',
        isInstalled: true,
        installPath: '/models/sensevoice-live.onnx',
      },
      {
        id: 'whisper-live',
        label: 'Whisper Live',
        isInstalled: false,
        installPath: '/models/whisper-live.onnx',
      },
    ],
    batch: [
      {
        id: 'sensevoice-batch',
        label: 'SenseVoice Batch',
        isInstalled: true,
        installPath: '/models/sensevoice-batch.onnx',
      },
      {
        id: 'whisper-batch',
        label: 'Whisper Batch',
        isInstalled: false,
        installPath: '/models/whisper-batch.onnx',
      },
    ],
    speakerSegmentation: [],
    speakerEmbedding: [],
  },
  modelPathById: {
    'sensevoice-live': '/models/sensevoice-live.onnx',
    'sensevoice-batch': '/models/sensevoice-batch.onnx',
  },
  modelIdByNormalizedPath: {
    '/models/sensevoice-live.onnx': 'sensevoice-live',
    '/models/sensevoice-batch.onnx': 'sensevoice-batch',
  },
  pathMatchTokens: [
    { id: 'whisper-live', token: 'whisper-live' },
    { id: 'whisper-batch', token: 'whisper-batch' },
  ],
  dependencyRequestsByModelId: {},
  restoreDefaults: {
    enableITN: true,
    vadBufferSize: 8,
    maxConcurrent: 1,
  },
};

describe('asrModelOptions', () => {
  describe('resolveActiveAsrModelId', () => {
    it('resolves online provider with submodel', () => {
      const config: ModelConfig = {
        streamingModelPath: '',
        batchModelPath: '',
        asr: {
          selections: {
            live: {
              engine: 'online',
              mode: 'streaming',
              providerId: 'volcengine-doubao',
              modelId: 'doubao-pro',
              modelPath: '',
            },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: { online: {} },
        },
      };

      expect(resolveActiveAsrModelId(config, 'live', mockCatalog)).toBe(
        'volcengine-doubao::doubao-pro'
      );
    });

    it('resolves online provider without submodel', () => {
      const config: ModelConfig = {
        streamingModelPath: '',
        batchModelPath: '',
        asr: {
          selections: {
            live: {
              engine: 'online',
              mode: 'streaming',
              providerId: 'groq-whisper',
              modelId: null,
              modelPath: '',
            },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: { online: {} },
        },
      };

      expect(resolveActiveAsrModelId(config, 'live', mockCatalog)).toBe('groq-whisper');
    });

    it('resolves local model from selection.modelId if present', () => {
      const config: ModelConfig = {
        streamingModelPath: '/models/sensevoice-live.onnx',
        batchModelPath: '',
        asr: {
          selections: {
            live: {
              engine: 'local',
              mode: 'streaming',
              modelId: 'sensevoice-live',
              modelPath: '/models/sensevoice-live.onnx',
            },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: { online: {} },
        },
      };

      expect(resolveActiveAsrModelId(config, 'live', mockCatalog)).toBe('sensevoice-live');
    });

    it('resolves local model from path lookup when modelId is null', () => {
      const config: ModelConfig = {
        streamingModelPath: '',
        batchModelPath: 'C:\\Models\\SenseVoice-Batch.onnx',
        asr: {
          selections: {
            live: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: {
              engine: 'local',
              mode: 'batch',
              modelId: null,
              modelPath: 'C:\\Models\\SenseVoice-Batch.onnx',
            },
          },
          providers: { online: {} },
        },
      };

      const catalogWithWindowsPath: ModelCatalogSnapshot = {
        ...mockCatalog,
        modelIdByNormalizedPath: {
          'c:/models/sensevoice-batch.onnx': 'sensevoice-batch',
        },
      };

      expect(resolveActiveAsrModelId(config, 'batch', catalogWithWindowsPath)).toBe(
        'sensevoice-batch'
      );
    });

    it('resolves local model via token match when exact path not found', () => {
      const config: ModelConfig = {
        streamingModelPath: '/custom/path/whisper-batch/model.onnx',
        batchModelPath: '/custom/path/whisper-batch/model.onnx',
      };

      expect(resolveActiveAsrModelId(config, 'batch', mockCatalog)).toBe('whisper-batch');
    });
  });

  describe('toDropdownOptions', () => {
    it('filters out uninstalled models unless selected', () => {
      const options = mockCatalog.selectionOptions.streaming;
      const resultNotSelected = toDropdownOptions(options, 'sensevoice-live');
      expect(resultNotSelected.map((o) => o.value)).toEqual(['sensevoice-live']);

      const resultSelected = toDropdownOptions(options, 'whisper-live');
      expect(resultSelected.map((o) => o.value)).toEqual(['sensevoice-live', 'whisper-live']);
    });
  });

  describe('buildAsrModelDropdownOptions', () => {
    const t = (key: string, opts?: Record<string, unknown>) =>
      (opts?.defaultValue as string) ?? key;

    it('includes local installed models and configured cloud models for streaming slot', () => {
      const config: ModelConfig = {
        streamingModelPath: '',
        batchModelPath: '',
        asr: {
          selections: {
            live: { engine: 'local', mode: 'streaming', modelId: 'sensevoice-live', modelPath: '' },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: null, modelPath: '' },
          },
          providers: {
            online: {
              'volcengine-doubao': {
                ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
                appId: 'test-app',
                accessToken: 'test-token',
                apiKey: 'test-token',
              },
            },
          },
        },
      };

      const options = buildAsrModelDropdownOptions({
        slot: 'live',
        snapshot: mockCatalog,
        selectedModelId: 'sensevoice-live',
        modelConfig: config,
        t,
      });

      const values = options.map((o) => o.value);
      expect(values).toContain('sensevoice-live');
      expect(values).not.toContain('whisper-live');
      // Should include configured volcengine-doubao models
      expect(values.some((v) => v.startsWith('volcengine-doubao'))).toBe(true);
    });

    it('includes local installed models and configured cloud models for batch slot', () => {
      const config: ModelConfig = {
        streamingModelPath: '',
        batchModelPath: '',
        asr: {
          selections: {
            live: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
            batch: { engine: 'local', mode: 'batch', modelId: 'sensevoice-batch', modelPath: '' },
          },
          providers: {
            online: {
              'volcengine-doubao': {
                ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
                apiKey: 'test-token',
              },
            },
          },
        },
      };

      const options = buildAsrModelDropdownOptions({
        slot: 'batch',
        snapshot: mockCatalog,
        selectedModelId: 'sensevoice-batch',
        modelConfig: config,
        t,
      });

      const values = options.map((o) => o.value);
      expect(values).toContain('sensevoice-batch');
      expect(values.some((v) => v.startsWith('volcengine-doubao'))).toBe(true);
    });
  });
});
