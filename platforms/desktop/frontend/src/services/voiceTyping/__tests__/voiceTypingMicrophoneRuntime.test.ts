import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  startMicrophoneCapture: vi.fn(async () => undefined),
  stopMicrophoneCapture: vi.fn(async () => undefined),
  loggerInfo: vi.fn(),
  loggerError: vi.fn(),
}));

vi.mock('../../tauri/audio', () => ({
  startMicrophoneCapture: mocks.startMicrophoneCapture,
  stopMicrophoneCapture: mocks.stopMicrophoneCapture,
}));

vi.mock('../../../utils/logger', () => ({
  logger: {
    info: mocks.loggerInfo,
    error: mocks.loggerError,
  },
}));

async function loadRuntime() {
  const runtimeModule = await import('../voiceTypingMicrophoneRuntime');
  const storeModule = await import('../../../stores/voiceTypingRuntimeStore');
  storeModule.useVoiceTypingRuntimeStore.getState().resetRuntimeStatus();
  return {
    VoiceTypingMicrophoneRuntime: runtimeModule.VoiceTypingMicrophoneRuntime,
    useVoiceTypingRuntimeStore: storeModule.useVoiceTypingRuntimeStore,
  };
}

describe('VoiceTypingMicrophoneRuntime', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('starts persistent voice-typing microphone capture once for the configured device', async () => {
    const { VoiceTypingMicrophoneRuntime } = await loadRuntime();
    const runtime = new VoiceTypingMicrophoneRuntime();

    await runtime.ensureStarted('usb-mic');
    await runtime.ensureStarted('usb-mic');

    expect(mocks.startMicrophoneCapture).toHaveBeenCalledTimes(1);
    expect(mocks.startMicrophoneCapture).toHaveBeenCalledWith({
      deviceName: 'usb-mic',
      instanceId: 'voice-typing',
    });
  });

  it('normalizes default microphone to null and stops only after capture started', async () => {
    const { VoiceTypingMicrophoneRuntime } = await loadRuntime();
    const runtime = new VoiceTypingMicrophoneRuntime();

    await runtime.stop();
    await runtime.ensureStarted('default');
    await runtime.stop();
    await runtime.stop();

    expect(mocks.startMicrophoneCapture).toHaveBeenCalledWith({
      deviceName: null,
      instanceId: 'voice-typing',
    });
    expect(mocks.stopMicrophoneCapture).toHaveBeenCalledTimes(1);
    expect(mocks.stopMicrophoneCapture).toHaveBeenCalledWith('voice-typing');
  });

  it('reports microphone warm-up failures to runtime status', async () => {
    mocks.startMicrophoneCapture.mockRejectedValueOnce(new Error('Microphone denied.'));
    const { VoiceTypingMicrophoneRuntime, useVoiceTypingRuntimeStore } = await loadRuntime();
    const runtime = new VoiceTypingMicrophoneRuntime();

    await runtime.ensureStarted('default');

    expect(useVoiceTypingRuntimeStore.getState()).toEqual(
      expect.objectContaining({
        warmup: 'error',
        lastErrorSource: 'microphone',
        lastErrorMessage: 'Microphone denied.',
      }),
    );
  });
});
