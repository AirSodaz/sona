import type { AppConfig } from '../../types/config';
import type { AsrTranscriptionRequest } from '../asrConfigService';
import { resolveAsrTranscriptionRequest } from '../asrConfigService';

export type VoiceTypingShortcutModifier = 'control' | 'alt' | 'shift' | 'meta';

export interface VoiceTypingConfigSnapshot {
  enabled: boolean;
  shortcut: string;
  asrSignature: string;
  vadModelPath: string;
  microphoneId: string;
  language: string;
  enableItn: boolean;
}

export interface VoiceTypingRuntimeChange {
  enabledChanged: boolean;
  shortcutChanged: boolean;
  vadModelChanged: boolean;
  microphoneChanged: boolean;
  asrChanged: boolean;
  languageChanged: boolean;
  enableItnChanged: boolean;
  configChanged: boolean;
  runtimeDependencyChanged: boolean;
}

export function resolveVoiceTypingAsr(config: AppConfig): AsrTranscriptionRequest {
  return resolveAsrTranscriptionRequest(config, 'voiceTyping');
}

export function buildVoiceTypingAsrSignature(asr: AsrTranscriptionRequest): string {
  if (asr.engine === 'local-sherpa') {
    return JSON.stringify({
      engine: asr.engine,
      mode: asr.mode,
      modelPath: asr.modelPath,
    });
  }
  return JSON.stringify({
    engine: asr.engine,
    mode: asr.mode,
    onlineProvider: asr.onlineProvider,
  });
}

export function resolveVoiceTypingConfigSnapshot(config: AppConfig): VoiceTypingConfigSnapshot {
  return {
    enabled: config.voiceTypingEnabled || false,
    shortcut: config.voiceTypingShortcut ?? 'Alt+V',
    asrSignature: buildVoiceTypingAsrSignature(resolveVoiceTypingAsr(config)),
    vadModelPath: config.vadModelPath || '',
    microphoneId: config.microphoneId || 'default',
    language: config.language || 'auto',
    enableItn: config.enableITN ?? true,
  };
}

export function resolveVoiceTypingRuntimeChange(
  previous: VoiceTypingConfigSnapshot,
  next: VoiceTypingConfigSnapshot,
): VoiceTypingRuntimeChange {
  const enabledChanged = next.enabled !== previous.enabled;
  const shortcutChanged = next.shortcut !== previous.shortcut;
  const vadModelChanged = next.vadModelPath !== previous.vadModelPath;
  const microphoneChanged = next.microphoneId !== previous.microphoneId;
  const asrChanged = next.asrSignature !== previous.asrSignature;
  const languageChanged = next.language !== previous.language;
  const enableItnChanged = next.enableItn !== previous.enableItn;

  return {
    enabledChanged,
    shortcutChanged,
    vadModelChanged,
    microphoneChanged,
    asrChanged,
    languageChanged,
    enableItnChanged,
    configChanged:
      asrChanged ||
      vadModelChanged ||
      microphoneChanged ||
      languageChanged ||
      enableItnChanged,
    runtimeDependencyChanged:
      shortcutChanged ||
      asrChanged ||
      vadModelChanged ||
      microphoneChanged,
  };
}

export function getVoiceTypingShortcutModifiers(shortcut: string): VoiceTypingShortcutModifier[] {
  const normalizedParts = shortcut
    .split('+')
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean);
  const modifiers = new Set<VoiceTypingShortcutModifier>();

  for (const part of normalizedParts) {
    if (part === 'ctrl' || part === 'control' || part === 'cmdorctrl') {
      modifiers.add('control');
      continue;
    }

    if (part === 'alt' || part === 'option') {
      modifiers.add('alt');
      continue;
    }

    if (part === 'shift') {
      modifiers.add('shift');
      continue;
    }

    if (
      part === 'meta' ||
      part === 'cmd' ||
      part === 'command' ||
      part === 'super' ||
      part === 'win'
    ) {
      modifiers.add('meta');
    }
  }

  return Array.from(modifiers);
}
