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

const STREAMING_SENSEVOICE_PATH = 'C:\\models\\sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const BATCH_QWEN_PATH = 'C:\\models\\sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25';
const VAD_PATH = 'C:\\models\\silero_vad.onnx';

const runtimeEnvironment = {
  ffmpegPath: 'C:\\app\\ffmpeg.exe',
  ffmpegExists: true,
  logDirPath: 'C:\\app\\logs',
};
let coreSnapshot: unknown;

function t(key: string, options?: Record<string, unknown>) {
  const translated: Record<string, string> = {
    'settings.diagnostics.open_model_settings': '打开模型设置',
    'settings.diagnostics.request_permission': '请求权限',
    'settings.mic_auto': '自动',
  };

  return translated[key] ?? (options?.defaultValue as string | undefined) ?? key;
}

function makeCoreSnapshot() {
  return {
    scannedAt: '2026-05-03T00:00:00.000Z',
    runtimeEnvironment,
    config: {
      streamingModelPath: STREAMING_SENSEVOICE_PATH,
      batchModelPath: BATCH_QWEN_PATH,
      vadModelPath: VAD_PATH,
      punctuationModelPath: '',
      microphoneId: 'default',
    },
    selectedModels: {
      live: { id: 'sensevoice-live', name: 'SenseVoice Live' },
      batch: { id: 'qwen-batch', name: 'Qwen Batch' },
    },
    modelRules: {
      live: { requiresVad: true, requiresPunctuation: false },
      batch: { requiresVad: false, requiresPunctuation: false },
    },
    pathStatuses: {
      liveModel: { path: STREAMING_SENSEVOICE_PATH, kind: 'directory', error: null },
      batchModel: { path: BATCH_QWEN_PATH, kind: 'directory', error: null },
      vad: { path: VAD_PATH, kind: 'file', error: null },
      punctuation: null,
    },
    permissionState: 'prompt',
    microphoneProbe: {
      options: [{ label: 'Auto', value: 'default' }],
      available: true,
      source: 'native',
    },
    systemAudioProbe: {
      options: [{ label: 'Auto', value: 'default' }],
      available: true,
      source: 'native',
    },
    voiceTypingReadiness: {
      state: 'off',
      shortcutConfigured: false,
      liveModelConfigured: true,
      requiresVad: true,
      vadConfigured: true,
      shortcutRegistration: 'idle',
      warmup: 'idle',
      inputDeviceState: 'off',
      runtimeState: 'off',
      lastErrorSource: null,
      lastErrorMessage: null,
    },
    asrRuntimeMetrics: {
      modelLoad: null,
      liveInference: null,
      batchInference: null,
    },
    onboardingReady: true,
    punctuationRequired: false,
  };
}

function assertCoreSnapshotHasNoUiSpec(value: unknown) {
  expect(value).not.toHaveProperty('overview');
  expect(value).not.toHaveProperty('sections');
  expect(JSON.stringify(value)).not.toContain('settings.diagnostics.open_model_settings');
  expect(JSON.stringify(value)).not.toContain('settingsTab');
}

function setInvokeMock() {
  mocks.invoke.mockImplementation(async (command: string) => {
    if (command === 'get_diagnostics_core_snapshot') {
      return coreSnapshot;
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
    coreSnapshot = makeCoreSnapshot();
    setInvokeMock();
  });

  it('collects browser-owned facts before calling the Rust diagnostics core', async () => {
    useConfigStore.setState({
      config: {
        ...DEFAULT_CONFIG,
        streamingModelPath: STREAMING_SENSEVOICE_PATH,
        batchModelPath: BATCH_QWEN_PATH,
        vadModelPath: VAD_PATH,
        voiceTypingEnabled: true,
        voiceTypingShortcut: 'Alt+V',
      },
    });
    await diagnosticsService.collectSnapshot(t);

    expect(mocks.invoke).toHaveBeenCalledWith(
      'get_diagnostics_core_snapshot',
      expect.objectContaining({
        input: expect.objectContaining({
          config: expect.objectContaining({
            streamingModelPath: STREAMING_SENSEVOICE_PATH,
            batchModelPath: BATCH_QWEN_PATH,
            vadModelPath: VAD_PATH,
            microphoneId: 'default',
          }),
          permissionState: 'prompt',
          microphoneProbe: expect.objectContaining({ available: true }),
          systemAudioProbe: expect.objectContaining({ available: true }),
          voiceTypingReadiness: expect.objectContaining({
            state: expect.any(String),
            liveModelConfigured: true,
          }),
        }),
      }),
    );
  });

  it('builds diagnostics UI spec in TS from Rust-owned fact fields', async () => {
    assertCoreSnapshotHasNoUiSpec(coreSnapshot);

    const snapshot = await diagnosticsService.collectSnapshot(t);
    const liveOverview = snapshot.overview.find((card) => card.id === 'live-record');
    const inputSection = snapshot.sections.find((section) => section.id === 'input-capture');
    const runtimeSection = snapshot.sections.find((section) => section.id === 'runtime-environment');
    const microphoneCheck = inputSection?.checks.find((check) => check.id === 'microphone-device');
    const logDirCheck = runtimeSection?.checks.find((check) => check.id === 'log-dir');
    const permissionCheck = inputSection?.checks.find((check) => check.id === 'microphone-permission');

    expect(snapshot.scannedAt).toBe('2026-05-03T00:00:00.000Z');
    expect(liveOverview).toEqual(
      expect.objectContaining({
        title: 'Live Record',
        action: expect.objectContaining({
          kind: 'request_microphone_permission',
          label: '请求权限',
        }),
      }),
    );
    expect(microphoneCheck).toEqual(
      expect.objectContaining({
        title: 'Input Device',
        meta: '自动',
      }),
    );
    expect(logDirCheck).toEqual(
      expect.objectContaining({
        title: 'Log Directory',
        meta: 'C:\\app\\logs',
      }),
    );
    expect(permissionCheck?.action).toEqual(
      expect.objectContaining({
        kind: 'request_microphone_permission',
        label: '请求权限',
      }),
    );
  });
});
