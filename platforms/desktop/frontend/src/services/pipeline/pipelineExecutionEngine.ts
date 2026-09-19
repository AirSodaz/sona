import type { AppConfig, TextReplacementRule, TextReplacementRuleSet } from '../../types/config';
import type { TranscriptSegment } from '../../types/transcript';
import type { ExportFormat, ExportMode } from '../../utils/exportFormats';
import { logger } from '../../utils/logger';
import { applyTextReplacements } from '../../utils/textProcessing';
import { getTermsForProject, parseYamlDictionary } from '../../utils/yamlDictionaryParser';
import { exportService } from '../exportService';
import { historyService } from '../historyService';
import { isFeatureLlmConfigComplete } from '../llm/configUtils';
import { polishService } from '../polishService';
import type { EffectivePipelineSnapshot } from '../projectPipeline';
import { summaryService } from '../summaryService';
import { translationService } from '../translationService';

export type PipelineStage = 'polishing' | 'translating' | 'summarizing' | 'exporting';

export interface ExecutePipelineOptions {
  historyId: string;
  segments: TranscriptSegment[];
  pipeline: EffectivePipelineSnapshot;
  globalConfig: AppConfig;
  baseFileName?: string;
  onProgress?: (stage: PipelineStage, progress: number) => void;
  onSegmentsUpdated?: (segments: TranscriptSegment[]) => void | Promise<void>;
  onExportComplete?: (exportPath: string) => void;
  isCancelRequested?: () => boolean;
}

export interface PipelineExecutionResult {
  segments: TranscriptSegment[];
  exportPath?: string;
}

export interface PipelineExecutionEnginePorts {
  polishService: typeof polishService;
  translationService: typeof translationService;
  summaryService: typeof summaryService;
  exportService: typeof exportService;
  historyService: typeof historyService;
  isFeatureLlmConfigComplete: typeof isFeatureLlmConfigComplete;
}

function extractReplacementsFromTerms(terms: string[] = []): TextReplacementRule[] {
  const rules: TextReplacementRule[] = [];
  for (const term of terms) {
    const trimmed = term.trim();
    if (!trimmed) continue;
    let from = '';
    let to = '';
    if (trimmed.includes('=>')) {
      const parts = trimmed.split('=>');
      from = parts[0]?.trim() || '';
      to = parts[1]?.trim() || '';
    } else if (trimmed.includes('->')) {
      const parts = trimmed.split('->');
      from = parts[0]?.trim() || '';
      to = parts[1]?.trim() || '';
    }
    if (from && to) {
      rules.push({ id: `term_${from}_${to}`, from, to });
    }
  }
  return rules;
}
export class PipelineExecutionEngine {
  constructor(private readonly ports: PipelineExecutionEnginePorts) {}

  private throwIfCancelled(isCancelRequested?: () => boolean): void {
    if (isCancelRequested?.()) {
      throw new Error('Pipeline task cancelled.');
    }
  }

  async execute(options: ExecutePipelineOptions): Promise<PipelineExecutionResult> {
    const {
      historyId,
      pipeline,
      globalConfig,
      baseFileName,
      onProgress,
      onSegmentsUpdated,
      onExportComplete,
      isCancelRequested,
    } = options;

    let currentSegments = [...options.segments];
    let exportedPath: string | undefined;

    if (currentSegments.length === 0) {
      return { segments: currentSegments };
    }

    // 0. Auto Text Replacements from Unified Dictionary (or legacy pipeline terms)
    const effectiveReplacementSets: TextReplacementRuleSet[] = [];
    if (globalConfig.dictionaryContent?.trim()) {
      const parsed = parseYamlDictionary(globalConfig.dictionaryContent);
      const projectId =
        'projectId' in pipeline && typeof pipeline.projectId === 'string'
          ? pipeline.projectId
          : undefined;
      const terms = getTermsForProject(parsed, projectId);
      if (terms.replacements.length > 0) {
        effectiveReplacementSets.push({
          id: 'unified-dictionary-replacements',
          name: 'Unified Dictionary',
          enabled: true,
          ignoreCase: false,
          rules: terms.replacements.map((r, i) => ({
            id: `rep_${i}`,
            from: r.from,
            to: r.to,
          })),
        });
      }
    } else {
      if (pipeline.replacementSetIds && pipeline.replacementSetIds.length > 0) {
        const globalSets = globalConfig.textReplacementSets ?? [];
        for (const setId of pipeline.replacementSetIds) {
          const found = globalSets.find((s) => s.id === setId);
          if (found) effectiveReplacementSets.push(found);
        }
      }
      const customReplacements = extractReplacementsFromTerms(pipeline.customTerms);
      if (customReplacements.length > 0) {
        effectiveReplacementSets.push({
          id: 'pipeline-custom-terms',
          name: 'Project Terms',
          enabled: true,
          ignoreCase: false,
          rules: customReplacements,
        });
      }
    }
    if (effectiveReplacementSets.length > 0) {
      currentSegments = currentSegments.map((seg) => ({
        ...seg,
        text: applyTextReplacements(seg.text, effectiveReplacementSets),
      }));
      await onSegmentsUpdated?.(currentSegments);
      if (historyId) {
        await this.ports.historyService.updateTranscript(historyId, currentSegments);
      }
    }

    // 1. Auto Polish
    if (pipeline.autoPolish) {
      this.throwIfCancelled(isCancelRequested);
      if (!this.ports.isFeatureLlmConfigComplete(globalConfig, 'polish')) {
        logger.warn(
          '[PipelineExecutionEngine] Polish model is not configured, skipping auto-polish.'
        );
      } else {
        onProgress?.('polishing', 96);
        const polishConfig: AppConfig = {
          ...globalConfig,
          polishPresetId: pipeline.polishPresetId || globalConfig.polishPresetId,
        };
        await this.ports.polishService.polishSegmentsWithConfig(
          polishConfig,
          currentSegments,
          async (polishedChunk) => {
            currentSegments = this.ports.polishService.applyPolishedSegmentsInMemory(
              currentSegments,
              polishedChunk
            );
            await onSegmentsUpdated?.(currentSegments);
          }
        );
        if (historyId) {
          await this.ports.historyService.updateTranscript(historyId, currentSegments);
        }
      }
    }

    // 2. Auto Translate
    if (pipeline.autoTranslate) {
      this.throwIfCancelled(isCancelRequested);
      if (!this.ports.isFeatureLlmConfigComplete(globalConfig, 'translation')) {
        logger.warn(
          '[PipelineExecutionEngine] Translation model is not configured, skipping auto-translate.'
        );
      } else {
        onProgress?.('translating', 98);
        const translateConfig: AppConfig = {
          ...globalConfig,
          translationLanguage: pipeline.targetLanguage || globalConfig.translationLanguage,
        };
        await this.ports.translationService.translateSegmentsWithConfig(
          translateConfig,
          currentSegments,
          async (translatedChunk) => {
            currentSegments = this.ports.translationService.applyTranslationsInMemory(
              currentSegments,
              translatedChunk
            );
            await onSegmentsUpdated?.(currentSegments);
          }
        );
        if (historyId) {
          await this.ports.historyService.updateTranscript(historyId, currentSegments);
        }
      }
    }

    // 3. Auto Summary
    if (pipeline.autoSummary && historyId) {
      this.throwIfCancelled(isCancelRequested);
      if (!this.ports.isFeatureLlmConfigComplete(globalConfig, 'summary')) {
        logger.warn(
          '[PipelineExecutionEngine] Summary model is not configured, skipping auto-summary.'
        );
      } else {
        onProgress?.('summarizing', 99);
        const templateId = pipeline.summaryTemplateId || globalConfig.summaryTemplateId;
        await this.ports.summaryService.retrySummaryTranscriptJob({
          segments: currentSegments,
          historyId,
          templateId,
          config: globalConfig,
        });
        await this.ports.summaryService.persistSummary(historyId);
      }
    }

    // 4. Auto Export
    if (pipeline.autoExport && pipeline.exportDirectory) {
      this.throwIfCancelled(isCancelRequested);
      onProgress?.('exporting', 100);
      const format = (pipeline.exportFormat as ExportFormat) || 'txt';
      const mode: ExportMode = pipeline.autoTranslate ? 'bilingual' : 'original';
      const prefix = (pipeline.exportFileNamePrefix || '').trim();
      const rawBaseName = baseFileName || historyId || 'transcript';
      const finalBaseName = prefix ? `${prefix} ${rawBaseName}`.trim() : rawBaseName;

      exportedPath = await this.ports.exportService.exportTranscriptToDirectory({
        segments: currentSegments,
        directory: pipeline.exportDirectory,
        baseFileName: finalBaseName,
        format,
        mode,
      });
      onExportComplete?.(exportedPath);
    }

    return {
      segments: currentSegments,
      exportPath: exportedPath,
    };
  }
}

export function createPipelineExecutionEngine(
  ports: PipelineExecutionEnginePorts = {
    polishService,
    translationService,
    summaryService,
    exportService,
    historyService,
    isFeatureLlmConfigComplete,
  }
): PipelineExecutionEngine {
  return new PipelineExecutionEngine(ports);
}

export const pipelineExecutionEngine = createPipelineExecutionEngine();
