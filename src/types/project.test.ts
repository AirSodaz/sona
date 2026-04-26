import { describe, expect, it } from 'vitest';
import { normalizeProjectRecord } from './project';

describe('normalizeProjectRecord', () => {
  it('preserves an incoming icon value', () => {
    const project = normalizeProjectRecord({
      id: 'project-1',
      name: 'Alpha',
      icon: '🧪',
      createdAt: 1,
      updatedAt: 2,
      defaults: {
        summaryTemplate: 'general',
        translationLanguage: 'en',
        polishScenario: 'custom',
        polishContext: '',
        exportFileNamePrefix: '',
        enabledTextReplacementSetIds: ['set-1'],
        enabledHotwordSetIds: ['hot-1'],
      },
    });

    expect(project.icon).toBe('🧪');
  });
});
