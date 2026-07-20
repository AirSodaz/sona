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

  it('passes Tag metadata to Rust without applying legacy Tag defaults', async () => {
    const config = createBaseConfig();
    const project = createProject();
    vi.mocked(resolveEffectiveConfigInRust).mockResolvedValueOnce(config);

    const effective = await resolveEffectiveConfig(config, project);

    expect(effective).toEqual(config);
    expect(resolveEffectiveConfigInRust).toHaveBeenCalledWith(config, project);
  });
});
