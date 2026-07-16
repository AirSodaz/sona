import type {
  ProjectCreateInput as GeneratedProjectCreateInput,
  ProjectDefaultsInput as GeneratedProjectDefaultsInput,
  ProjectDefaultsPatch as GeneratedProjectDefaultsPatch,
  ProjectDefaults_Serialize as GeneratedProjectDefaults,
  ProjectRecord_Serialize as GeneratedProjectRecord,
  ProjectUpdateInput as GeneratedProjectUpdateInput,
} from '../bindings';
import type { SummaryTemplateId } from './transcript';

type WithoutNull<T> = { [K in keyof T]: Exclude<T[K], null> };
type NormalizedProjectCreateInput = WithoutNull<GeneratedProjectCreateInput>;
type NormalizedProjectDefaultsInput = WithoutNull<GeneratedProjectDefaultsInput>;
type NormalizedProjectDefaults = WithoutNull<GeneratedProjectDefaults>;
type NormalizedProjectDefaultsPatch = WithoutNull<GeneratedProjectDefaultsPatch>;
type NormalizedProjectUpdateInput = WithoutNull<GeneratedProjectUpdateInput>;

export type ProjectDefaults = Omit<NormalizedProjectDefaults, 'summaryTemplateId'> & {
  summaryTemplateId: SummaryTemplateId;
};
export type ProjectDefaultsInput = Omit<
  NormalizedProjectDefaultsInput,
  'summaryTemplateId'
> & {
  summaryTemplateId?: SummaryTemplateId;
};
export type ProjectCreateInput = Omit<NormalizedProjectCreateInput, 'defaults'> & {
  defaults: ProjectDefaultsInput;
};
export type ProjectDefaultsPatch = Omit<
  NormalizedProjectDefaultsPatch,
  'summaryTemplateId'
> & {
  summaryTemplateId?: SummaryTemplateId;
};
export type ProjectRecord = Omit<GeneratedProjectRecord, 'defaults'> & {
  defaults: ProjectDefaults;
};
export type ProjectUpdateInput = Omit<NormalizedProjectUpdateInput, 'defaults'> & {
  defaults?: ProjectDefaultsPatch;
};

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
