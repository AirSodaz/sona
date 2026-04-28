import { invoke } from '@tauri-apps/api/core';
import { logger } from '../utils/logger';

interface AudioDevice {
  name: string;
}

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

function dedupeOptions(options: DeviceOption[]): DeviceOption[] {
  return options.filter((option, index, currentOptions) => (
    index === currentOptions.findIndex((candidate) => candidate.value === option.value)
  ));
}

function toBrowserDeviceOptions(
  devices: MediaDeviceInfo[],
  defaultLabel: string,
): DeviceOption[] {
  return dedupeOptions([
    { label: defaultLabel, value: 'default' },
    ...devices.map((device) => ({
      label: device.label || `Microphone ${device.deviceId.slice(0, 5)}...`,
      value: device.deviceId,
    })),
  ]);
}

function toNativeDeviceOptions(devices: AudioDevice[], defaultLabel: string): DeviceOption[] {
  return dedupeOptions([
    { label: defaultLabel, value: 'default' },
    ...devices.map((device) => ({
      label: device.name,
      value: device.name,
    })),
  ]);
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
    const nativeDevices = await invoke<AudioDevice[]>('get_microphone_devices');
    if (nativeDevices && nativeDevices.length > 0) {
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
    return {
      options: [{ label: defaultLabel, value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: 'Browser device enumeration is unavailable.',
    };
  }

  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const audioInputs = devices.filter((device) => device.kind === 'audioinput');
    return {
      options: toBrowserDeviceOptions(audioInputs, defaultLabel),
      available: audioInputs.length > 0,
      source: 'browser',
      errorMessage: audioInputs.length > 0 ? undefined : 'No microphone devices were returned.',
    };
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to enumerate browser microphones:', error);
    return {
      options: [{ label: defaultLabel, value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: error instanceof Error ? error.message : String(error),
    };
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
    return [{ label: defaultLabel, value: 'default' }];
  }

  try {
    let devices = await navigator.mediaDevices.enumerateDevices();
    let audioInputs = devices.filter((device) => device.kind === 'audioinput');

    if (!audioInputs.some((device) => device.label)) {
      await requestMicrophonePermission();
      devices = await navigator.mediaDevices.enumerateDevices();
      audioInputs = devices.filter((device) => device.kind === 'audioinput');
    }

    return toBrowserDeviceOptions(audioInputs, defaultLabel);
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to enumerate browser microphones:', error);
    return [{ label: defaultLabel, value: 'default' }];
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
    const devices = await invoke<AudioDevice[]>('get_system_audio_devices');
    if (!devices || devices.length === 0) {
      return {
        options: [{ label: defaultLabel, value: 'default' }],
        available: false,
        source: 'fallback',
        errorMessage: 'No system audio devices were returned.',
      };
    }

    return {
      options: dedupeOptions([
        { label: defaultLabel, value: 'default' },
        ...devices.map((device) => ({
          label: device.name,
          value: device.name,
        })),
      ]),
      available: true,
      source: 'native',
    };
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to get system audio devices:', error);
    return {
      options: [{ label: defaultLabel, value: 'default' }],
      available: false,
      source: 'fallback',
      errorMessage: error instanceof Error ? error.message : String(error),
    };
  }
}
