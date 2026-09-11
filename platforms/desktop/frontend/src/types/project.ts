import type {
  TagCreateInput as GeneratedCreateInput,
  TagRecord as GeneratedTagRecord,
  TagUpdateInput as GeneratedUpdateInput,
} from './tag';

export type ProjectPipelineConfig = {
  enabled: boolean;
  autoPolish: boolean;
  polishPresetId?: string;
  polishPromptOverride?: string;
  autoTranslate: boolean;
  targetLanguage?: string;
  autoSummary: boolean;
  summaryTemplateId?: string;
  hotwordSetIds?: string[];
  replacementSetIds?: string[];
  autoExport: boolean;
  exportFormat?: 'txt' | 'srt' | 'vtt' | 'json' | 'docx' | 'md';
  exportDirectory?: string;
  exportFileNamePrefix?: string;
};

export type EffectivePipelineSnapshot = ProjectPipelineConfig & {
  isProjectPipeline: boolean;
};

export type ProjectRecord = Omit<GeneratedTagRecord, 'color' | 'sortOrder'> & {
  color?: string;
  sortOrder?: number;
  pipeline?: ProjectPipelineConfig;
};

export type ProjectCreateInput = GeneratedCreateInput & { pipeline?: ProjectPipelineConfig };
export type ProjectUpdateInput = GeneratedUpdateInput & { pipeline?: ProjectPipelineConfig };

export const DEFAULT_PROJECT_PIPELINE: ProjectPipelineConfig = {
  enabled: false,
  autoPolish: false,
  autoTranslate: false,
  autoSummary: false,
  autoExport: false,
};
