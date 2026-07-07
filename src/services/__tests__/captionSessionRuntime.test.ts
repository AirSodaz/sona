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

const tauriFsMocks = vi.hoisted(() => ({
  remove: vi.fn(),
}));

const transcriptionServiceMocks = vi.hoisted(() => ({
  captionStart: vi.fn(),
  captionStop: vi.fn(),
  captionSendAudioInt16: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriCoreMocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: tauriEventMocks.listen,
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  remove: tauriFsMocks.remove,
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
    start: transcriptionServiceMocks.captionStart,
    stop: transcriptionServiceMocks.captionStop,
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
    transcriptionServiceMocks.captionStart.mockResolvedValue(undefined);
    transcriptionServiceMocks.captionStop.mockResolvedValue(undefined);
    tauriEventMocks.listen.mockResolvedValue(vi.fn());
    tauriFsMocks.remove.mockResolvedValue(undefined);
  });

  it('starts and stops native caption capture through the caption service', async () => {
    const unlisten = vi.fn();
    tauriEventMocks.listen.mockResolvedValue(unlisten);
    tauriCoreMocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'start_system_audio_capture') {
        return undefined;
      }

      if (command === 'stop_system_audio_capture') {
        return 'C:/tmp/caption.wav';
      }

      throw new Error(`Unexpected native command: ${command}`);
    });

    await captionSessionRuntime.start(config, () => true, vi.fn());
    await captionSessionRuntime.stop();

    expect(transcriptionServiceMocks.captionStart).toHaveBeenCalledTimes(1);
    expect(captionWindowService.open).toHaveBeenCalledWith(expect.objectContaining({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
    }));
    expect(tauriCoreMocks.invoke).toHaveBeenCalledWith('start_system_audio_capture', {
      deviceName: null,
      instanceId: 'caption',
    });
    expect(transcriptionServiceMocks.captionStop).toHaveBeenCalledTimes(1);
    expect(tauriCoreMocks.invoke).toHaveBeenCalledWith('stop_system_audio_capture', {
      instanceId: 'caption',
    });
    expect(tauriFsMocks.remove).toHaveBeenCalledWith('C:/tmp/caption.wav');
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
