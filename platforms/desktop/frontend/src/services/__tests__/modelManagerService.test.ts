import { describe, expect, it } from 'vitest';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import {
  buildModelPathConfigPatch,
  buildModelRemovalConfigPatch,
  buildRestoreDefaultModelConfigPatch,
} from '../modelConfigPatches';
import type { ModelCatalogRestoreDefaults, ModelInfo } from '../modelService';

const streamingModel: ModelInfo = {
  id: 'streaming-model',
  name: 'Streaming Model',
  description: '',
  type: 'sensevoice',
  modes: ['streaming', 'batch'],
  language: 'en',
  size: '1 MB',
  engine: 'sherpa-onnx',
};

const vadModel: ModelInfo = {
  id: 'vad-model',
  name: 'VAD Model',
  description: '',
  type: 'vad',
  language: 'en',
  size: '1 MB',
  engine: 'sherpa-onnx',
};

function makeRestoreDefaults(overrides: Partial<ModelCatalogRestoreDefaults> = {}): ModelCatalogRestoreDefaults {
  return {
    punctuationModelPath: '',
    speakerSegmentationModelPath: '',
    speakerEmbeddingModelPath: '',
    enableITN: true,
    vadBufferSize: 5,
    maxConcurrent: 2,
    ...overrides,
  };
}

describe('modelConfigPatches', () => {
  it('builds ASR selection patches for streaming/Batch Model loads', () => {
    const config = buildTestConfig();

    expect(buildModelPathConfigPatch(config, streamingModel, '/models/streaming')).toEqual(expect.objectContaining({
      streamingModelPath: '/models/streaming',
      batchModelPath: '/models/streaming',
      asr: expect.objectContaining({
        selections: expect.objectContaining({
          live: expect.objectContaining({
            modelId: 'streaming-model',
            modelPath: '/models/streaming',
          }),
          caption: expect.objectContaining({
            modelId: 'streaming-model',
            modelPath: '/models/streaming',
          }),
          voiceTyping: expect.objectContaining({
            modelId: 'streaming-model',
            modelPath: '/models/streaming',
          }),
          batch: expect.objectContaining({
            modelId: 'streaming-model',
            modelPath: '/models/streaming',
          }),
        }),
      }),
    }));
  });

  it('builds per-scenario patches for auxiliary models', () => {
    const config = buildTestConfig();

    expect(buildModelPathConfigPatch(config, vadModel, '/models/vad')).toEqual({
      liveVadModelPath: '/models/vad',
      batchVadModelPath: '/models/vad',
    });
  });

  it('clears every config field pointing to a deleted model path', () => {
    const config = buildTestConfig({
      streamingModelPath: '/models/deleted',
      batchModelPath: '/models/deleted',
      livePunctuationModelPath: '/models/deleted',
      batchPunctuationModelPath: '/models/deleted',
      liveVadModelPath: '/models/deleted',
      batchVadModelPath: '/models/deleted',
      liveSpeakerSegmentationModelPath: '/models/deleted',
      batchSpeakerSegmentationModelPath: '/models/deleted',
      liveSpeakerEmbeddingModelPath: '/models/deleted',
      batchSpeakerEmbeddingModelPath: '/models/deleted',
    });

    expect(buildModelRemovalConfigPatch(config, '/models/deleted')).toEqual(expect.objectContaining({
      streamingModelPath: '',
      batchModelPath: '',
      livePunctuationModelPath: '',
      batchPunctuationModelPath: '',
      liveVadModelPath: '',
      batchVadModelPath: '',
      liveSpeakerSegmentationModelPath: '',
      batchSpeakerSegmentationModelPath: '',
      liveSpeakerEmbeddingModelPath: '',
      batchSpeakerEmbeddingModelPath: '',
      asr: expect.objectContaining({
        selections: expect.objectContaining({
          live: expect.objectContaining({ modelPath: '' }),
          caption: expect.objectContaining({ modelPath: '' }),
          voiceTyping: expect.objectContaining({ modelPath: '' }),
          batch: expect.objectContaining({ modelPath: '' }),
        }),
      }),
    }));
  });

  it('restores catalog defaults and keeps current paths when optional defaults are absent', () => {
    const config = buildTestConfig({
      streamingModelPath: '/current/live',
      batchModelPath: '/current/batch',
      liveVadModelPath: '/current/vad',
      batchVadModelPath: '/current/vad',
      liveVadBufferSize: 9,
      maxConcurrent: 4,
      enableITN: false,
    });

    const patch = buildRestoreDefaultModelConfigPatch(config, makeRestoreDefaults({
      streamingModelPath: '/models/default-live',
      batchModelPath: '/models/default-batch',
    }));
    const nextConfig = { ...config, ...patch };

    expect(nextConfig).toEqual(expect.objectContaining({
      streamingModelPath: '/models/default-live',
      batchModelPath: '/models/default-batch',
      // Restore defaults apply the catalog default VAD path to both scenarios.
      liveVadModelPath: '',
      batchVadModelPath: '',
      liveVadBufferSize: 5,
      batchVadBufferSize: 5,
      maxConcurrent: 2,
      enableITN: true,
      asr: expect.objectContaining({
        providers: expect.objectContaining({
          online: expect.objectContaining({
            'volcengine-doubao': expect.objectContaining({
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
              batchResourceId: 'volc.bigasr.auc_turbo',
            }),
          }),
        }),
      }),
    }));
  });
});
