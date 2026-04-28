import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  exists: vi.fn(),
  getPermissionState: vi.fn(),
  probeMicrophones: vi.fn(),
  probeSystemAudio: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  exists: mocks.exists,
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
const VAD_PATH = 'C:\\models\\silero_vad.onnx';

function t(key: string, options?: Record<string, unknown>) {
  return (options?.defaultValue as string | undefined) ?? key;
}

function setExistingPaths(paths: string[]) {
  const existing = new Set(paths);
  mocks.exists.mockImplementation(async (path: string) => existing.has(path));
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
    mocks.invoke.mockResolvedValue({
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: true,
      logDirPath: 'C:\\app\\logs',
    });
    setExistingPaths([]);
  });

  it('marks a missing live model as missing and points to model settings', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        offlineModelPath: OFFLINE_QWEN_PATH,
      },
    });
    setExistingPaths([OFFLINE_QWEN_PATH]);

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
    setExistingPaths([STREAMING_PARAFORMER_PATH]);

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

  it('surfaces microphone permission denial as the clearest live-record repair action', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_PARAFORMER_PATH,
      },
    });
    setExistingPaths([STREAMING_PARAFORMER_PATH]);
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
    setExistingPaths([STREAMING_SENSEVOICE_PATH, VAD_PATH]);
    useVoiceTypingRuntimeStore.getState().reportRuntimeError('warmup', 'Warm-up failed.');
    mocks.invoke.mockResolvedValue({
      ffmpegPath: 'C:\\app\\ffmpeg.exe',
      ffmpegExists: false,
      logDirPath: 'C:\\app\\logs',
    });

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
});
