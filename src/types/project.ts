import type { AppConfig, HotwordRuleSet, PolishCustomPreset, TextReplacementRuleSet } from './config';
import type { SummaryTemplate } from './transcript';
import {
  DEFAULT_POLISH_PRESET_ID,
  migrateLegacyPolishSelection,
  normalizePolishCustomPresets,
} from '../utils/polishPresets';

export interface ProjectDefaults {
  summaryTemplate: SummaryTemplate;
  translationLanguage: string;
  polishPresetId: string;
  /** Deprecated legacy scenario, retained only for migration. */
  polishScenario?: string;
  /** Deprecated legacy context, retained only for migration. */
  polishContext?: string;
  exportFileNamePrefix: string;
  enabledTextReplacementSetIds: string[];
  enabledHotwordSetIds: string[];
}

export interface ProjectRecord {
  id: string;
  name: string;
  description: string;
  icon?: string;
  createdAt: number;
  updatedAt: number;
  defaults: ProjectDefaults;
}

export function buildProjectDefaultsFromConfig(config: AppConfig): ProjectDefaults {
  return {
    summaryTemplate: 'general',
    translationLanguage: config.translationLanguage || 'zh',
    polishPresetId: config.polishPresetId || DEFAULT_POLISH_PRESET_ID,
    exportFileNamePrefix: '',
    enabledTextReplacementSetIds: (config.textReplacementSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id),
    enabledHotwordSetIds: (config.hotwordSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id),
  };
}

function resolveEnabledSetIds<T extends { id: string }>(
  sets: T[] | undefined,
  enabledIds: string[],
): T[] {
  if (!sets || sets.length === 0) {
    return [];
  }

  const enabledIdSet = new Set(enabledIds);
  return sets.map((set) => ({
    ...set,
    enabled: enabledIdSet.has(set.id),
  })) as T[];
}

export function resolveProjectAwareTextReplacementSets(
  sets: TextReplacementRuleSet[] | undefined,
  project: ProjectRecord | null,
): TextReplacementRuleSet[] | undefined {
  if (!project) {
    return sets;
  }

  return resolveEnabledSetIds(sets, project.defaults.enabledTextReplacementSetIds);
}

export function resolveProjectAwareHotwordSets(
  sets: HotwordRuleSet[] | undefined,
  project: ProjectRecord | null,
): HotwordRuleSet[] | undefined {
  if (!project) {
    return sets;
  }

  return resolveEnabledSetIds(sets, project.defaults.enabledHotwordSetIds);
}

export function normalizeProjectRecord(input: Partial<ProjectRecord>): ProjectRecord {
  const now = Date.now();
  const defaults = (input.defaults || {}) as Partial<ProjectDefaults> & {
    polishScenario?: string;
    polishContext?: string;
  };

  return {
    id: input.id || '',
    name: input.name?.trim() || 'Untitled Project',
    description: input.description || '',
    icon: input.icon || '',
    createdAt: input.createdAt || now,
    updatedAt: input.updatedAt || input.createdAt || now,
    defaults: {
      summaryTemplate: defaults.summaryTemplate || 'general',
      translationLanguage: defaults.translationLanguage || 'zh',
      polishPresetId: defaults.polishPresetId
        || ((defaults.polishScenario || defaults.polishContext) ? '' : DEFAULT_POLISH_PRESET_ID),
      polishScenario: defaults.polishScenario,
      polishContext: defaults.polishContext,
      exportFileNamePrefix: defaults.exportFileNamePrefix || '',
      enabledTextReplacementSetIds: defaults.enabledTextReplacementSetIds || [],
      enabledHotwordSetIds: defaults.enabledHotwordSetIds || [],
    },
  };
}

export function migrateProjectPolishDefaults(
  projects: ProjectRecord[],
  initialCustomPresets: PolishCustomPreset[] | null | undefined,
): {
  projects: ProjectRecord[];
  customPresets: PolishCustomPreset[];
  migrated: boolean;
} {
  let customPresets = normalizePolishCustomPresets(initialCustomPresets);
  let migrated = false;

  const nextProjects = projects.map((project) => {
    const selection = migrateLegacyPolishSelection(
      {
        presetId: project.defaults.polishPresetId,
        scenario: project.defaults.polishScenario,
        context: project.defaults.polishContext,
      },
      customPresets,
      `${project.name} Context`,
    );

    const nextDefaults: ProjectDefaults = {
      summaryTemplate: project.defaults.summaryTemplate,
      translationLanguage: project.defaults.translationLanguage,
      polishPresetId: selection.presetId,
      exportFileNamePrefix: project.defaults.exportFileNamePrefix,
      enabledTextReplacementSetIds: [...project.defaults.enabledTextReplacementSetIds],
      enabledHotwordSetIds: [...project.defaults.enabledHotwordSetIds],
    };

    customPresets = selection.customPresets;

    const projectMigrated =
      project.defaults.polishPresetId !== nextDefaults.polishPresetId
      || typeof project.defaults.polishScenario !== 'undefined'
      || typeof project.defaults.polishContext !== 'undefined';

    if (projectMigrated) {
      migrated = true;
      return {
        ...project,
        defaults: nextDefaults,
      };
    }

    return project;
  });

  if (
    JSON.stringify(normalizePolishCustomPresets(initialCustomPresets)) !== JSON.stringify(customPresets)
  ) {
    migrated = true;
  }

  return {
    projects: nextProjects,
    customPresets,
    migrated,
  };
}
