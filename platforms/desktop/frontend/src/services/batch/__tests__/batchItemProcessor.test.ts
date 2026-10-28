import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEmbyHistoryStore } from '../../../stores/embyHistoryStore';
import type { BatchQueueItem } from '../../../types/batchQueue';
import type { AppConfig } from '../../../types/config';
import type { AsrTranscriptionRequest } from '../../asrConfigService';
import { downloadEmbyAudio, uploadEmbySubtitle } from '../../emby/embyService';
import { BatchItemProcessor, type BatchItemProcessorPorts } from '../batchItemProcessor';

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
vi.mock('../../emby/embyService', () => ({
  downloadEmbyAudio: vi.fn().mockResolvedValue('/tmp/downloaded_emby.mp3'),
  uploadEmbySubtitle: vi.fn().mockResolvedValue(undefined),
  generateBilingualSrt: vi.fn().mockReturnValue('1\n00:00:00,000 --> 00:00:05,000\n你好\nHello'),
  generatePureTranslationSrt: vi.fn().mockReturnValue('1\n00:00:00,000 --> 00:00:05,000\n你好'),
  generateOriginalSrt: vi.fn().mockReturnValue('1\n00:00:00,000 --> 00:00:05,000\nHello'),
}));

describe('BatchItemProcessor', () => {
  let mockPorts: BatchItemProcessorPorts;
  let processor: BatchItemProcessor;

  beforeEach(() => {
    mockPorts = {
      transcriptionService: {
        transcribeFile: vi
          .fn()
          .mockResolvedValue([{ id: 'seg-1', start: 0, end: 10, text: 'Raw transcription' }]),
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
      onInstanceIdAssigned: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        historyId: 'hist-1',
        pipeline: expect.objectContaining({
          polishPresetId: 'proj-preset',
          autoPolish: true,
        }),
      })
    );
  });

  it('does not save history when cancellation error occurs', async () => {
    const item: BatchQueueItem = {
      id: 'queue-cancel',
      filename: 'audio.mp3',
      filePath: '/path/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
    };

    const config: AppConfig = {
      language: 'en',
    } as unknown as AppConfig;

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockRejectedValue(
      new Error('Task cancelled.')
    );

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(true),
      onInstanceIdAssigned: vi.fn(),
    };

    await expect(processor.processBatchQueueItem({ item, config, callbacks })).rejects.toThrow(
      'Task cancelled.'
    );

    expect(mockPorts.historyService.saveImportedFile).not.toHaveBeenCalled();
    expect(mockPorts.historyService.saveImportedFileToProject).not.toHaveBeenCalled();
    expect(callbacks.onHistorySaved).not.toHaveBeenCalled();
  });

  it('preserves custom alias when saving stream history item', async () => {
    const updateItemMetaMock = vi.fn().mockResolvedValue(undefined);
    vi.mocked(mockPorts.useHistoryStore.getState).mockReturnValue({
      addItem: vi.fn(),
      updateTranscript: vi.fn(),
      updateItemMeta: updateItemMetaMock,
    } as unknown as never);

    const item: BatchQueueItem = {
      id: 'queue-stream-1',
      filename: 'Custom Podcast Title',
      filePath: 'https://example.com/audio/stream.mp3',
      sourceType: 'url_stream',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
    };

    const config: AppConfig = { language: 'en' } as unknown as AppConfig;
    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(updateItemMetaMock).toHaveBeenCalledWith('hist-1', {
      title: 'Custom Podcast Title',
    });
    expect(callbacks.onHistorySaved).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'hist-1',
        title: 'Custom Podcast Title',
      })
    );
  });

  it('gracefully completes and executes pipeline when stream is stopped via Finish and Save', async () => {
    const item: BatchQueueItem = {
      id: 'queue-stream-finish',
      filename: 'Live Stream',
      filePath: 'https://live.example.com/stream.m3u8',
      sourceType: 'url_stream',
      status: 'processing',
      progress: 50,
      segments: [],
      projectId: null,
    };

    const config: AppConfig = { language: 'en' } as unknown as AppConfig;

    // Simulate transcribeFile streaming chunks and then terminating on cancelBatchTask
    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockImplementation(
      async (_input, _onProgress, onSegment) => {
        if (onSegment) {
          onSegment({ id: 'seg-1', start: 0, end: 5, text: 'Stream speech chunk', isFinal: true });
        }
        throw new Error('Task cancelled.');
      }
    );

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false), // User did NOT cancel the queue item
      onInstanceIdAssigned: vi.fn(),
    };

    await expect(
      processor.processBatchQueueItem({ item, config, callbacks })
    ).resolves.toBeUndefined();
    expect(mockPorts.historyService.saveImportedFileToProject).toHaveBeenCalled();
    expect(callbacks.onHistorySaved).toHaveBeenCalled();
    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalled();
  });

  it('rethrows Task cancelled when stream is cancelled with zero segments', async () => {
    const item: BatchQueueItem = {
      id: 'queue-stream-empty',
      filename: 'Empty Stream',
      filePath: 'https://live.example.com/empty.m3u8',
      sourceType: 'url_stream',
      status: 'processing',
      progress: 0,
      segments: [],
      projectId: null,
    };

    const config: AppConfig = { language: 'en' } as unknown as AppConfig;

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockRejectedValue(
      new Error('Task cancelled.')
    );

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
    };

    await expect(processor.processBatchQueueItem({ item, config, callbacks })).rejects.toThrow(
      'Task cancelled.'
    );
    expect(mockPorts.historyService.saveImportedFileToProject).not.toHaveBeenCalled();
    expect(callbacks.onInstanceIdAssigned).toHaveBeenCalledWith(null);
  });

  it('passes convertedPath when retainAudio is true for stream', async () => {
    const item: BatchQueueItem = {
      id: 'queue-stream-retain',
      filename: 'Retained Audio Stream',
      filePath: 'https://live.example.com/audio.m3u8',
      sourceType: 'url_stream',
      retainAudio: true,
      status: 'processing',
      progress: 0,
      segments: [],
      projectId: null,
    };

    const config: AppConfig = { language: 'en' } as unknown as AppConfig;
    vi.mocked(mockPorts.asrConfigService.resolveAsrTranscriptionRequest).mockReturnValue({
      engine: 'local',
      mode: 'batch',
      modelPath: '/path/to/model',
    } as unknown as AsrTranscriptionRequest);

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockResolvedValue([
      { id: 'seg-1', start: 0, end: 2, text: 'Hello', isFinal: true },
    ]);

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockPorts.historyService.saveImportedFileToProject).toHaveBeenCalledWith(
      'https://live.example.com/audio.m3u8',
      expect.any(Array),
      2,
      expect.stringContaining('.wav'),
      null,
      'queue-stream-retain'
    );
  });

  it('downloads audio and uploads subtitles when embyContext is present', async () => {
    const item: BatchQueueItem = {
      id: 'queue-emby-1',
      filename: 'Emby Movie',
      filePath: '',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-1',
        itemName: 'Emby Movie',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
        existingExternalSubtitleIndex: 2,
      },
    };

    const config: AppConfig = { language: 'auto' } as unknown as AppConfig;
    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(downloadEmbyAudio).toHaveBeenCalledWith('movie-1', 1);
    expect(uploadEmbySubtitle).toHaveBeenCalledWith(
      expect.objectContaining({
        itemId: 'movie-1',
        language: 'zh-CN',
        format: 'srt',
        content: expect.stringContaining('你好'),
      }),
      2
    );
    expect(useEmbyHistoryStore.getState().getContext('hist-1')).toEqual(item.embyContext);
  });
});
