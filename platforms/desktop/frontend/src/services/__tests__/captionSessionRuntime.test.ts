import { beforeEach, describe, expect, it, vi } from 'vitest';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { captionSessionRuntime } from '../captionSessionRuntime';
import { captionWindowService } from '../captionWindowService';
import { TauriEvent } from '../tauri/events';

const tauriCoreMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

const tauriEventMocks = vi.hoisted(() => ({
  listen: vi.fn(),
}));
const globalShortcutMocks = vi.hoisted(() => ({
  register: vi.fn().mockResolvedValue(undefined),
  unregister: vi.fn().mockResolvedValue(undefined),
  isRegistered: vi.fn().mockResolvedValue(false),
}));

vi.mock('../tauri/platform/globalShortcut', () => ({
  register: globalShortcutMocks.register,
  unregister: globalShortcutMocks.unregister,
  isRegistered: globalShortcutMocks.isRegistered,
}));

vi.mock('@tauri-apps/plugin-global-shortcut', () => ({
  register: globalShortcutMocks.register,
  unregister: globalShortcutMocks.unregister,
  isRegistered: globalShortcutMocks.isRegistered,
}));

const transcriptionServiceMocks = vi.hoisted(() => ({
  captionStartNative: vi.fn(),
  captionStartExternal: vi.fn(),
  captionStop: vi.fn(),
  captionRestart: vi.fn(),
  captionSendAudioInt16: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriCoreMocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: tauriEventMocks.listen,
}));

vi.mock('../captionWindowService', () => ({
  captionWindowService: {
    open: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    sendSegments: vi.fn().mockResolvedValue(undefined),
    updateStyle: vi.fn().mockResolvedValue(undefined),
    setClickThrough: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../transcriptionService', () => ({
  captionTranscriptionService: {
    startNative: transcriptionServiceMocks.captionStartNative,
    startExternal: transcriptionServiceMocks.captionStartExternal,
    stop: transcriptionServiceMocks.captionStop,
    restartStream: transcriptionServiceMocks.captionRestart,
    sendAudioInt16: transcriptionServiceMocks.captionSendAudioInt16,
  },
}));

vi.stubGlobal(
  'AudioContext',
  class {
    state = 'running';
    destination = {};
    audioWorklet = {
      addModule: vi.fn().mockResolvedValue(undefined),
    };
    createMediaStreamSource() {
      return { connect: vi.fn() };
    }
    close = vi.fn().mockResolvedValue(undefined);
    resume = vi.fn().mockResolvedValue(undefined);
  }
);

vi.stubGlobal(
  'AudioWorkletNode',
  class {
    port = { onmessage: null };
    connect = vi.fn();
  }
);

vi.stubGlobal(
  'MediaStream',
  class {
    tracks: any[];
    constructor(tracks?: any[]) {
      this.tracks = tracks || [];
    }
    getAudioTracks() {
      return this.tracks;
    }
    getVideoTracks() {
      return [];
    }
    getTracks() {
      return this.tracks;
    }
  }
);

describe('captionSessionRuntime', () => {
  const config = buildTestConfig({
    streamingModelPath: '/path/to/model',
    batchModelPath: '/path/to/model',
    language: 'en',
  });

  beforeEach(() => {
    vi.clearAllMocks();
    captionSessionRuntime.resetForTesting();
    transcriptionServiceMocks.captionStartNative.mockResolvedValue(undefined);
    transcriptionServiceMocks.captionStartExternal.mockResolvedValue(undefined);
    transcriptionServiceMocks.captionStop.mockResolvedValue(undefined);
    transcriptionServiceMocks.captionRestart.mockResolvedValue(undefined);
    tauriEventMocks.listen.mockResolvedValue(vi.fn());
    globalShortcutMocks.register.mockClear();
    globalShortcutMocks.unregister.mockClear();
  });

  it('starts and stops native caption capture through the caption service', async () => {
    const unlisten = vi.fn();
    tauriEventMocks.listen.mockImplementation((event: string) => {
      if (event === TauriEvent.audio.systemPeak) {
        return Promise.resolve(unlisten);
      }
      return Promise.resolve(vi.fn());
    });
    await captionSessionRuntime.start(config, () => true, vi.fn());
    await captionSessionRuntime.stop();

    expect(transcriptionServiceMocks.captionStartNative).toHaveBeenCalledWith(
      expect.any(Function),
      expect.any(Function),
      {
        sourceKind: 'system',
        deviceName: null,
        callbackOwner: 'caption',
      }
    );
    expect(captionWindowService.open).toHaveBeenCalledWith(
      expect.objectContaining({
        width: config.captionWindowWidth,
        fontSize: config.captionFontSize,
      })
    );
    expect(transcriptionServiceMocks.captionStop).toHaveBeenCalledTimes(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('updates caption window style through the runtime boundary', async () => {
    await captionSessionRuntime.updateStyle(config);

    expect(captionWindowService.updateStyle).toHaveBeenCalledWith({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
      color: config.captionFontColor,
      backgroundColor: config.captionBackgroundColor,
      backgroundOpacity: config.captionBackgroundOpacity,
      displayMode: config.captionDisplayMode,
      bilingualOrder: config.captionBilingualOrder,
      translationColor: config.captionTranslationFontColor,
      translationFontSize: config.captionTranslationFontSize,
      displayPriority: config.captionDisplayPriority,
      textStroke: config.captionTextStroke,
      strokeWidth: config.captionStrokeWidth,
      strokeColor: config.captionStrokeColor,
      textShadow: config.captionTextShadow,
      textAlign: config.captionTextAlign,
      disappearDelay: config.captionDisappearDelay,
    });
  });

  it('rolls back and cleans up resources if isActive becomes false during start', async () => {
    let callCount = 0;
    // active initially, but becomes false after native capture starts
    const isActive = vi.fn(() => {
      callCount += 1;
      return callCount <= 2;
    });

    const unlisten = vi.fn();
    tauriEventMocks.listen.mockResolvedValue(unlisten);

    await captionSessionRuntime.start(config, isActive, vi.fn());

    expect(transcriptionServiceMocks.captionStop).toHaveBeenCalledTimes(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(captionWindowService.close).toHaveBeenCalledTimes(1);
  });
  it('skips restartRecognizer when the caption session is not running', async () => {
    await captionSessionRuntime.restartRecognizer();
    expect(transcriptionServiceMocks.captionRestart).not.toHaveBeenCalled();
  });

  it('unregisters existing shortcut before registering configured shortcut and cleans up on stop', async () => {
    const lockToggleUnlisten = vi.fn();
    tauriEventMocks.listen.mockImplementation((event: string) => {
      if (event === 'caption:lock-toggle') {
        return Promise.resolve(lockToggleUnlisten);
      }
      return Promise.resolve(vi.fn());
    });

    await captionSessionRuntime.start(config, () => true, vi.fn());

    expect(tauriEventMocks.listen).toHaveBeenCalledWith(
      'caption:lock-toggle',
      expect.any(Function)
    );
    expect(globalShortcutMocks.register).toHaveBeenCalledWith('Alt+C', expect.any(Function));

    // Dynamically update to a different shortcut
    const customConfig = buildTestConfig({
      ...config,
      captionLockShortcut: 'Ctrl+Shift+L',
    });
    await captionSessionRuntime.updateShortcut(customConfig.captionLockShortcut);
    expect(globalShortcutMocks.unregister).toHaveBeenCalledWith('Alt+C');
    expect(globalShortcutMocks.register).toHaveBeenCalledWith('Ctrl+Shift+L', expect.any(Function));

    await captionSessionRuntime.stop();
    expect(globalShortcutMocks.unregister).toHaveBeenCalledWith('Ctrl+Shift+L');
    expect(lockToggleUnlisten).toHaveBeenCalled();
  });
});
