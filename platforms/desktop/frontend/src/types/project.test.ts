import { describe, expect, it } from 'vitest';
import {
  buildProjectDefaultsFromConfig,
  migrateProjectPolishDefaults,
  normalizeProjectRecord,
  normalizeProjectRecordWithKeywordSetBackfill,
  resolveProjectAwareTextReplacementSets,
} from './project';
import { DEFAULT_CONFIG } from '../stores/configStore';

describe('normalizeProjectRecord', () => {
  it('preserves an incoming icon value', () => {
    const project = normalizeProjectRecord({
      id: 'project-1',
      name: 'Alpha',
      icon: '🧪',
      createdAt: 1,
      updatedAt: 2,
      defaults: {
        summaryTemplateId: 'general',
        translationLanguage: 'en',
        polishPresetId: 'general',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: ['set-1'],
        enabledHotwordSetIds: ['hot-1'],
        enabledPolishKeywordSetIds: ['kw-1'],
        enabledSpeakerProfileIds: ['speaker-1'],
      },
    });

    expect(project.icon).toBe('🧪');
  });

  it('migrates legacy project polish context into a shared custom preset reference', () => {
    const result = migrateProjectPolishDefaults([
      normalizeProjectRecord({
        id: 'project-1',
        name: 'Alpha',
        defaults: {
          summaryTemplateId: 'general',
          translationLanguage: 'en',
          polishPresetId: '',
          polishScenario: 'custom',
          polishContext: 'Use product terminology.',
          exportFileNamePrefix: '',
          enabledTextReplacementSetIds: [],
          enabledHotwordSetIds: [],
          enabledPolishKeywordSetIds: [],
          enabledSpeakerProfileIds: [],
        },
      }),
    ], []);

    expect(result.customPresets).toHaveLength(1);
    expect(result.projects[0].defaults.polishPresetId).toBe(result.customPresets[0].id);
    expect(result.migrated).toBe(true);
  });

  it('backfills missing project keyword set ids and speaker profile ids from the current global selection', () => {
    const result = normalizeProjectRecordWithKeywordSetBackfill({
      id: 'project-1',
      name: 'Alpha',
      defaults: {
        summaryTemplateId: 'general',
        translationLanguage: 'en',
        polishPresetId: 'general',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: [],
        enabledHotwordSetIds: [],
      },
    }, ['kw-1', 'kw-3'], ['speaker-1']);

    expect(result.migrated).toBe(true);
    expect(result.project.defaults.enabledPolishKeywordSetIds).toEqual(['kw-1', 'kw-3']);
    expect(result.project.defaults.enabledSpeakerProfileIds).toEqual(['speaker-1']);
  });

  it('migrates a legacy summaryTemplate field to summaryTemplateId', () => {
    const project = normalizeProjectRecord({
      id: 'project-1',
      name: 'Alpha',
      defaults: {
        summaryTemplate: 'meeting',
        translationLanguage: 'en',
        polishPresetId: 'general',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: [],
        enabledHotwordSetIds: [],
        enabledPolishKeywordSetIds: [],
        enabledSpeakerProfileIds: [],
      },
    });

    expect(project.defaults.summaryTemplateId).toBe('meeting');
  });

  it('keeps runtime helpers re-exported from the type facade', () => {
    expect(typeof buildProjectDefaultsFromConfig).toBe('function');
    expect(typeof normalizeProjectRecord).toBe('function');
    expect(typeof normalizeProjectRecordWithKeywordSetBackfill).toBe('function');
    expect(typeof migrateProjectPolishDefaults).toBe('function');
    expect(typeof resolveProjectAwareTextReplacementSets).toBe('function');
  });

  it('builds project defaults from only enabled global config sets through the facade', () => {
    const defaults = buildProjectDefaultsFromConfig({
      ...DEFAULT_CONFIG,
      translationLanguage: 'ja',
      polishPresetId: 'technical',
      textReplacementSets: [
        { id: 'replace-on', name: 'On', enabled: true, ignoreCase: true, rules: [] },
        { id: 'replace-off', name: 'Off', enabled: false, ignoreCase: true, rules: [] },
      ],
      hotwordSets: [
        { id: 'hot-on', name: 'On', enabled: true, rules: [] },
        { id: 'hot-off', name: 'Off', enabled: false, rules: [] },
      ],
      polishKeywordSets: [
        { id: 'kw-on', name: 'On', enabled: true, keywords: '' },
        { id: 'kw-off', name: 'Off', enabled: false, keywords: '' },
      ],
      speakerProfiles: [
        { id: 'speaker-on', name: 'On', enabled: true, samples: [] },
        { id: 'speaker-off', name: 'Off', enabled: false, samples: [] },
      ],
    });

    expect(defaults.enabledTextReplacementSetIds).toEqual(['replace-on']);
    expect(defaults.enabledHotwordSetIds).toEqual(['hot-on']);
    expect(defaults.enabledPolishKeywordSetIds).toEqual(['kw-on']);
    expect(defaults.enabledSpeakerProfileIds).toEqual(['speaker-on']);
  });
});
