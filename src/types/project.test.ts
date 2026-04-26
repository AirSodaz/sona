import { describe, expect, it } from 'vitest';
import {
  migrateProjectPolishDefaults,
  normalizeProjectRecord,
  normalizeProjectRecordWithKeywordSetBackfill,
} from './project';

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
        },
      }),
    ], []);

    expect(result.customPresets).toHaveLength(1);
    expect(result.projects[0].defaults.polishPresetId).toBe(result.customPresets[0].id);
    expect(result.migrated).toBe(true);
  });

  it('backfills missing project keyword set ids from the current global selection', () => {
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
    }, ['kw-1', 'kw-3']);

    expect(result.migrated).toBe(true);
    expect(result.project.defaults.enabledPolishKeywordSetIds).toEqual(['kw-1', 'kw-3']);
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
      },
    });

    expect(project.defaults.summaryTemplateId).toBe('meeting');
  });
});
