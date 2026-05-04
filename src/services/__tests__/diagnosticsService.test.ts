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
import type { DiagnosticsCoreSnapshotSpec } from '../diagnosticsSnapshotBuilders';

const STREAMING_SENSEVOICE_PATH = 'C:\\models\\sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const OFFLINE_QWEN_PATH = 'C:\\models\\sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25';
const VAD_PATH = 'C:\\models\\silero_vad.onnx';

const runtimeEnvironment = {
  ffmpegPath: 'C:\\app\\ffmpeg.exe',
  ffmpegExists: true,
  logDirPath: 'C:\\app\\logs',
};
let coreSnapshot: DiagnosticsCoreSnapshotSpec;

function t(key: string, options?: Record<string, unknown>) {
  const translated: Record<string, string> = {
    'settings.diagnostics.open_model_settings': '打开模型设置',
    'settings.diagnostics.request_permission': '请求权限',
    'settings.mic_auto': '自动',
  };

  return translated[key] ?? (options?.defaultValue as string | undefined) ?? key;
}

function text(key: string, defaultValue: string, params?: Record<string, unknown>) {
  return { key, defaultValue, params };
}

function makeCoreSnapshot(): DiagnosticsCoreSnapshotSpec {
  return {
    scannedAt: '2026-05-03T00:00:00.000Z',
    runtimeEnvironment,
    overview: [
      {
        id: 'live-record',
        title: text('settings.diagnostics.live_record_card', 'Live Record'),
        description: text(
          'settings.diagnostics.live_record_card_description',
          'Model, VAD, permission, and microphone selection for real-time capture.',
        ),
        status: 'missing',
        action: {
          kind: 'open_settings',
          label: text('settings.diagnostics.open_model_settings', 'Open Model Settings'),
          settingsTab: 'models',
        },
      },
    ],
    sections: [
      {
        id: 'input-capture',
        title: text('settings.diagnostics.input_section', 'Input & Capture'),
        description: text(
          'settings.diagnostics.input_section_description',
          'Check permissions and the availability of input or capture devices.',
        ),
        checks: [
          {
            id: 'microphone-device',
            title: text('settings.diagnostics.microphone_title', 'Input Device'),
            description: text(
              'settings.diagnostics.microphone_ready',
              'The current input-device selection is still available.',
            ),
            status: 'ready',
            meta: text('settings.mic_auto', 'Auto'),
          },
          {
            id: 'microphone-permission',
            title: text('settings.diagnostics.permission_title', 'Microphone Permission'),
            description: text(
              'settings.diagnostics.permission_prompt',
              'Microphone access has not been granted yet.',
            ),
            status: 'warning',
            action: {
              kind: 'request_microphone_permission',
              label: text('settings.diagnostics.request_permission', 'Request Permission'),
            },
          },
        ],
      },
      {
        id: 'runtime-environment',
        title: text('settings.diagnostics.runtime_section', 'Runtime Environment'),
        description: text(
          'settings.diagnostics.runtime_section_description',
          'Check bundled runtime dependencies and troubleshooting paths.',
        ),
        checks: [
          {
            id: 'log-dir',
            title: text('settings.diagnostics.log_dir_title', 'Log Directory'),
            description: text(
              'settings.diagnostics.log_dir_ready',
              'Runtime logs can be resolved for troubleshooting.',
            ),
            status: 'ready',
            meta: text('diagnostics.literal_meta', 'C:\\app\\logs'),
          },
        ],
      },
    ],
  };
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
        offlineModelPath: OFFLINE_QWEN_PATH,
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
            offlineModelPath: OFFLINE_QWEN_PATH,
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

  it('hydrates Rust text specs and actions into the existing diagnostics snapshot shape', async () => {
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
          kind: 'open_settings',
          label: '打开模型设置',
          settingsTab: 'models',
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
