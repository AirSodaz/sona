import { describe, it, expect, vi, beforeEach } from 'vitest';
import { BatchItemProcessor, type BatchItemProcessorPorts } from '../batchItemProcessor';
import type { BatchQueueItem } from '../../../types/batchQueue';
import type { AppConfig } from '../../../types/config';

vi.mock('@tauri-apps/api/path', () => ({
  tempDir: vi.fn(() => Promise.resolve('/tmp')),
  join: vi.fn((...args: string[]) => Promise.resolve(args.join('/'))),
}));

vi.mock('../../tauri/platform/fs', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../tauri/platform/fs')>();
  return {
    ...actual,
    remove: vi.fn(() => Promise.resolve()),
  };
});

describe('BatchItemProcessor', () => {
  let mockPorts: BatchItemProcessorPorts;
  let processor: BatchItemProcessor;

  beforeEach(() => {
    mockPorts = {
      transcriptionService: {
        transcribeFile: vi.fn().mockResolvedValue([
          { id: 'seg-1', start: 0, end: 10, text: 'Raw transcription' },
        ]),
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
      } as unknown as BatchItemProcessorPorts['transcriptionService'],
      historyService: {
        saveImportedFileToProject: vi.fn().mockResolvedValue({
          id: 'hist-1',
          title: 'Imported',
          duration: 10,
        }),
        saveImportedFile: vi.fn().mockResolvedValue({
          id: 'hist-1',
          title: 'Imported',
          duration: 10,
        }),
      } as unknown as BatchItemProcessorPorts['historyService'],
      polishService: {} as unknown as BatchItemProcessorPorts['polishService'],
      translationService: {} as unknown as BatchItemProcessorPorts['translationService'],
      summaryService: {
        persistSummary: vi.fn().mockResolvedValue(undefined),
      } as unknown as BatchItemProcessorPorts['summaryService'],
      exportTranscriptToDirectory: vi.fn(),
      asrConfigService: {
        resolveAsrTranscriptionRequest: vi.fn().mockReturnValue({
          engine: 'online',
          mode: 'batch',
          providerId: 'volcengine-doubao',
        }),
        isAsrRequestConfigured: vi.fn().mockReturnValue(true),
      } as unknown as BatchItemProcessorPorts['asrConfigService'],
      useHistoryStore: {
        getState: vi.fn().mockReturnValue({
          addItem: vi.fn(),
          updateTranscript: vi.fn(),
        }),
      } as unknown as BatchItemProcessorPorts['useHistoryStore'],
      useProjectStore: {
        getState: vi.fn().mockReturnValue({
          projects: [
            {
              id: 'proj-1',
              name: 'My Project',
              pipeline: {
                enabled: true,
                autoPolish: true,
                polishPresetId: 'proj-preset',
                autoTranslate: false,
                autoSummary: false,
                autoExport: false,
              },
            },
          ],
          getProjectById: vi.fn().mockReturnValue({ id: 'proj-1' }),
        }),
      } as unknown as BatchItemProcessorPorts['useProjectStore'],
      pipelineExecutionEngine: {
        execute: vi.fn().mockResolvedValue({
          segments: [{ id: 'seg-1', start: 0, end: 10, text: 'Polished transcription' }],
        }),
      } as unknown as BatchItemProcessorPorts['pipelineExecutionEngine'],
    };

    processor = new BatchItemProcessor(mockPorts);
  });

  it('delegates post-processing to pipelineExecutionEngine with project pipeline', async () => {
    const item: BatchQueueItem = {
      id: 'queue-1',
      filename: 'audio.mp3',
      filePath: '/path/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: 'proj-1',
      pipelineSnapshot: {
        isProjectPipeline: true,
        enabled: true,
        autoPolish: true,
        polishPresetId: 'proj-preset',
        autoTranslate: false,
        autoSummary: false,
        autoExport: false,
      },
    };

    const config: AppConfig = {
      language: 'en',
      enableITN: true,
      polishPresetId: 'global-preset',
    } as unknown as AppConfig;

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        historyId: 'hist-1',
        pipeline: expect.objectContaining({
          polishPresetId: 'proj-preset',
          autoPolish: true,
        }),
      }),
    );
  });
});
