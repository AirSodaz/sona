import { describe, expect, it, vi } from 'vitest';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import {
  buildBatchTranscriptionRequest,
  buildStreamingAsrRequest,
  resolveStreamingSlot,
} from '../transcriptionRequest';

vi.mock('../../modelService', () => ({
  PRESET_MODELS: [],
  PRESET_MODELS_MAP: new Map([
    ['qwen3-asr-0.6b-q8-gguf', {
      id: 'qwen3-asr-0.6b-q8-gguf',
      name: 'Qwen3 ASR',
      description: '',
      url: 'https://example.com/qwen.gguf',
      type: 'qwen3-asr',
      modes: ['batch'],
      languages: ['en', 'zh'],
      languageMode: 'auto',
      size: '1 GB',
      engine: 'llama-cpp',
      fileConfig: {
        model: 'Qwen3-ASR-0.6B-Q8_0.gguf',
        mmproj: 'mmproj-Qwen3-ASR-0.6B-Q8_0.gguf',
      },
    }],
  ]),
  modelService: {
    getModelRules: vi.fn(() => ({
      requiresPunctuation: false,
      requiresVad: true,
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

  it('builds a record streaming request with local model override and timeline enabled', () => {
    const config = buildTestConfig({
      batchModelPath: '/models/batch',
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

    const asrRequest = buildStreamingAsrRequest({
      appConfig: config,
      instanceId: 'record',
      modelPathOverride: '/models/runtime-streaming',
      language: 'ja',
      enableItn: true,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      engine: 'local',
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
  });

  it('disables timeline for non-record streaming instances', () => {
    const config = buildTestConfig({
      streamingModelPath: '/models/streaming',
      enableTimeline: true,
    });

    const request = buildStreamingAsrRequest({
      appConfig: config,
      instanceId: 'caption',
      language: 'en',
      enableItn: true,
    });

    expect(request.normalizationOptions).toEqual({ enableTimeline: false });
  });

  it('builds batch process requests with speaker processing and save target', () => {
    const config = buildTestConfig({
      batchModelPath: '/models/batch',
      batchSpeakerSegmentationModelPath: '/models/speaker-segmentation',
      batchSpeakerEmbeddingModelPath: '/models/speaker-embedding.onnx',
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
      mode: 'batch',
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
      batchModelPath: '/models/batch',
      batchVadEnabled: false,
    });

    const { request, asrRequest } = buildBatchTranscriptionRequest({
      appConfig: config,
      filePath: 'C:/audio/demo.wav',
      language: 'auto',
      enableItn: true,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      mode: 'batch',
      modelPath: '/models/batch',
      vadModel: null,
      batchSegmentationMode: 'whole',
    }));
    expect(request.asrRequest).toBe(asrRequest);
  });

  it('strips unsupported llama.cpp options but keeps VAD segmentation for Qwen batch requests', () => {
    const config = buildTestConfig({
      language: 'zh',
      enableITN: true,
      hotwords: ['Sona'],
      liveVadModelPath: '/models/vad.onnx',
      batchVadModelPath: '/models/vad.onnx',
      livePunctuationModelPath: '/models/punctuation.onnx',
      batchPunctuationModelPath: '/models/punctuation.onnx',
      batchSpeakerSegmentationModelPath: '/models/speaker-segmentation',
      asr: {
        selections: {
          batch: {
            engine: 'local',
            mode: 'batch',
            modelId: 'qwen3-asr-0.6b-q8-gguf',
            modelPath: '/models/qwen3-asr-0.6b-q8-gguf',
          },
        },
      },
    });

    const { request, asrRequest } = buildBatchTranscriptionRequest({
      appConfig: config,
      filePath: 'C:/audio/demo.wav',
      saveToPath: 'C:/audio/copy.wav',
      language: 'zh',
      enableItn: true,
    });

    expect(asrRequest).toEqual(expect.objectContaining({
      localEngine: 'llama-cpp',
      language: 'auto',
      enableItn: false,
      hotwords: null,
      vadModel: '/models/vad.onnx',
      punctuationModel: null,
      batchSegmentationMode: 'vad',
      modelType: 'qwen3-asr',
      fileConfig: expect.objectContaining({
        mmproj: 'mmproj-Qwen3-ASR-0.6B-Q8_0.gguf',
      }),
    }));
    expect(request.saveToPath).toBeNull();
    expect(request.speakerProcessing).toBeNull();
  });
});
