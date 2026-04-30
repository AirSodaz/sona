import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export interface AudioDevice {
  name: string;
}

export interface StartAudioCaptureRequest {
  deviceName: string | null;
  instanceId: string;
  outputPath?: string;
}

export interface SetCapturePausedRequest {
  instanceId: string;
  paused: boolean;
}

export async function setSystemAudioMute(mute: boolean): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.setSystemAudioMute, { mute });
}

export async function getSystemAudioDevices(): Promise<AudioDevice[]> {
  return invokeTauri<AudioDevice[]>(TauriCommand.audio.getSystemAudioDevices);
}

export async function startSystemAudioCapture(request: StartAudioCaptureRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.startSystemAudioCapture, request);
}

export async function stopSystemAudioCapture(instanceId: string): Promise<string> {
  return invokeTauri<string>(TauriCommand.audio.stopSystemAudioCapture, { instanceId });
}

export async function setSystemAudioCapturePaused(request: SetCapturePausedRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.setSystemAudioCapturePaused, request);
}

export async function setMicrophoneBoost(boost: number): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.setMicrophoneBoost, { boost });
}

export async function getMicrophoneDevices(): Promise<AudioDevice[]> {
  return invokeTauri<AudioDevice[]>(TauriCommand.audio.getMicrophoneDevices);
}

export async function startMicrophoneCapture(request: StartAudioCaptureRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.startMicrophoneCapture, request);
}

export async function stopMicrophoneCapture(instanceId: string): Promise<string> {
  return invokeTauri<string>(TauriCommand.audio.stopMicrophoneCapture, { instanceId });
}

export async function setMicrophoneCapturePaused(request: SetCapturePausedRequest): Promise<void> {
  await invokeTauri<void>(TauriCommand.audio.setMicrophoneCapturePaused, request);
}
