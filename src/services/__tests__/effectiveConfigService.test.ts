import { describe, it, expect } from 'vitest';
import { resolveEffectiveConfig } from '../effectiveConfigService';
import type { AppConfig } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import { createLlmSettings } from '../llm/state';

function createBaseConfig(): AppConfig {
  return {
    configVersion: 5,
    appLanguage: 'en',
    theme: 'light',
    font: 'system',
    minimizeToTrayOnExit: true,
    autoCheckUpdates: true,
    liveRecordShortcut: 'Ctrl + Space',
    microphoneId: 'default',
    systemAudioDeviceId: 'default',
    muteDuringRecording: false,
    streamingModelPath: '/models/live',
    offlineModelPath: '/models/offline',
    punctuationModelPath: '',
    vadModelPath: '',
    language: 'auto',
    enableTimeline: false,
    enableITN: true,
    vadBufferSize: 5,
    maxConcurrent: 2,
    llmSettings: createLlmSettings(),
    summaryEnabled: true,
    summaryTemplateId: 'general',
    summaryCustomTemplates: [],
    translationLanguage: 'zh',
    polishPresetId: 'general',
    polishCustomPresets: [
      { id: 'custom-team', name: 'Team', context: 'Team sync notes' },
    ],
    polishKeywordSets: [
      { id: 'kw-1', name: 'Brand', enabled: true, keywords: 'Sona\nSherpa-onnx' },
      { id: 'kw-2', name: 'Style', enabled: false, keywords: 'Keep sentence case.' },
    ],
    speakerProfiles: [
      { id: 'speaker-a', name: 'Alice', enabled: true, samples: [] },
      { id: 'speaker-b', name: 'Bob', enabled: false, samples: [] },
    ],
    autoPolish: false,
    autoPolishFrequency: 5,
    voiceTypingEnabled: false,
    voiceTypingShortcut: 'Alt+V',
    voiceTypingMode: 'hold',
    textReplacementSets: [
      { id: 'set-a', name: 'A', enabled: true, ignoreCase: false, rules: [] },
      { id: 'set-b', name: 'B', enabled: true, ignoreCase: false, rules: [] },
    ],
    hotwordSets: [
      { id: 'hot-a', name: 'Hot A', enabled: true, rules: [] },
      { id: 'hot-b', name: 'Hot B', enabled: false, rules: [] },
    ],
    hotwords: [],
  };
}

function createProject(): ProjectRecord {
  return {
    id: 'project-1',
    name: 'Project One',
    description: '',
    createdAt: Date.now(),
    updatedAt: Date.now(),
    defaults: {
      summaryTemplateId: 'meeting',
      translationLanguage: 'ja',
      polishPresetId: 'meeting',
      exportFileNamePrefix: 'TEAM',
      enabledTextReplacementSetIds: ['set-b'],
      enabledHotwordSetIds: ['hot-a'],
      enabledPolishKeywordSetIds: ['kw-2'],
      enabledSpeakerProfileIds: ['speaker-b'],
    },
  };
}

describe('effectiveConfigService', () => {
  it('keeps global config unchanged when no project is active', () => {
    const config = createBaseConfig();
    expect(resolveEffectiveConfig(config, null)).toEqual(config);
  });

  it('overrides workflow defaults and filters enabled rule sets for an active project', () => {
    const effective = resolveEffectiveConfig(createBaseConfig(), createProject());

    expect(effective.summaryTemplateId).toBe('meeting');
    expect(effective.translationLanguage).toBe('ja');
    expect(effective.polishPresetId).toBe('meeting');
    expect(effective.polishKeywordSets?.[0].name).toBe('Brand');
    expect(effective.polishKeywordSets?.find((set) => set.id === 'kw-1')?.enabled).toBe(false);
    expect(effective.polishKeywordSets?.find((set) => set.id === 'kw-2')?.enabled).toBe(true);
    expect(effective.textReplacementSets?.find((set) => set.id === 'set-a')?.enabled).toBe(false);
    expect(effective.textReplacementSets?.find((set) => set.id === 'set-b')?.enabled).toBe(true);
    expect(effective.hotwordSets?.find((set) => set.id === 'hot-a')?.enabled).toBe(true);
    expect(effective.hotwordSets?.find((set) => set.id === 'hot-b')?.enabled).toBe(false);
    expect(effective.speakerProfiles?.find((profile) => profile.id === 'speaker-a')?.enabled).toBe(false);
    expect(effective.speakerProfiles?.find((profile) => profile.id === 'speaker-b')?.enabled).toBe(true);
  });
});
