import { invoke } from '@tauri-apps/api/core';
import { logger } from '../utils/logger';

interface AudioDevice {
  name: string;
}

export interface DeviceOption {
  label: string;
  value: string;
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

/**
 * Lists available microphone devices using native APIs first and browser APIs as fallback.
 */
export async function listMicrophoneDeviceOptions(defaultLabel: string): Promise<DeviceOption[]> {
  try {
    const nativeDevices = await invoke<AudioDevice[]>('get_microphone_devices');
    if (nativeDevices && nativeDevices.length > 0) {
      return dedupeOptions([
        { label: defaultLabel, value: 'default' },
        ...nativeDevices.map((device) => ({
          label: device.name,
          value: device.name,
        })),
      ]);
    }
  } catch (error) {
    logger.warn('[AudioDeviceService] Native microphone lookup failed, falling back:', error);
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
  try {
    const devices = await invoke<AudioDevice[]>('get_system_audio_devices');
    if (!devices || devices.length === 0) {
      return [{ label: defaultLabel, value: 'default' }];
    }

    return dedupeOptions([
      { label: defaultLabel, value: 'default' },
      ...devices.map((device) => ({
        label: device.name,
        value: device.name,
      })),
    ]);
  } catch (error) {
    logger.error('[AudioDeviceService] Failed to get system audio devices:', error);
    return [{ label: defaultLabel, value: 'default' }];
  }
}
