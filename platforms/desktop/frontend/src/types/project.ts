// Compatibility aliases kept for one release while desktop callers migrate to Tag.
export type {
  TagCreateInput as ProjectCreateInput,
  TagDefaults as ProjectDefaults,
  TagDefaultsInput as ProjectDefaultsInput,
  TagDefaultsPatch as ProjectDefaultsPatch,
  TagRecord as ProjectRecord,
  TagUpdateInput as ProjectUpdateInput,
} from './tag';

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
