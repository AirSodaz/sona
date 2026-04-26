import type { AppConfig, HotwordRuleSet, TextReplacementRuleSet } from './config';
import type { SummaryTemplate } from './transcript';

export interface ProjectDefaults {
  summaryTemplate: SummaryTemplate;
  translationLanguage: string;
  polishScenario: string;
  polishContext: string;
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
    polishScenario: config.polishScenario || 'custom',
    polishContext: config.polishContext || '',
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

  return {
    id: input.id || '',
    name: input.name?.trim() || 'Untitled Project',
    description: input.description || '',
    createdAt: input.createdAt || now,
    updatedAt: input.updatedAt || input.createdAt || now,
    defaults: {
      summaryTemplate: input.defaults?.summaryTemplate || 'general',
      translationLanguage: input.defaults?.translationLanguage || 'zh',
      polishScenario: input.defaults?.polishScenario || 'custom',
      polishContext: input.defaults?.polishContext || '',
      exportFileNamePrefix: input.defaults?.exportFileNamePrefix || '',
      enabledTextReplacementSetIds: input.defaults?.enabledTextReplacementSetIds || [],
      enabledHotwordSetIds: input.defaults?.enabledHotwordSetIds || [],
    },
  };
}
