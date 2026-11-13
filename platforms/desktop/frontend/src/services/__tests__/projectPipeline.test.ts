import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../../types/config';
import type { ProjectRecord } from '../../types/project';
import { resolveItemPipeline } from '../projectPipeline';

const config = {
  polishPresetId: 'global-polish',
  translationLanguage: 'zh',
  summaryTemplateId: 'global-summary',
} as unknown as AppConfig;

const project = (pipeline?: ProjectRecord['pipeline']): ProjectRecord => ({
  id: 'project-1',
  name: 'Project',
  description: '',
  icon: '',
  color: '#6366F1',
  sortOrder: 0,
  createdAt: 1,
  updatedAt: 1,
  pipeline,
});

describe('resolveItemPipeline', () => {
  it('falls back to global defaults for Inbox and missing projects', () => {
    expect(resolveItemPipeline(null, [project()], config)).toMatchObject({
      isProjectPipeline: false,
      autoPolish: false,
      autoTranslate: false,
      autoSummary: false,
      polishPresetId: 'global-polish',
    });
    expect(resolveItemPipeline('missing', [project()], config).isProjectPipeline).toBe(false);
  });

  it('uses enabled project values and inherits omitted global values', () => {
    const snapshot = resolveItemPipeline(
      'project-1',
      [
        project({
          enabled: true,
          autoPolish: false,
          autoTranslate: true,
          targetLanguage: 'en',
          autoSummary: false,
          autoExport: true,
          exportFormat: 'vtt',
        }),
      ],
      config
    );
    expect(snapshot).toMatchObject({
      isProjectPipeline: true,
      autoPolish: false,
      autoTranslate: true,
      targetLanguage: 'en',
      summaryTemplateId: 'global-summary',
      exportFormat: 'vtt',
    });
  });

  it('falls back to a positive auto-polish frequency', () => {
    const base = {
      enabled: true,
      autoPolish: true,
      autoTranslate: false,
      autoSummary: false,
      autoExport: false,
    };

    expect(
      resolveItemPipeline('project-1', [project({ ...base, autoPolishFrequency: 0 })], config)
        .autoPolishFrequency
    ).toBe(5);
    expect(
      resolveItemPipeline('project-1', [project({ ...base })], config).autoPolishFrequency
    ).toBe(5);
    expect(
      resolveItemPipeline('project-1', [project({ ...base, autoPolishFrequency: 3 })], config)
        .autoPolishFrequency
    ).toBe(3);
  });

  it('treats disabled project pipelines as global defaults', () => {
    expect(
      resolveItemPipeline(
        'project-1',
        [
          project({
            enabled: false,
            autoPolish: false,
            autoTranslate: true,
            autoSummary: false,
            autoExport: false,
          }),
        ],
        config
      ).isProjectPipeline
    ).toBe(false);
  });
});
