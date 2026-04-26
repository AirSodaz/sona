import type {
  AppConfig,
  HotwordRuleSet,
  PolishCustomPreset,
  PolishKeywordRuleSet,
  TextReplacementRuleSet,
} from './config';
import type { SummaryTemplateId } from './transcript';
import {
  DEFAULT_POLISH_PRESET_ID,
  migrateLegacyPolishSelection,
  normalizePolishCustomPresets,
} from '../utils/polishPresets';
import { DEFAULT_SUMMARY_TEMPLATE_ID } from './transcript';
import { coerceSummaryTemplateId } from '../utils/summaryTemplates';

export interface ProjectDefaults {
  summaryTemplateId: SummaryTemplateId;
  translationLanguage: string;
  polishPresetId: string;
  /** Deprecated legacy scenario, retained only for migration. */
  polishScenario?: string;
  /** Deprecated legacy context, retained only for migration. */
  polishContext?: string;
  exportFileNamePrefix: string;
  enabledTextReplacementSetIds: string[];
  enabledHotwordSetIds: string[];
  enabledPolishKeywordSetIds: string[];
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

type ProjectDefaultsInput = Partial<ProjectDefaults> & {
  summaryTemplate?: string;
  polishScenario?: string;
  polishContext?: string;
};

type ProjectRecordInput = Partial<Omit<ProjectRecord, 'defaults'>> & {
  defaults?: ProjectDefaultsInput;
};

export function buildProjectDefaultsFromConfig(config: AppConfig): ProjectDefaults {
  return {
    summaryTemplateId: coerceSummaryTemplateId(
      config.summaryTemplateId,
      config.summaryCustomTemplates,
    ),
    translationLanguage: config.translationLanguage || 'zh',
    polishPresetId: config.polishPresetId || DEFAULT_POLISH_PRESET_ID,
    exportFileNamePrefix: '',
    enabledTextReplacementSetIds: (config.textReplacementSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id),
    enabledHotwordSetIds: (config.hotwordSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id),
    enabledPolishKeywordSetIds: (config.polishKeywordSets || [])
      .filter((set) => set.enabled)
      .map((set) => set.id),
  };
}

function resolveEnabledSetIds<T extends { id: string; enabled: boolean }>(
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

export function resolveProjectAwarePolishKeywordSets(
  sets: PolishKeywordRuleSet[] | undefined,
  project: ProjectRecord | null,
): PolishKeywordRuleSet[] | undefined {
  if (!project) {
    return sets;
  }

  return resolveEnabledSetIds(sets, project.defaults.enabledPolishKeywordSetIds);
}

export function normalizeProjectRecord(input: ProjectRecordInput): ProjectRecord {
  const now = Date.now();
  const defaults = input.defaults || {};

  return {
    id: input.id || '',
    name: input.name?.trim() || 'Untitled Project',
    description: input.description || '',
    icon: input.icon || '',
    createdAt: input.createdAt || now,
    updatedAt: input.updatedAt || input.createdAt || now,
    defaults: {
      summaryTemplateId: normalizeProjectSummaryTemplateId(defaults.summaryTemplateId, defaults.summaryTemplate),
      translationLanguage: defaults.translationLanguage || 'zh',
      polishPresetId: defaults.polishPresetId
        || ((defaults.polishScenario || defaults.polishContext) ? '' : DEFAULT_POLISH_PRESET_ID),
      polishScenario: defaults.polishScenario,
      polishContext: defaults.polishContext,
      exportFileNamePrefix: defaults.exportFileNamePrefix || '',
      enabledTextReplacementSetIds: defaults.enabledTextReplacementSetIds || [],
      enabledHotwordSetIds: defaults.enabledHotwordSetIds || [],
      enabledPolishKeywordSetIds: defaults.enabledPolishKeywordSetIds || [],
    },
  };
}

export function normalizeProjectRecordWithKeywordSetBackfill(
  input: ProjectRecordInput,
  fallbackEnabledPolishKeywordSetIds: string[],
): {
  project: ProjectRecord;
  migrated: boolean;
} {
  const defaults = input.defaults || {};
  const migrated =
    !Array.isArray(defaults.enabledPolishKeywordSetIds)
    || !isNonEmptyString(defaults.summaryTemplateId);

  if (!migrated) {
    return {
      project: normalizeProjectRecord(input),
      migrated: false,
    };
  }

  return {
    project: normalizeProjectRecord({
      ...input,
      defaults: {
        ...defaults,
        enabledPolishKeywordSetIds: [...fallbackEnabledPolishKeywordSetIds],
      },
    }),
    migrated: true,
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
      summaryTemplateId: project.defaults.summaryTemplateId,
      translationLanguage: project.defaults.translationLanguage,
      polishPresetId: selection.presetId,
      exportFileNamePrefix: project.defaults.exportFileNamePrefix,
      enabledTextReplacementSetIds: [...project.defaults.enabledTextReplacementSetIds],
      enabledHotwordSetIds: [...project.defaults.enabledHotwordSetIds],
      enabledPolishKeywordSetIds: [...project.defaults.enabledPolishKeywordSetIds],
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

function normalizeProjectSummaryTemplateId(
  summaryTemplateId: string | null | undefined,
  legacySummaryTemplate: string | null | undefined,
): SummaryTemplateId {
  if (isNonEmptyString(summaryTemplateId)) {
    return summaryTemplateId.trim();
  }

  if (isNonEmptyString(legacySummaryTemplate)) {
    return legacySummaryTemplate.trim();
  }

  return DEFAULT_SUMMARY_TEMPLATE_ID;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}
