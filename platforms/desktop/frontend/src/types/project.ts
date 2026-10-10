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
  customTerms?: string[];
  autoExport: boolean;
  exportFormat?: 'txt' | 'srt' | 'vtt' | 'json' | 'docx' | 'md';
  exportDirectory?: string;
  exportFileNamePrefix?: string;
};

export type EffectivePipelineSnapshot = ProjectPipelineConfig & {
  isProjectPipeline: boolean;
};

export interface ProjectRecord {
  id: string;
  name: string;
  description?: string;
  icon?: string | null;
  color?: string | null;
  sortOrder?: number;
  createdAt: number;
  updatedAt: number;
  pipeline?: ProjectPipelineConfig;
}

export interface ProjectCreateInput {
  name: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  pipeline?: ProjectPipelineConfig;
}

export interface ProjectUpdateInput {
  name?: string | null;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  pipeline?: ProjectPipelineConfig;
}

export const DEFAULT_PROJECT_PIPELINE: ProjectPipelineConfig = {
  enabled: false,
  autoPolish: false,
  autoTranslate: false,
  autoSummary: false,
  autoExport: false,
  customTerms: [],
};
