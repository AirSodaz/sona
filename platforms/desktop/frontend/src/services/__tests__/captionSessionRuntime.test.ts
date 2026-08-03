import { beforeEach, describe, expect, it, vi } from 'vitest';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { captionSessionRuntime } from '../captionSessionRuntime';
import { captionWindowService } from '../captionWindowService';

const tauriCoreMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

const tauriEventMocks = vi.hoisted(() => ({
  listen: vi.fn(),
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

vi.stubGlobal('AudioContext', class {
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
});

vi.stubGlobal('AudioWorkletNode', class {
  port = { onmessage: null };
  connect = vi.fn();
});

vi.stubGlobal('MediaStream', class {
  tracks: any[];
  constructor(tracks?: any[]) {
    this.tracks = tracks || [];
  }
  getAudioTracks() { return this.tracks; }
  getVideoTracks() { return []; }
  getTracks() { return this.tracks; }
});

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
  });

  it('starts and stops native caption capture through the caption service', async () => {
    const unlisten = vi.fn();
    tauriEventMocks.listen.mockResolvedValue(unlisten);
    await captionSessionRuntime.start(config, () => true, vi.fn());
    await captionSessionRuntime.stop();

    expect(transcriptionServiceMocks.captionStartNative).toHaveBeenCalledWith(
      expect.any(Function),
      expect.any(Function),
      {
        sourceKind: 'system',
        deviceName: null,
        callbackOwner: 'caption',
      },
    );
    expect(captionWindowService.open).toHaveBeenCalledWith(expect.objectContaining({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
    }));
    expect(transcriptionServiceMocks.captionStop).toHaveBeenCalledTimes(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('updates caption window style through the runtime boundary', async () => {
    await captionSessionRuntime.updateStyle(config);

    expect(captionWindowService.updateStyle).toHaveBeenCalledWith({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
      color: config.captionFontColor,
      backgroundOpacity: config.captionBackgroundOpacity,
    });
  });
});
