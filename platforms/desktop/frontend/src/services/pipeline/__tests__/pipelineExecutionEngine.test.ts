import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { TranscriptSegment } from '../../../types/transcript';
import type { EffectivePipelineSnapshot } from '../../projectPipeline';
import {
  PipelineExecutionEngine,
  type PipelineExecutionEnginePorts,
} from '../pipelineExecutionEngine';

describe('PipelineExecutionEngine', () => {
  let mockPorts: PipelineExecutionEnginePorts;
  let engine: PipelineExecutionEngine;

  const mockSegments: TranscriptSegment[] = [
    { id: 'seg-1', start: 0, end: 5, text: 'Hello world', isFinal: true },
  ];

  const baseConfig: AppConfig = {
    polishPresetId: 'global-polish',
    translationLanguage: 'zh',
    summaryTemplateId: 'global-summary',
  } as unknown as AppConfig;

  beforeEach(() => {
    mockPorts = {
      polishService: {
        polishSegmentsWithConfig: vi.fn().mockImplementation(async (_cfg, _segments, cb) => {
          await cb([{ id: 'seg-1', text: 'Polished text' }]);
        }),
        applyPolishedSegmentsInMemory: vi
          .fn()
          .mockReturnValue([{ id: 'seg-1', start: 0, end: 5, text: 'Polished text' }]),
      } as unknown as PipelineExecutionEnginePorts['polishService'],
      translationService: {
        translateSegmentsWithConfig: vi.fn().mockImplementation(async (_cfg, _segments, cb) => {
          await cb([{ id: 'seg-1', translation: '你好世界' }]);
        }),
        applyTranslationsInMemory: vi
          .fn()
          .mockReturnValue([
            { id: 'seg-1', start: 0, end: 5, text: 'Polished text', translation: '你好世界' },
          ]),
      } as unknown as PipelineExecutionEnginePorts['translationService'],
      summaryService: {
        retrySummaryTranscriptJob: vi.fn().mockResolvedValue(undefined),
        persistSummary: vi.fn().mockResolvedValue(undefined),
      } as unknown as PipelineExecutionEnginePorts['summaryService'],
      exportService: {
        exportTranscriptToDirectory: vi.fn().mockResolvedValue('/path/to/export.txt'),
      } as unknown as PipelineExecutionEnginePorts['exportService'],
      historyService: {
        updateTranscript: vi.fn().mockResolvedValue(undefined),
      } as unknown as PipelineExecutionEnginePorts['historyService'],
      isFeatureLlmConfigComplete: vi.fn().mockReturnValue(true),
    };

    engine = new PipelineExecutionEngine(mockPorts);
  });

  it('runs all enabled pipeline stages with project overrides', async () => {
    const pipeline: EffectivePipelineSnapshot = {
      isProjectPipeline: true,
      enabled: true,
      autoPolish: true,
      polishPresetId: 'custom-preset',
      autoTranslate: true,
      targetLanguage: 'ja',
      autoSummary: true,
      summaryTemplateId: 'custom-template',
      autoExport: true,
      exportDirectory: '/exports',
      exportFormat: 'srt',
      exportFileNamePrefix: 'AUD',
    };

    const onProgress = vi.fn();
    const onSegmentsUpdated = vi.fn();
    const onExportComplete = vi.fn();

    const result = await engine.execute({
      historyId: 'hist-1',
      segments: mockSegments,
      pipeline,
      globalConfig: baseConfig,
      baseFileName: 'meeting',
      onProgress,
      onSegmentsUpdated,
      onExportComplete,
    });

    // Verify polish called with project preset override
    expect(mockPorts.polishService.polishSegmentsWithConfig).toHaveBeenCalledWith(
      expect.objectContaining({ polishPresetId: 'custom-preset' }),
      expect.any(Array),
      expect.any(Function)
    );
    expect(mockPorts.historyService.updateTranscript).toHaveBeenCalledWith(
      'hist-1',
      expect.any(Array)
    );

    // Verify translation called with project target language override
    expect(mockPorts.translationService.translateSegmentsWithConfig).toHaveBeenCalledWith(
      expect.objectContaining({ translationLanguage: 'ja' }),
      expect.any(Array),
      expect.any(Function)
    );

    // Verify summary called with project template override
    expect(mockPorts.summaryService.retrySummaryTranscriptJob).toHaveBeenCalledWith(
      expect.objectContaining({ templateId: 'custom-template', historyId: 'hist-1' })
    );
    expect(mockPorts.summaryService.persistSummary).toHaveBeenCalledWith('hist-1');

    // Verify export called with project directory, format, and filename
    expect(mockPorts.exportService.exportTranscriptToDirectory).toHaveBeenCalledWith({
      segments: expect.any(Array),
      directory: '/exports',
      baseFileName: 'AUD meeting',
      format: 'srt',
      mode: 'bilingual',
    });
    expect(onExportComplete).toHaveBeenCalledWith('/path/to/export.txt');
    expect(result.exportPath).toBe('/path/to/export.txt');
  });

  it('falls back to global configs when project overrides are omitted', async () => {
    const pipeline: EffectivePipelineSnapshot = {
      isProjectPipeline: false,
      enabled: false,
      autoPolish: true,
      polishPresetId: undefined,
      autoTranslate: true,
      targetLanguage: undefined,
      autoSummary: true,
      summaryTemplateId: undefined,
      autoExport: false,
    };

    await engine.execute({
      historyId: 'hist-2',
      segments: mockSegments,
      pipeline,
      globalConfig: baseConfig,
    });

    expect(mockPorts.polishService.polishSegmentsWithConfig).toHaveBeenCalledWith(
      expect.objectContaining({ polishPresetId: 'global-polish' }),
      expect.any(Array),
      expect.any(Function)
    );
    expect(mockPorts.translationService.translateSegmentsWithConfig).toHaveBeenCalledWith(
      expect.objectContaining({ translationLanguage: 'zh' }),
      expect.any(Array),
      expect.any(Function)
    );
    expect(mockPorts.summaryService.retrySummaryTranscriptJob).toHaveBeenCalledWith(
      expect.objectContaining({ templateId: 'global-summary' })
    );
  });

  it('stops and throws when cancellation is requested', async () => {
    const pipeline: EffectivePipelineSnapshot = {
      isProjectPipeline: true,
      enabled: true,
      autoPolish: true,
      autoTranslate: true,
      autoSummary: true,
      autoExport: true,
    };

    let cancel = false;
    await expect(
      engine.execute({
        historyId: 'hist-3',
        segments: mockSegments,
        pipeline,
        globalConfig: baseConfig,
        isCancelRequested: () => {
          cancel = true;
          return cancel;
        },
      })
    ).rejects.toThrow('Pipeline task cancelled.');
  });
  it('applies project-scoped text replacements from dictionaryContent when projectId is provided', async () => {
    const configWithDict = {
      ...baseConfig,
      dictionaryContent: `
- global_term -> GlobalReplacement
projects:
  Alpha (id:proj-1):
    - project_term -> ProjectReplacement
`,
    } as unknown as AppConfig;

    const pipeline: EffectivePipelineSnapshot = {
      isProjectPipeline: true,
      enabled: false,
      autoPolish: false,
      autoTranslate: false,
      autoSummary: false,
      autoExport: false,
    };

    const inputSegments: TranscriptSegment[] = [
      {
        id: 'seg-1',
        start: 0,
        end: 5,
        text: 'This has global_term and project_term.',
        isFinal: true,
      },
    ];

    const result = await engine.execute({
      historyId: 'hist-4',
      projectId: 'proj-1',
      segments: inputSegments,
      pipeline,
      globalConfig: configWithDict,
    });

    expect(result.segments[0].text).toBe('This has GlobalReplacement and ProjectReplacement.');
  });
});
