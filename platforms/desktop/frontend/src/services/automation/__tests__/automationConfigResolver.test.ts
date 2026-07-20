import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { AutomationProfile, AutomationRule } from '../../../types/automation';
import {
  resolveAutomationQueueSnapshot,
  resolveMatchingTagAutomationRule,
} from '../automationConfigResolver';

function profile(id: string, language: string): AutomationProfile {
  return {
    id,
    name: id,
    translationLanguage: language,
    polishPresetId: `${id}-polish`,
    summaryTemplateId: `${id}-summary`,
    enabledTextReplacementSetIds: [],
    enabledHotwordSetIds: [],
    enabledPolishKeywordSetIds: [],
    enabledSpeakerProfileIds: [],
    createdAt: 1,
    updatedAt: 1,
  };
}

function rule(overrides: Partial<AutomationRule>): AutomationRule {
  return {
    id: 'rule',
    name: 'Rule',
    kind: 'tag',
    priority: 0,
    tagIds: [],
    presetId: 'custom',
    watchDirectory: '',
    recursive: false,
    enabled: true,
    actions: { autoPolish: false, autoTranslate: false, autoSummary: false },
    stageConfig: { autoPolish: false, autoTranslate: false, exportEnabled: false },
    exportConfig: { directory: '', format: 'txt', mode: 'original' },
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

const globalConfig = {
  translationLanguage: 'zh',
  polishPresetId: 'general',
  summaryTemplateId: 'general',
  textReplacementSets: [],
  hotwordSets: [],
  polishKeywordSets: [],
  speakerProfiles: [],
} as unknown as AppConfig;

describe('automationConfigResolver', () => {
  it('matches any configured Tag and selects the highest priority rule', () => {
    const lower = rule({ id: 'b', priority: 10, tagIds: ['tag-a', 'tag-b'] });
    const higher = rule({ id: 'c', priority: 20, tagIds: ['tag-c', 'tag-b'] });

    expect(resolveMatchingTagAutomationRule([lower, higher], ['tag-b'])).toBe(higher);
  });

  it('uses stable rule ID ordering for equal priorities', () => {
    const second = rule({ id: 'b', priority: 10, tagIds: ['tag-a'] });
    const first = rule({ id: 'a', priority: 10, tagIds: ['tag-a'] });

    expect(resolveMatchingTagAutomationRule([second, first], ['tag-a'])).toBe(first);
  });

  it('uses the matching Tag profile and freezes actions without export', () => {
    const tagRule = rule({
      id: 'tag-rule',
      tagIds: ['tag-a'],
      profileId: 'tag-profile',
      actions: { autoPolish: true, autoTranslate: true, autoSummary: true },
    });
    const snapshot = resolveAutomationQueueSnapshot({
      globalConfig,
      profiles: [profile('tag-profile', 'ja')],
      rules: [tagRule],
      tagIds: ['tag-a'],
      resolvedAt: 42,
    });

    expect(snapshot.config.translationLanguage).toBe('ja');
    expect(snapshot.stageConfig).toEqual(expect.objectContaining({
      autoPolish: true,
      autoTranslate: true,
      autoSummary: true,
      exportEnabled: false,
    }));
    expect(snapshot.resolution).toEqual(expect.objectContaining({
      tagRuleId: 'tag-rule',
      profileId: 'tag-profile',
      profileSource: 'tag',
      resolvedAt: 42,
    }));
  });

  it('lets an explicit file profile override the Tag profile while preserving Tag actions', () => {
    const tagRule = rule({
      id: 'tag-rule',
      tagIds: ['tag-a'],
      profileId: 'tag-profile',
      actions: { autoPolish: true, autoTranslate: false, autoSummary: true },
    });
    const fileRule = rule({
      id: 'file-rule',
      kind: 'file',
      profileId: 'file-profile',
      profileSource: 'explicit',
      stageConfig: { autoPolish: false, autoTranslate: false, exportEnabled: true },
    });
    const snapshot = resolveAutomationQueueSnapshot({
      globalConfig,
      profiles: [profile('tag-profile', 'ja'), profile('file-profile', 'en')],
      rules: [tagRule, fileRule],
      fileRule,
      tagIds: ['tag-a'],
    });

    expect(snapshot.config.translationLanguage).toBe('en');
    expect(snapshot.resolution.profileSource).toBe('file');
    expect(snapshot.resolution.actions).toEqual(tagRule.actions);
    expect(snapshot.stageConfig.exportEnabled).toBe(true);
  });

  it('falls back to global settings and no actions when no Tag rule matches', () => {
    const snapshot = resolveAutomationQueueSnapshot({
      globalConfig,
      profiles: [],
      rules: [],
      tagIds: [],
    });

    expect(snapshot.config.translationLanguage).toBe('zh');
    expect(snapshot.resolution.profileSource).toBe('global');
    expect(snapshot.resolution.actions).toEqual({
      autoPolish: false,
      autoTranslate: false,
      autoSummary: false,
    });
  });
});
