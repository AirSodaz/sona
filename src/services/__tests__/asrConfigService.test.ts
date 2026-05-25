import { describe, expect, it, vi } from 'vitest';
import type { AppConfig } from '../../types/config';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import {
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
});
