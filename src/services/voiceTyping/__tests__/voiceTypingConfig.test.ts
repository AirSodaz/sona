import { describe, expect, it } from 'vitest';
import {
  buildVoiceTypingAsrSignature,
  getVoiceTypingShortcutModifiers,
  resolveVoiceTypingConfigSnapshot,
  resolveVoiceTypingRuntimeChange,
} from '../voiceTypingConfig';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import { resolveAsrTranscriptionRequest } from '../../asrConfigService';

describe('voiceTypingConfig', () => {
  it('builds a stable signature from the ASR fields that affect runtime warm-up', () => {
    const config = buildTestConfig({
      streamingModelPath: '/models/live',
      asr: {
        selections: {
          voiceTyping: {
            engine: 'local-sherpa',
            mode: 'streaming',
            modelId: 'live-model',
            modelPath: '/models/live',
          },
        },
      },
    });
    const asr = resolveAsrTranscriptionRequest(config, 'voiceTyping');

    expect(buildVoiceTypingAsrSignature(asr)).toBe(JSON.stringify({
      engine: 'local-sherpa',
      mode: 'streaming',
      modelPath: '/models/live',
      providerId: null,
      profileId: null,
      onlineProvider: undefined,
    }));
  });

  it('detects shortcut, ASR, VAD, microphone, language, and ITN changes separately', () => {
    const previous = resolveVoiceTypingConfigSnapshot(buildTestConfig({
      voiceTypingEnabled: true,
      voiceTypingShortcut: 'Alt+V',
      vadModelPath: '/models/vad',
      microphoneId: 'default',
      language: 'auto',
      enableITN: true,
    }));
    const next = resolveVoiceTypingConfigSnapshot(buildTestConfig({
      voiceTypingEnabled: true,
      voiceTypingShortcut: 'Ctrl+Space',
      streamingModelPath: '/models/live-next',
      vadModelPath: '/models/vad-next',
      microphoneId: 'usb',
      language: 'zh',
      enableITN: false,
    }));

    expect(resolveVoiceTypingRuntimeChange(previous, next)).toEqual({
      enabledChanged: false,
      shortcutChanged: true,
      vadModelChanged: true,
      microphoneChanged: true,
      asrChanged: true,
      languageChanged: true,
      enableItnChanged: true,
      configChanged: true,
      runtimeDependencyChanged: true,
    });
  });

  it('parses shortcut modifiers in injection order', () => {
    expect(getVoiceTypingShortcutModifiers('Ctrl + Shift + V')).toEqual(['control', 'shift']);
    expect(getVoiceTypingShortcutModifiers('CmdOrCtrl + Option + Space')).toEqual(['control', 'alt']);
    expect(getVoiceTypingShortcutModifiers('Win + Alt + V')).toEqual(['meta', 'alt']);
    expect(getVoiceTypingShortcutModifiers('Space')).toEqual([]);
  });
});
