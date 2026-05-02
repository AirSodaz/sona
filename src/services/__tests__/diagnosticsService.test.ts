import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  getPermissionState: vi.fn(),
  probeMicrophones: vi.fn(),
  probeSystemAudio: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('../audioDeviceService', () => ({
  getMicrophonePermissionState: mocks.getPermissionState,
  probeMicrophoneDeviceOptions: mocks.probeMicrophones,
  probeSystemAudioDeviceOptions: mocks.probeSystemAudio,
}));

import { diagnosticsService } from '../diagnosticsService';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import { useOnboardingStore } from '../../stores/onboardingStore';
import { useVoiceTypingRuntimeStore } from '../../stores/voiceTypingRuntimeStore';

const STREAMING_PARAFORMER_PATH = 'C:\\models\\sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en';
const STREAMING_SENSEVOICE_PATH = 'C:\\models\\sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const OFFLINE_QWEN_PATH = 'C:\\models\\sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25';
const OFFLINE_FUNASR_NANO_PATH = 'C:\\models\\sherpa-onnx-funasr-nano-int8-2025-12-30';
const VAD_PATH = 'C:\\models\\silero_vad.onnx';
const PUNCTUATION_PATH = 'C:\\models\\ct-transformer.onnx';

let runtimeEnvironment = {
  ffmpegPath: 'C:\\app\\ffmpeg.exe',
  ffmpegExists: true,
  logDirPath: 'C:\\app\\logs',
};
let asrRuntimeMetrics: any = {
  modelLoad: null,
  liveInference: null,
  batchInference: null,
};

function t(key: string, options?: Record<string, unknown>) {
  return (options?.defaultValue as string | undefined) ?? key;
}

function setPathStatuses({
  filePaths = [],
  directoryPaths = [],
  unknownPaths = [],
}: {
  filePaths?: string[];
  directoryPaths?: string[];
  unknownPaths?: string[];
}) {
  const files = new Set(filePaths);
  const directories = new Set(directoryPaths);
  const unknown = new Set(unknownPaths);

  mocks.invoke.mockImplementation(async (command: string, payload?: { paths?: string[] }) => {
    if (command === 'get_runtime_environment_status') {
      return runtimeEnvironment;
    }

    if (command === 'get_asr_runtime_metrics') {
      return asrRuntimeMetrics;
    }

    if (command === 'get_path_statuses') {
      return (payload?.paths ?? []).map((path) => ({
        path,
        kind: unknown.has(path)
          ? 'unknown'
          : files.has(path)
            ? 'file'
            : directories.has(path)
              ? 'directory'
              : 'missing',
        error: unknown.has(path) ? 'Scope denied' : null,
      }));
    }

    throw new Error(`Unexpected command: ${command}`);
  });
}

describe('diagnosticsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
      },
    });
    useOnboardingStore.setState({
      persistedState: { version: 1, status: 'pending' },
    });
    useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();

    mocks.getPermissionState.mockResolvedValue('prompt');
    mocks.probeMicrophones.mockResolvedValue({
      options: [
        { label: 'Auto', value: 'default' },
        { label: 'USB Mic', value: 'usb-mic' },
      ],
      available: true,
      source: 'native',
    });
    mocks.probeSystemAudio.mockResolvedValue({
      options: [{ label: 'Auto', value: 'default' }],
      available: true,
      source: 'native',
    });
    runtimeEnvironment = {
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: true,
      logDirPath: 'C:\\app\\logs',
    };
    asrRuntimeMetrics = {
      modelLoad: null,
      liveInference: null,
      batchInference: null,
    };
    setPathStatuses({});
  });

  it('marks a missing live model as missing and points to model settings', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        offlineModelPath: OFFLINE_QWEN_PATH,
      },
    });
    setPathStatuses({ filePaths: [OFFLINE_QWEN_PATH] });

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const modelsSection = snapshot.sections.find((section) => section.id === 'models');
    const liveModelCheck = modelsSection?.checks.find((check) => check.id === 'live-model');

    expect(liveModelCheck).toEqual(
      expect.objectContaining({
        status: 'missing',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
  });

  it('returns a warning when punctuation is required but not configured', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_PARAFORMER_PATH,
      },
    });
    setPathStatuses({ directoryPaths: [STREAMING_PARAFORMER_PATH] });

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const modelsSection = snapshot.sections.find((section) => section.id === 'models');
    const punctuationCheck = modelsSection?.checks.find((check) => check.id === 'punctuation');

    expect(punctuationCheck).toEqual(
      expect.objectContaining({
        status: 'warning',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
  });

  it('uses the same unverified-path policy for live and offline model checks', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_SENSEVOICE_PATH,
        offlineModelPath: OFFLINE_QWEN_PATH,
      },
    });
    setPathStatuses({
      unknownPaths: [STREAMING_SENSEVOICE_PATH, OFFLINE_QWEN_PATH],
    });

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const modelsSection = snapshot.sections.find((section) => section.id === 'models');
    const liveModelCheck = modelsSection?.checks.find((check) => check.id === 'live-model');
    const offlineModelCheck = modelsSection?.checks.find((check) => check.id === 'offline-model');

    expect(liveModelCheck).toEqual(
      expect.objectContaining({
        status: 'info',
        description: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
    expect(offlineModelCheck).toEqual(
      expect.objectContaining({
        status: 'info',
        description: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
  });

  it('surfaces microphone permission denial as the clearest live-record repair action', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_PARAFORMER_PATH,
      },
    });
    setPathStatuses({ directoryPaths: [STREAMING_PARAFORMER_PATH] });
    mocks.getPermissionState.mockResolvedValue('denied');

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const inputSection = snapshot.sections.find((section) => section.id === 'input-capture');
    const permissionCheck = inputSection?.checks.find((check) => check.id === 'microphone-permission');
    const liveOverview = snapshot.overview.find((card) => card.id === 'live-record');

    expect(permissionCheck).toEqual(
      expect.objectContaining({
        status: 'failed',
        action: expect.objectContaining({
          kind: 'request_microphone_permission',
        }),
      }),
    );
    expect(liveOverview?.action).toEqual(
      expect.objectContaining({
        kind: 'request_microphone_permission',
      }),
    );
  });

  it('reports runtime failures for voice typing and ffmpeg separately', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        voiceTypingEnabled: true,
        voiceTypingShortcut: 'Alt+V',
        streamingModelPath: STREAMING_SENSEVOICE_PATH,
        vadModelPath: VAD_PATH,
      },
    });
    setPathStatuses({
      directoryPaths: [STREAMING_SENSEVOICE_PATH],
      filePaths: [VAD_PATH],
    });
    useVoiceTypingRuntimeStore.getState().reportRuntimeError('warmup', 'Warm-up failed.');
    runtimeEnvironment = {
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: false,
      logDirPath: 'C:\\app\\logs',
    };

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const runtimeSection = snapshot.sections.find((section) => section.id === 'runtime-environment');
    const voiceTypingCheck = runtimeSection?.checks.find((check) => check.id === 'voice-typing');
    const ffmpegCheck = runtimeSection?.checks.find((check) => check.id === 'ffmpeg');

    expect(voiceTypingCheck).toEqual(
      expect.objectContaining({
        status: 'failed',
        action: expect.objectContaining({
          kind: 'retry_voice_typing_warmup',
        }),
      }),
    );
    expect(ffmpegCheck).toEqual(
      expect.objectContaining({
        status: 'failed',
        action: expect.objectContaining({
          kind: 'open_log_folder',
        }),
      }),
    );
  });

  it('keeps unverifiable model paths as non-blocking diagnostics', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        offlineModelPath: OFFLINE_QWEN_PATH,
        punctuationModelPath: PUNCTUATION_PATH,
      },
    });
    setPathStatuses({
      unknownPaths: [OFFLINE_QWEN_PATH, PUNCTUATION_PATH],
    });

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const modelsSection = snapshot.sections.find((section) => section.id === 'models');
    const offlineModelCheck = modelsSection?.checks.find((check) => check.id === 'offline-model');

    expect(offlineModelCheck).toEqual(
      expect.objectContaining({
        status: 'info',
        description: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
      }),
    );
  });

  it('keeps vad missing-path failures and punctuation unknown-path warnings on their current severities', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_SENSEVOICE_PATH,
        offlineModelPath: OFFLINE_FUNASR_NANO_PATH,
        vadModelPath: VAD_PATH,
        punctuationModelPath: PUNCTUATION_PATH,
      },
    });
    setPathStatuses({
      directoryPaths: [STREAMING_SENSEVOICE_PATH, OFFLINE_FUNASR_NANO_PATH],
      unknownPaths: [PUNCTUATION_PATH],
    });

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const modelsSection = snapshot.sections.find((section) => section.id === 'models');
    const vadCheck = modelsSection?.checks.find((check) => check.id === 'vad');
    const punctuationCheck = modelsSection?.checks.find((check) => check.id === 'punctuation');

    expect(vadCheck).toEqual(
      expect.objectContaining({
        status: 'failed',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
    expect(punctuationCheck).toEqual(
      expect.objectContaining({
        status: 'warning',
        description: 'Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.',
        action: expect.objectContaining({
          kind: 'open_settings',
          settingsTab: 'models',
        }),
      }),
    );
  });

  it('keeps live-record overview action precedence on models before permission', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
      },
    });
    mocks.getPermissionState.mockResolvedValue('denied');

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const liveOverview = snapshot.overview.find((card) => card.id === 'live-record');

    expect(liveOverview?.action).toEqual(
      expect.objectContaining({
        kind: 'open_settings',
        settingsTab: 'models',
      }),
    );
  });

  it('falls back to input-device repair when live-record models and permission are ready', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_SENSEVOICE_PATH,
        vadModelPath: VAD_PATH,
        microphoneId: 'missing-mic',
      },
    });
    setPathStatuses({
      directoryPaths: [STREAMING_SENSEVOICE_PATH],
      filePaths: [VAD_PATH],
    });
    mocks.getPermissionState.mockResolvedValue('granted');

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const liveOverview = snapshot.overview.find((card) => card.id === 'live-record');

    expect(liveOverview?.action).toEqual(
      expect.objectContaining({
        kind: 'open_settings',
        settingsTab: 'microphone',
      }),
    );
  });

  it('keeps batch-import overview action precedence on models before ffmpeg logs', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        offlineModelPath: OFFLINE_QWEN_PATH,
      },
    });
    runtimeEnvironment = {
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: false,
      logDirPath: 'C:\\app\\logs',
    };

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const batchImportOverview = snapshot.overview.find((card) => card.id === 'batch-import');

    expect(batchImportOverview?.action).toEqual(
      expect.objectContaining({
        kind: 'open_settings',
        settingsTab: 'models',
      }),
    );
  });

  it('uses open-log-folder as the batch-import overview action once models are ready', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        offlineModelPath: OFFLINE_QWEN_PATH,
      },
    });
    setPathStatuses({ directoryPaths: [OFFLINE_QWEN_PATH] });
    runtimeEnvironment = {
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: false,
      logDirPath: 'C:\\app\\logs',
    };

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const batchImportOverview = snapshot.overview.find((card) => card.id === 'batch-import');

    expect(batchImportOverview?.action).toEqual(
      expect.objectContaining({
        kind: 'open_log_folder',
      }),
    );
  });

  it('adds ASR performance diagnostics when runtime metrics are available', async () => {
    asrRuntimeMetrics = {
      modelLoad: {
        occurredAtMs: 1760000000000,
        instanceId: 'record',
        modelPath: 'C:\\models\\sensevoice',
        modelType: 'sensevoice',
        recognizerKind: 'offline',
        numThreads: 4,
        reusedFromPool: false,
        loadMs: 123.4,
        rssBeforeMb: 416.25,
        rssAfterMb: 512.5,
        rssDeltaMb: 96.25,
        processRssMb: 512.5,
      },
      liveInference: {
        occurredAtMs: 1760000001000,
        source: 'live',
        instanceId: 'record',
        stage: 'final',
        isFinal: true,
        audioDurationMs: 1600,
        bufferedSamples: 25600,
        audioExtractMs: null,
        decodeMs: 42.2,
        emitLatencyMs: 60.1,
        totalMs: null,
        rtf: 0.026,
        segmentCount: null,
        processRssMb: 520.1,
      },
      batchInference: {
        occurredAtMs: 1760000002000,
        source: 'batch',
        instanceId: null,
        stage: 'batch_complete',
        isFinal: true,
        audioDurationMs: 120000,
        bufferedSamples: 1920000,
        audioExtractMs: 320.4,
        decodeMs: 1800.2,
        emitLatencyMs: null,
        totalMs: 2500.6,
        rtf: 0.015,
        segmentCount: 8,
        processRssMb: 640.75,
      },
    };

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const asrSection = snapshot.sections.find((section) => section.id === 'asr-performance');
    const modelMemoryCheck = asrSection?.checks.find((check) => check.id === 'asr-model-memory');
    const liveLatencyCheck = asrSection?.checks.find((check) => check.id === 'asr-live-latency');
    const batchLatencyCheck = asrSection?.checks.find((check) => check.id === 'asr-batch-latency');

    expect(asrSection?.title).toBe('ASR Performance');
    expect(modelMemoryCheck).toEqual(expect.objectContaining({
      status: 'ready',
      title: 'Model memory',
      description: expect.stringContaining('sensevoice'),
      meta: expect.stringContaining('RSS 512.5 MB'),
    }));
    expect(modelMemoryCheck?.meta).toContain('delta +96.3 MB');
    expect(liveLatencyCheck).toEqual(expect.objectContaining({
      status: 'ready',
      title: 'Live transcription latency',
      meta: expect.stringContaining('decode 42 ms'),
    }));
    expect(liveLatencyCheck?.meta).toContain('latency 60 ms');
    expect(liveLatencyCheck?.meta).toContain('RTF 0.03');
    expect(batchLatencyCheck).toEqual(expect.objectContaining({
      status: 'ready',
      title: 'Batch transcription latency',
      meta: expect.stringContaining('total 2501 ms'),
    }));
    expect(batchLatencyCheck?.meta).toContain('extract 320 ms');
    expect(batchLatencyCheck?.meta).toContain('segments 8');
  });

  it('keeps ASR performance diagnostics informational when no metrics have been captured', async () => {
    const snapshot = await diagnosticsService.collectSnapshot(t);
    const asrSection = snapshot.sections.find((section) => section.id === 'asr-performance');

    expect(asrSection?.checks).toEqual([
      expect.objectContaining({
        id: 'asr-model-memory',
        status: 'info',
        description: 'No ASR runtime metrics have been captured yet.',
      }),
      expect.objectContaining({
        id: 'asr-live-latency',
        status: 'info',
        description: 'No live transcription latency has been captured yet.',
      }),
      expect.objectContaining({
        id: 'asr-batch-latency',
        status: 'info',
        description: 'No batch transcription latency has been captured yet.',
      }),
    ]);
  });
});
