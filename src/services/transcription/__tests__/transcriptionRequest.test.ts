import { describe, expect, it, vi } from 'vitest';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import {
  buildBatchTranscriptionRequest,
  buildRecognizerInitRequest,
  resolveStreamingSlot,
} from '../transcriptionRequest';

vi.mock('../../modelService', () => ({
  PRESET_MODELS: [],
  PRESET_MODELS_MAP: new Map(),
  modelService: {
    getModelRules: vi.fn(() => ({
      requiresPunctuation: false,
      requiresVad: false,
    })),
  },
}));

describe('transcriptionRequest helpers', () => {
  it('resolves streaming slots from recognizer instance ids', () => {
    expect(resolveStreamingSlot('record')).toBe('live');
    expect(resolveStreamingSlot('caption')).toBe('caption');
    expect(resolveStreamingSlot('voice-typing')).toBe('voiceTyping');
    expect(resolveStreamingSlot('custom')).toBe('live');
  });

  it('builds a record recognizer init request with local model override and timeline enabled', () => {
    const config = buildTestConfig({
      offlineModelPath: '/models/offline',
      streamingModelPath: '/models/streaming',
      enableTimeline: true,
      enableITN: false,
      textReplacementSets: [
        {
          id: 'set-1',
          name: 'test',
          enabled: true,
          ignoreCase: false,
          rules: [{ from: 'apple', to: 'orange' }],
        },
      ],
    });

    const { request, asrRequest } = buildRecognizerInitRequest({
      appConfig: config,
      instanceId: 'record',
      modelPathOverride: '/models/runtime-streaming',
      language: 'ja',
      enableItn: true,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      engine: 'local-sherpa',
      mode: 'streaming',
      modelPath: '/models/runtime-streaming',
      language: 'ja',
      enableItn: true,
      normalizationOptions: { enableTimeline: true },
      postprocessOptions: {
        textReplacementSets: config.textReplacementSets,
        dropFinalDotSegments: true,
      },
    }));
    expect(request).toEqual({
      instanceId: 'record',
      asrRequest,
    });
  });

  it('disables timeline for non-record streaming instances', () => {
    const config = buildTestConfig({
      streamingModelPath: '/models/streaming',
      enableTimeline: true,
    });

    const { request } = buildRecognizerInitRequest({
      appConfig: config,
      instanceId: 'caption',
      language: 'en',
      enableItn: true,
    });

    expect(request.asrRequest.normalizationOptions).toEqual({ enableTimeline: false });
  });

  it('builds batch process requests with speaker processing and save target', () => {
    const config = buildTestConfig({
      offlineModelPath: '/models/offline',
      speakerSegmentationModelPath: '/models/speaker-segmentation',
      speakerEmbeddingModelPath: '/models/speaker-embedding.onnx',
      speakerProfiles: [
        { id: 'profile-1', name: 'Alice', enabled: true, samples: [] },
      ],
      textReplacementSets: [
        {
          id: 'set-1',
          name: 'test',
          enabled: true,
          ignoreCase: false,
          rules: [{ from: 'hello', to: 'hi' }],
        },
      ],
    });

    const { request, asrRequest } = buildBatchTranscriptionRequest({
      appConfig: config,
      filePath: 'C:/audio/demo.wav',
      saveToPath: 'C:/audio/demo.json',
      modelPathOverride: '/models/runtime-offline',
      language: 'zh',
      enableItn: false,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      mode: 'offline',
      modelPath: '/models/runtime-offline',
      language: 'zh',
      enableItn: false,
      batchSegmentationMode: 'vad',
      postprocessOptions: {
        textReplacementSets: config.textReplacementSets,
        dropFinalDotSegments: true,
      },
    }));
    expect(request).toEqual({
      filePath: 'C:/audio/demo.wav',
      saveToPath: 'C:/audio/demo.json',
      speakerProcessing: {
        speakerSegmentationModelPath: '/models/speaker-segmentation',
        speakerEmbeddingModelPath: '/models/speaker-embedding.onnx',
        speakerProfiles: [
          { id: 'profile-1', name: 'Alice', enabled: true, samples: [] },
        ],
      },
      asrRequest,
    });
  });

  it('builds batch process requests in whole-file mode when batch VAD is disabled', () => {
    const config = buildTestConfig({
      offlineModelPath: '/models/offline',
      batchVadEnabled: false,
    });

    const { request, asrRequest } = buildBatchTranscriptionRequest({
      appConfig: config,
      filePath: 'C:/audio/demo.wav',
      language: 'auto',
      enableItn: true,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      mode: 'offline',
      modelPath: '/models/offline',
      vadModel: null,
      batchSegmentationMode: 'whole',
    }));
    expect(request.asrRequest).toBe(asrRequest);
  });
});
