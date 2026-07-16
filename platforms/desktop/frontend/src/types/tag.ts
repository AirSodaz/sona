import type {
  TagCreateInput as GeneratedTagCreateInput,
  TagDefaultsInput as GeneratedTagDefaultsInput,
  TagDefaultsPatch as GeneratedTagDefaultsPatch,
  TagDefaults_Serialize as GeneratedTagDefaults,
  TagRecord_Serialize as GeneratedTagRecord,
  TagUpdateInput as GeneratedTagUpdateInput,
} from '../bindings';
import type { SummaryTemplateId } from './transcript';

type WithoutNull<T> = { [K in keyof T]: Exclude<T[K], null> };
type NormalizedTagCreateInput = WithoutNull<GeneratedTagCreateInput>;
type NormalizedTagDefaultsInput = WithoutNull<GeneratedTagDefaultsInput>;
type NormalizedTagDefaults = WithoutNull<GeneratedTagDefaults>;
type NormalizedTagDefaultsPatch = WithoutNull<GeneratedTagDefaultsPatch>;
type NormalizedTagUpdateInput = WithoutNull<GeneratedTagUpdateInput>;

export type TagDefaults = Omit<NormalizedTagDefaults, 'summaryTemplateId'> & {
  summaryTemplateId: SummaryTemplateId;
};
export type TagDefaultsInput = Omit<NormalizedTagDefaultsInput, 'summaryTemplateId'> & {
  summaryTemplateId?: SummaryTemplateId;
};
export type TagCreateInput = Omit<NormalizedTagCreateInput, 'defaults'> & {
  defaults: TagDefaultsInput;
};
export type TagDefaultsPatch = Omit<NormalizedTagDefaultsPatch, 'summaryTemplateId'> & {
  summaryTemplateId?: SummaryTemplateId;
};
export type TagRecord = Omit<GeneratedTagRecord, 'defaults' | 'color' | 'sortOrder'> & {
  color?: string;
  sortOrder?: number;
  defaults: TagDefaults;
};
export type TagUpdateInput = Omit<NormalizedTagUpdateInput, 'defaults'> & {
  defaults?: TagDefaultsPatch;
};
