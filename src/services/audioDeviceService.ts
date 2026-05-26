import { logger } from '../utils/logger';
import { extractErrorMessage } from '../utils/errorUtils';
import {
  getMicrophoneDevices,
  getSystemAudioDevices,
  type AudioDevice,
} from './tauri/audio';

export interface DeviceOption {
  label: string;
  value: string;
}

export type MicrophonePermissionState = PermissionState | 'unsupported';

export interface DeviceProbeResult {
  options: DeviceOption[];
  available: boolean;
  source: 'native' | 'browser' | 'fallback';
  errorMessage?: string;
}

const DEFAULT_DEVICE_VALUE = 'default';
const MICROPHONE_ENUMERATION_UNAVAILABLE_MESSAGE = 'Browser device enumeration is unavailable.';
const NO_MICROPHONE_DEVICES_MESSAGE = 'No microphone devices were returned.';
const NO_SYSTEM_AUDIO_DEVICES_MESSAGE = 'No system audio devices were returned.';

function createDefaultDeviceOption(defaultLabel: string): DeviceOption {
  return {
    label: defaultLabel,
    value: DEFAULT_DEVICE_VALUE,
  };
}

function createFallbackProbeResult(
  defaultLabel: string,
  errorMessage: string,
): DeviceProbeResult {
  return {
    options: [createDefaultDeviceOption(defaultLabel)],
    available: false,
    source: 'fallback',
    errorMessage,
  };
}

function dedupeOptions(options: DeviceOption[]): DeviceOption[] {
  return options.filter((option, index, currentOptions) => (
    index === currentOptions.findIndex((candidate) => candidate.value === option.value)
  ));
}

function mapDeviceOptions<TDevice>(
  defaultLabel: string,
  devices: TDevice[],
  toOption: (device: TDevice) => DeviceOption,
): DeviceOption[] {
  return dedupeOptions([
    createDefaultDeviceOption(defaultLabel),
    ...devices.map(toOption),
  ]);
}

function toBrowserDeviceOption(device: MediaDeviceInfo): DeviceOption {
  return {
    label: device.label || `Microphone ${device.deviceId.slice(0, 5)}...`,
    value: device.deviceId,
  };
}

function toNativeDeviceOption(device: AudioDevice): DeviceOption {
  return {
    label: device.name,
    value: device.name,
  };
}

function getAudioInputDevices(devices: MediaDeviceInfo[]): MediaDeviceInfo[] {
  return devices.filter((device) => device.kind === 'audioinput');
}

function hasVisibleDeviceLabels(devices: MediaDeviceInfo[]): boolean {
  return devices.some((device) => device.label);
}

function toBrowserDeviceOptions(
  audioInputDevices: MediaDeviceInfo[],
  defaultLabel: string,
): DeviceOption[] {
  return mapDeviceOptions(defaultLabel, audioInputDevices, toBrowserDeviceOption);
}

function toNativeDeviceOptions(devices: AudioDevice[], defaultLabel: string): DeviceOption[] {
  return mapDeviceOptions(defaultLabel, devices, toNativeDeviceOption);
}

export async function getMicrophonePermissionState(): Promise<MicrophonePermissionState> {
  if (!navigator.mediaDevices?.getUserMedia) {
    return 'unsupported';
  }

  if (!navigator.permissions?.query) {
    return 'prompt';
  }

  try {
    const status = await navigator.permissions.query({ name: 'microphone' as PermissionName });
    if (status.state === 'granted' || status.state === 'denied' || status.state === 'prompt') {
      return status.state;
    }
  } catch (error) {
    logger.debug?.('[AudioDeviceService] Passive microphone permission query failed:', error);
  }

  return 'prompt';
}

/**
 * Requests browser-level microphone permission for onboarding and settings.
 */
export async function requestMicrophonePermission(): Promise<boolean> {
  if (!navigator.mediaDevices?.getUserMedia) {
    return false;
  }

  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    stream.getTracks().forEach((track) => track.stop());
    return true;
  } catch (error) {
    logger.warn('[AudioDeviceService] Microphone permission denied:', error);
    return false;
  }
}

export async function probeMicrophoneDeviceOptions(defaultLabel: string): Promise<DeviceProbeResult> {
  try {
    const nativeDevices = await getMicrophoneDevices();
    if (nativeDevices.length > 0) {
      return {
        options: toNativeDeviceOptions(nativeDevices, defaultLabel),
        available: true,
        source: 'native',
      };
    }
  } catch (error) {
    logger.warn('[AudioDeviceService] Native microphone lookup failed, falling back:', error);
  }

  if (!navigator.mediaDevices?.enumerateDevices) {
    return createFallbackProbeResult(defaultLabel, MICROPHONE_ENUMERATION_UNAVAILABLE_MESSAGE);
  }

  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const audioInputs = getAudioInputDevices(devices);
    return {
      options: toBrowserDeviceOptions(audioInputs, defaultLabel),
      available: audioInputs.length > 0,
      source: 'browser',
      errorMessage: audioInputs.length > 0 ? undefined : NO_MICROPHONE_DEVICES_MESSAGE,
    };
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to enumerate browser microphones:', error);
    return createFallbackProbeResult(defaultLabel, extractErrorMessage(error));
  }
}

/**
 * Lists available microphone devices using native APIs first and browser APIs as fallback.
 */
export async function listMicrophoneDeviceOptions(defaultLabel: string): Promise<DeviceOption[]> {
  const probe = await probeMicrophoneDeviceOptions(defaultLabel);
  if (probe.source === 'native') {
    return probe.options;
  }

  if (!navigator.mediaDevices?.enumerateDevices) {
    return [createDefaultDeviceOption(defaultLabel)];
  }

  try {
    let devices = await navigator.mediaDevices.enumerateDevices();
    let audioInputs = getAudioInputDevices(devices);

    if (!hasVisibleDeviceLabels(audioInputs)) {
      await requestMicrophonePermission();
      devices = await navigator.mediaDevices.enumerateDevices();
      audioInputs = getAudioInputDevices(devices);
    }

    return toBrowserDeviceOptions(audioInputs, defaultLabel);
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to enumerate browser microphones:', error);
    return [createDefaultDeviceOption(defaultLabel)];
  }
}

/**
 * Lists available system-audio capture devices for the settings screen.
 */
export async function listSystemAudioDeviceOptions(defaultLabel: string): Promise<DeviceOption[]> {
  const probe = await probeSystemAudioDeviceOptions(defaultLabel);
  return probe.options;
}

export async function probeSystemAudioDeviceOptions(defaultLabel: string): Promise<DeviceProbeResult> {
  try {
    const devices = await getSystemAudioDevices();
    if (devices.length === 0) {
      return createFallbackProbeResult(defaultLabel, NO_SYSTEM_AUDIO_DEVICES_MESSAGE);
    }

    return {
      options: toNativeDeviceOptions(devices, defaultLabel),
      available: true,
      source: 'native',
    };
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to get system audio devices:', error);
    return createFallbackProbeResult(defaultLabel, extractErrorMessage(error));
  }
}
