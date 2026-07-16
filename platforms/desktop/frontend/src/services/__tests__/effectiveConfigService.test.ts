import { beforeEach, describe, it, expect, vi } from 'vitest';
import { resolveEffectiveConfig } from '../effectiveConfigService';
import type { AppConfig } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import { createLlmSettings } from '../llm/state';
import { buildTestConfig } from '../../test-utils/configTestUtils';
import { resolveEffectiveConfig as resolveEffectiveConfigInRust } from '../tauri/app';

vi.mock('../tauri/app', () => ({
  resolveEffectiveConfig: vi.fn(),
}));

function createBaseConfig(): AppConfig {
  return buildTestConfig({
    configVersion: 5,
    appLanguage: 'en',
    theme: 'light',
    streamingModelPath: '/models/live',
    batchModelPath: '/models/batch',
    punctuationModelPath: '',
    vadModelPath: '',
    llmSettings: createLlmSettings(),
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
  });
}

function createProject(): ProjectRecord {
  return {
    id: 'project-1',
    name: 'Project One',
    description: '',
    icon: '',
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
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('delegates global config resolution to Rust when no project is active', async () => {
    const config = createBaseConfig();
    vi.mocked(resolveEffectiveConfigInRust).mockResolvedValueOnce(config);

    await expect(resolveEffectiveConfig(config, null)).resolves.toEqual(config);
    expect(resolveEffectiveConfigInRust).toHaveBeenCalledWith(config, null);
  });

  it('returns the Rust-resolved project overrides and filtered rule-set states', async () => {
    const config = createBaseConfig();
    const project = createProject();
    const effectiveConfig = {
      ...config,
      summaryTemplateId: 'meeting',
      translationLanguage: 'ja',
      polishPresetId: 'meeting',
      polishKeywordSets: config.polishKeywordSets?.map((set) => ({
        ...set,
        enabled: set.id === 'kw-2',
      })),
      textReplacementSets: config.textReplacementSets?.map((set) => ({
        ...set,
        enabled: set.id === 'set-b',
      })),
      hotwordSets: config.hotwordSets?.map((set) => ({
        ...set,
        enabled: set.id === 'hot-a',
      })),
      speakerProfiles: config.speakerProfiles?.map((profile) => ({
        ...profile,
        enabled: profile.id === 'speaker-b',
      })),
    };
    vi.mocked(resolveEffectiveConfigInRust).mockResolvedValueOnce(effectiveConfig);

    const effective = await resolveEffectiveConfig(config, project);

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
    expect(resolveEffectiveConfigInRust).toHaveBeenCalledWith(config, project);
  });
});
