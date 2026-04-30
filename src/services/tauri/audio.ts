import { TauriCommand } from './commands';
import type { TauriCommandArgs, TauriCommandResult } from './contracts';
import { invokeTauri } from './invoke';

export type AudioDevice =
  TauriCommandResult<typeof TauriCommand.audio.getSystemAudioDevices>[number];

export type StartAudioCaptureRequest =
  TauriCommandArgs<typeof TauriCommand.audio.startSystemAudioCapture>;

export type SetCapturePausedRequest =
  TauriCommandArgs<typeof TauriCommand.audio.setSystemAudioCapturePaused>;

export async function setSystemAudioMute(mute: boolean): Promise<void> {
  await invokeTauri(TauriCommand.audio.setSystemAudioMute, { mute });
}

export async function getSystemAudioDevices(): Promise<AudioDevice[]> {
  return invokeTauri(TauriCommand.audio.getSystemAudioDevices);
}

export async function startSystemAudioCapture(request: StartAudioCaptureRequest): Promise<void> {
  await invokeTauri(TauriCommand.audio.startSystemAudioCapture, request);
}

export async function stopSystemAudioCapture(instanceId: string): Promise<string> {
  return invokeTauri(TauriCommand.audio.stopSystemAudioCapture, { instanceId });
}

export async function setSystemAudioCapturePaused(request: SetCapturePausedRequest): Promise<void> {
  await invokeTauri(TauriCommand.audio.setSystemAudioCapturePaused, request);
}

export async function setMicrophoneBoost(boost: number): Promise<void> {
  await invokeTauri(TauriCommand.audio.setMicrophoneBoost, { boost });
}

export async function getMicrophoneDevices(): Promise<AudioDevice[]> {
  return invokeTauri(TauriCommand.audio.getMicrophoneDevices);
}

export async function startMicrophoneCapture(request: StartAudioCaptureRequest): Promise<void> {
  await invokeTauri(TauriCommand.audio.startMicrophoneCapture, request);
}

export async function stopMicrophoneCapture(instanceId: string): Promise<string> {
  return invokeTauri(TauriCommand.audio.stopMicrophoneCapture, { instanceId });
}

export async function setMicrophoneCapturePaused(request: SetCapturePausedRequest): Promise<void> {
  await invokeTauri(TauriCommand.audio.setMicrophoneCapturePaused, request);
}
