import type { SummaryTemplateId } from './transcript';

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
  enabledSpeakerProfileIds: string[];
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

export {
  buildProjectDefaultsFromConfig,
  migrateProjectPolishDefaults,
  normalizeProjectRecord,
  normalizeProjectRecordWithKeywordSetBackfill,
  resolveProjectAwareHotwordSets,
  resolveProjectAwarePolishKeywordSets,
  resolveProjectAwareSpeakerProfiles,
  resolveProjectAwareTextReplacementSets,
} from '../services/project/projectDefaults';
