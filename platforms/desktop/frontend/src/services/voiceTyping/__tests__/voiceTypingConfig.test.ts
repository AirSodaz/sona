import { describe, expect, it } from 'vitest';
import {
  buildVoiceTypingAsrSignature,
  getVoiceTypingShortcutModifiers,
  resolveVoiceTypingConfigSnapshot,
  resolveVoiceTypingRuntimeChange,
} from '../voiceTypingConfig';
import { buildTestConfig } from '../../../test-utils/configTestUtils';
import { resolveAsrTranscriptionRequest } from '../../asrConfigService';
import type { AppConfig } from '../../../types/config';

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

    }));
  });

  it('detects shortcut, ASR, VAD, microphone, language, and ITN changes separately', () => {
    const previous = resolveVoiceTypingConfigSnapshot(buildTestConfig({
      voiceTypingEnabled: true,
      voiceTypingShortcut: 'Alt+V',
      vadModelPath: '/models/vad',
      microphoneId: 'default',
      keepMicrophoneActive: true,
      language: 'auto',
      enableITN: true,
    }));
    const next = resolveVoiceTypingConfigSnapshot(buildTestConfig({
      voiceTypingEnabled: true,
      voiceTypingShortcut: 'Ctrl+Space',
      streamingModelPath: '/models/live-next',
      vadModelPath: '/models/vad-next',
      microphoneId: 'usb',
      keepMicrophoneActive: false,
      language: 'zh',
      enableITN: false,
    }));

    expect(resolveVoiceTypingRuntimeChange(previous, next)).toEqual({
      enabledChanged: false,
      shortcutChanged: true,
      vadModelChanged: true,
      microphoneChanged: true,
      keepMicrophoneActiveChanged: true,
      asrChanged: true,
      languageChanged: true,
      enableItnChanged: true,
      configChanged: true,
      runtimeDependencyChanged: true,
    });
  });

  it('includes the global microphone persistence preference in the runtime snapshot', () => {
    const snapshot = resolveVoiceTypingConfigSnapshot(buildTestConfig({
      keepMicrophoneActive: false,
    }));

    expect(snapshot.keepMicrophoneActive).toBe(false);
  });

  it('defaults the global microphone persistence preference to false when omitted', () => {
    const config = buildTestConfig({
      keepMicrophoneActive: true,
    }) as AppConfig;
    delete (config as Partial<AppConfig>).keepMicrophoneActive;

    const snapshot = resolveVoiceTypingConfigSnapshot(config);

    expect(snapshot.keepMicrophoneActive).toBe(false);
  });

  it('parses shortcut modifiers in injection order', () => {
    expect(getVoiceTypingShortcutModifiers('Ctrl + Shift + V')).toEqual(['control', 'shift']);
    expect(getVoiceTypingShortcutModifiers('CmdOrCtrl + Option + Space')).toEqual(['control', 'alt']);
    expect(getVoiceTypingShortcutModifiers('Win + Alt + V')).toEqual(['meta', 'alt']);
    expect(getVoiceTypingShortcutModifiers('Space')).toEqual([]);
  });
});
