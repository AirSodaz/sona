import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  listMicrophoneDeviceOptions,
  probeMicrophoneDeviceOptions,
  probeSystemAudioDeviceOptions,
} from '../audioDeviceService';
import {
  getMicrophoneDevices,
  getSystemAudioDevices,
} from '../tauri/audio';

const tauriAudioMocks = vi.hoisted(() => ({
  getMicrophoneDevices: vi.fn(),
  getSystemAudioDevices: vi.fn(),
}));

vi.mock('../tauri/audio', () => ({
  getMicrophoneDevices: tauriAudioMocks.getMicrophoneDevices,
  getSystemAudioDevices: tauriAudioMocks.getSystemAudioDevices,
}));

function setMediaDevices(mediaDevices: Partial<MediaDevices>): void {
  Object.defineProperty(navigator, 'mediaDevices', {
    configurable: true,
    value: mediaDevices,
  });
}

function createMediaDevice(
  kind: MediaDeviceKind,
  deviceId: string,
  label: string,
): MediaDeviceInfo {
  return {
    deviceId,
    groupId: '',
    kind,
    label,
    toJSON: () => ({}),
  };
}

describe('audioDeviceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setMediaDevices({});
  });

  it('returns deduped native microphone options when native devices are available', async () => {
    vi.mocked(getMicrophoneDevices).mockResolvedValue([
      { name: 'Desk Mic' },
      { name: 'Desk Mic' },
    ]);

    const probe = await probeMicrophoneDeviceOptions('Auto');

    expect(probe).toEqual({
      options: [
        { label: 'Auto', value: 'default' },
        { label: 'Desk Mic', value: 'Desk Mic' },
      ],
      available: true,
      source: 'native',
    });
  });

  it('falls back to browser microphone enumeration when native lookup fails', async () => {
    vi.mocked(getMicrophoneDevices).mockRejectedValue(new Error('native unavailable'));
    const enumerateDevices = vi.fn().mockResolvedValue([
      createMediaDevice('audioinput', 'browser-mic', 'Browser Mic'),
      createMediaDevice('audiooutput', 'speaker', 'Speaker'),
    ]);
    setMediaDevices({ enumerateDevices });

    const probe = await probeMicrophoneDeviceOptions('Auto');

    expect(probe).toEqual({
      options: [
        { label: 'Auto', value: 'default' },
        { label: 'Browser Mic', value: 'browser-mic' },
      ],
      available: true,
      source: 'browser',
      errorMessage: undefined,
    });
  });

  it('returns the microphone fallback result when browser enumeration is unavailable', async () => {
    vi.mocked(getMicrophoneDevices).mockRejectedValue(new Error('native unavailable'));

    const probe = await probeMicrophoneDeviceOptions('Auto');

    expect(probe).toEqual({
      options: [{ label: 'Auto', value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: 'Browser device enumeration is unavailable.',
    });
  });

  it('requests permission and re-enumerates unlabeled browser microphones', async () => {
    vi.mocked(getMicrophoneDevices).mockRejectedValue(new Error('native unavailable'));
    const stop = vi.fn();
    const enumerateDevices = vi
      .fn()
      .mockResolvedValueOnce([
        createMediaDevice('audioinput', 'desk-mic', ''),
      ])
      .mockResolvedValueOnce([
        createMediaDevice('audioinput', 'desk-mic', ''),
      ])
      .mockResolvedValueOnce([
        createMediaDevice('audioinput', 'desk-mic', 'Desk Mic'),
      ]);
    const getUserMedia = vi.fn().mockResolvedValue({
      getTracks: () => [{ stop }],
    });
    setMediaDevices({ enumerateDevices, getUserMedia });

    const options = await listMicrophoneDeviceOptions('Auto');

    expect(getUserMedia).toHaveBeenCalledTimes(1);
    expect(stop).toHaveBeenCalledTimes(1);
    expect(enumerateDevices).toHaveBeenCalledTimes(3);
    expect(options).toEqual([
      { label: 'Auto', value: 'default' },
      { label: 'Desk Mic', value: 'desk-mic' },
    ]);
  });

  it('returns the system-audio fallback result when native devices are empty', async () => {
    vi.mocked(getSystemAudioDevices).mockResolvedValue([]);

    const probe = await probeSystemAudioDeviceOptions('Auto');

    expect(probe).toEqual({
      options: [{ label: 'Auto', value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: 'No system audio devices were returned.',
    });
  });

  it('returns the system-audio error fallback result when native lookup fails', async () => {
    vi.mocked(getSystemAudioDevices).mockRejectedValue(new Error('backend unavailable'));

    const probe = await probeSystemAudioDeviceOptions('Auto');

    expect(probe).toEqual({
      options: [{ label: 'Auto', value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: 'backend unavailable',
    });
  });
});
