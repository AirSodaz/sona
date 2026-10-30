import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEmbyHistoryStore } from '../../../stores/embyHistoryStore';
import type { BatchQueueItem } from '../../../types/batchQueue';
import type { AppConfig } from '../../../types/config';
import type { AsrTranscriptionRequest } from '../../asrConfigService';
import { downloadEmbyAudio, syncEmbySubtitles } from '../../emby/embyService';
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
  syncEmbySubtitles: vi.fn().mockResolvedValue(undefined),
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
      updateFilePath: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(downloadEmbyAudio).toHaveBeenCalledWith('movie-1', 1);
    expect(syncEmbySubtitles).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        itemId: 'movie-1',
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
      }),
      { deleteExistingIndex: 2 }
    );
    expect(useEmbyHistoryStore.getState().getContext('hist-1')).toEqual(item.embyContext);
    expect(callbacks.updateFilePath).toHaveBeenCalledWith('/tmp/downloaded_emby.mp3');
    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        pipeline: expect.objectContaining({
          autoTranslate: true,
          targetLanguage: 'zh-CN',
        }),
      })
    );
  });
  it('propagates subtitle upload error when uploadEmbySubtitle fails', async () => {
    const item: BatchQueueItem = {
      id: 'batch-emby-fail',
      filename: 'Emby Movie Fail',
      filePath: '/tmp/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-fail',
        itemName: 'Emby Movie Fail',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
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

    vi.mocked(syncEmbySubtitles).mockRejectedValueOnce(new Error('Network error on upload'));

    await expect(processor.processBatchQueueItem({ item, config, callbacks })).rejects.toThrow(
      'Network error on upload'
    );
    expect(item.errorStage).toBe('upload');
    expect(item.segments.length).toBeGreaterThan(0);
  });

  it('fast-paths upload retry without invoking audio download or transcription', async () => {
    const existingSegments = [
      { id: 'seg-1', start: 0, end: 2, text: 'Hello from previous transcription', isFinal: true },
    ];
    const item: BatchQueueItem = {
      id: 'batch-emby-fastpath',
      filename: 'Emby Movie Fastpath',
      filePath: '/tmp/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: existingSegments,
      projectId: null,
      errorStage: 'upload',
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-fastpath',
        itemName: 'Emby Movie Fastpath',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
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

    vi.mocked(downloadEmbyAudio).mockClear();
    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockClear();
    vi.mocked(syncEmbySubtitles).mockClear();
    vi.mocked(syncEmbySubtitles).mockResolvedValueOnce();

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(downloadEmbyAudio).not.toHaveBeenCalled();
    expect(mockPorts.transcriptionService.transcribeFile).not.toHaveBeenCalled();
    expect(syncEmbySubtitles).toHaveBeenCalledWith(
      existingSegments,
      expect.objectContaining({ itemId: 'movie-fastpath', targetLanguage: 'zh-CN' }),
      expect.anything()
    );
    expect(item.errorStage).toBeUndefined();
  });

  it('uses streamUrl directly as inputSource without calling downloadEmbyAudio when streamUrl is present', async () => {
    const item: BatchQueueItem = {
      id: 'batch-emby-stream',
      filename: 'Emby Movie Stream',
      filePath: '',
      sourceType: 'url_stream',
      streamUrl:
        'http://localhost:8096/Videos/movie-stream/stream.mkv?Static=true&MediaSourceId=mediasource_1',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-stream',
        itemName: 'Emby Movie Stream',
        audioStreamIndex: 2,
        subtitleMode: 'pure_translation',
        targetLanguage: 'zh-CN',
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
      updateFilePath: vi.fn(),
    };

    vi.mocked(downloadEmbyAudio).mockClear();
    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockClear();
    vi.mocked(syncEmbySubtitles).mockClear();
    vi.mocked(syncEmbySubtitles).mockResolvedValueOnce();

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(downloadEmbyAudio).not.toHaveBeenCalled();
    expect(mockPorts.transcriptionService.transcribeFile).toHaveBeenCalled();
    expect(vi.mocked(mockPorts.transcriptionService.transcribeFile).mock.calls[0][0]).toBe(
      'http://localhost:8096/Videos/movie-stream/stream.mkv?Static=true&MediaSourceId=mediasource_1'
    );
  });

  it('executes downloadEmbyAudio before transcription and uses downloadedPath when streamUrl is absent', async () => {
    const item: BatchQueueItem = {
      id: 'batch-emby-precedence',
      filename: 'Emby Movie Precedence',
      filePath: '',
      sourceType: 'file',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-prec',
        itemName: 'Emby Movie Precedence',
        audioStreamIndex: 2,
        subtitleMode: 'pure_translation',
        targetLanguage: 'zh-CN',
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
      updateFilePath: vi.fn(),
    };

    vi.mocked(downloadEmbyAudio).mockClear();
    vi.mocked(downloadEmbyAudio).mockResolvedValueOnce('/tmp/downloaded-track.mp3');
    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockClear();
    vi.mocked(syncEmbySubtitles).mockClear();
    vi.mocked(syncEmbySubtitles).mockResolvedValueOnce();

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(downloadEmbyAudio).toHaveBeenCalledWith('movie-prec', 2);
    expect(item.filePath).toBe('/tmp/downloaded-track.mp3');
    expect(mockPorts.transcriptionService.transcribeFile).toHaveBeenCalled();
    expect(vi.mocked(mockPorts.transcriptionService.transcribeFile).mock.calls[0][0]).toBe(
      '/tmp/downloaded-track.mp3'
    );
  });

  it('correctly maps existingExternalSubtitleIndex to target language in separate_tracks mode', async () => {
    const item: BatchQueueItem = {
      id: 'queue-emby-sep',
      filename: 'Emby Movie Separate',
      filePath: '/tmp/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-sep',
        itemName: 'Emby Movie Separate',
        audioStreamIndex: 1,
        subtitleMode: 'separate_tracks',
        sourceLanguage: 'en',
        targetLanguage: 'zh-CN',
        existingExternalSubtitleIndex: 5,
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

    expect(syncEmbySubtitles).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        itemId: 'movie-sep',
        subtitleMode: 'separate_tracks',
        sourceLanguage: 'en',
        targetLanguage: 'zh-CN',
      }),
      { deleteExistingIndex: 5 }
    );
  });

  it('re-registers embyContext when item has historyId on retry', async () => {
    const item: BatchQueueItem = {
      id: 'queue-emby-retry',
      filename: 'Emby Movie Retry',
      filePath: '/tmp/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      historyId: 'existing-hist-42',
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-42',
        itemName: 'Emby Movie 42',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
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

    expect(useEmbyHistoryStore.getState().getContext('existing-hist-42')).toEqual(item.embyContext);
  });

  it('preserves autoExport when item.exportConfig is provided', async () => {
    const item: BatchQueueItem = {
      id: 'queue-export-config',
      filename: 'audio.mp3',
      filePath: '/path/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: 'proj-1',
      exportConfig: {
        mode: 'original',
        format: 'srt',
        directory: '/custom/export',
      },
      pipelineSnapshot: {
        isProjectPipeline: true,
        enabled: true,
        autoPolish: false,
        autoTranslate: false,
        autoSummary: false,
        autoExport: false,
      },
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

    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        pipeline: expect.objectContaining({
          autoExport: true,
          exportDirectory: '/custom/export',
          exportFormat: 'srt',
        }),
      })
    );
  });

  it('does not call syncEmbySubtitles when callbacks.isCancelRequested() returns true', async () => {
    const item: BatchQueueItem = {
      id: 'queue-emby-cancel',
      filename: 'Emby Movie Cancelled',
      filePath: '/tmp/audio.mp3',
      status: 'pending',
      progress: 0,
      segments: [{ id: 'seg-1', start: 0, end: 10, text: 'Some text', isFinal: true }],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-cancel',
        itemName: 'Emby Movie Cancelled',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
      },
    };

    const config: AppConfig = { language: 'auto' } as unknown as AppConfig;
    let cancelRequested = false;
    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockImplementation(() => cancelRequested),
      onInstanceIdAssigned: vi.fn(),
    };

    vi.mocked(mockPorts.pipelineExecutionEngine!.execute).mockImplementation(async () => {
      cancelRequested = true;
      return {
        segments: [{ id: 'seg-1', start: 0, end: 10, text: 'Polished text', isFinal: true }],
      };
    });

    vi.mocked(syncEmbySubtitles).mockClear();

    await expect(processor.processBatchQueueItem({ item, config, callbacks })).rejects.toThrow(
      'Task cancelled.'
    );

    expect(syncEmbySubtitles).not.toHaveBeenCalled();
  });

  it('marks errorStage as transcribe when downloadEmbyAudio fails', async () => {
    const item: BatchQueueItem = {
      id: 'batch-emby-dl-fail',
      filename: 'Emby Movie Fail',
      filePath: '',
      sourceType: 'file',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-fail',
        itemName: 'Emby Movie Fail',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
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

    vi.mocked(downloadEmbyAudio).mockRejectedValueOnce(
      new Error('Network timeout downloading audio')
    );

    await expect(processor.processBatchQueueItem({ item, config, callbacks })).rejects.toThrow(
      'Network timeout downloading audio'
    );

    expect(item.errorStage).toBe('transcribe');
  });

  it('passes local item.filePath and non-stream flag to saveImportedFile when item has embyContext', async () => {
    const item: BatchQueueItem = {
      id: 'batch-emby-save-history',
      filename: 'Emby Movie History',
      filePath: '',
      sourceType: 'file',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      embyContext: {
        serverId: 'server-1',
        serverUrl: 'http://localhost:8096',
        itemId: 'movie-hist',
        itemName: 'Emby Movie History',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
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
      updateFilePath: vi.fn(),
    };

    vi.mocked(downloadEmbyAudio).mockResolvedValueOnce('/tmp/downloaded-emby-history.mp3');
    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockResolvedValueOnce([
      { id: 'seg-1', start: 0, end: 5, text: 'Hello world', isFinal: true },
    ]);
    vi.mocked(mockPorts.historyService.saveImportedFileToProject).mockClear();

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockPorts.historyService.saveImportedFileToProject).toHaveBeenCalledWith(
      '/tmp/downloaded-emby-history.mp3',
      expect.any(Array),
      5,
      undefined,
      null,
      'batch-emby-save-history'
    );
  });
  it('performs concurrent streaming translation during transcribeFile and reconciles translations onto final segments', async () => {
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segs: Array<{ id: string; text: string }>) => {
        return segs.map((s) => ({ id: s.id, translation: `Translated: ${s.text}` }));
      });
    mockPorts.translationService = {
      translateSegmentsWithConfig: mockTranslate,
    } as unknown as BatchItemProcessorPorts['translationService'];

    const emittedSegments = Array.from({ length: 12 }, (_, i) => ({
      id: `seg-${i + 1}`,
      start: i,
      end: i + 1,
      text: `Hello ${i + 1}`,
      isFinal: true,
    }));

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockImplementationOnce(
      async (_src, _onProgress, onSegment) => {
        for (const seg of emittedSegments) {
          onSegment?.(seg);
        }
        return emittedSegments;
      }
    );

    const item: BatchQueueItem = {
      id: 'queue-concurrent-1',
      filename: 'video.mp4',
      filePath: '/path/video.mp4',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      pipelineSnapshot: {
        isProjectPipeline: false,
        enabled: true,
        autoPolish: false,
        autoTranslate: true,
        targetLanguage: 'zh-CN',
        autoSummary: false,
        autoExport: false,
      },
    };

    const config: AppConfig = {
      language: 'auto',
      translationLanguage: 'zh-CN',
    } as unknown as AppConfig;

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
      updateFilePath: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockTranslate).toHaveBeenCalled();
    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        segments: expect.arrayContaining([
          expect.objectContaining({ id: 'seg-1', translation: 'Translated: Hello 1' }),
          expect.objectContaining({ id: 'seg-12', translation: 'Translated: Hello 12' }),
        ]),
      })
    );
  });

  it('bypasses streaming translation when autoPolish is enabled', async () => {
    const mockTranslate = vi.fn().mockResolvedValue([]);
    mockPorts.translationService = {
      translateSegmentsWithConfig: mockTranslate,
    } as unknown as BatchItemProcessorPorts['translationService'];

    const emittedSegments = [
      { id: 'seg-1', start: 0, end: 1, text: 'Hello 1', isFinal: true },
      { id: 'seg-2', start: 1, end: 2, text: 'Hello 2', isFinal: true },
    ];

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockImplementationOnce(
      async (_src, _onProgress, onSegment) => {
        for (const seg of emittedSegments) {
          onSegment?.(seg);
        }
        return emittedSegments;
      }
    );

    const item: BatchQueueItem = {
      id: 'queue-polish-bypass',
      filename: 'video.mp4',
      filePath: '/path/video.mp4',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      pipelineSnapshot: {
        isProjectPipeline: false,
        enabled: true,
        autoPolish: true,
        autoTranslate: true,
        targetLanguage: 'zh-CN',
        autoSummary: false,
        autoExport: false,
      },
    };

    const config: AppConfig = {
      language: 'auto',
      translationLanguage: 'zh-CN',
    } as unknown as AppConfig;

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: () => false,
      onInstanceIdAssigned: vi.fn(),
      updateFilePath: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    expect(mockTranslate).not.toHaveBeenCalled();
  });

  it('does not push non-final intermediate segments to streaming translation scheduler until finalized', async () => {
    const translatedTexts: string[] = [];
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segs: Array<{ id: string; text: string }>) => {
        for (const s of segs) {
          translatedTexts.push(s.text);
        }
        return segs.map((s) => ({ id: s.id, translation: `Translated: ${s.text}` }));
      });
    mockPorts.translationService = {
      translateSegmentsWithConfig: mockTranslate,
    } as unknown as BatchItemProcessorPorts['translationService'];

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockImplementationOnce(
      async (_src, _onProgress, onSegment) => {
        // Emit non-final draft
        onSegment?.({
          id: 'seg-1',
          start: 0,
          end: 1,
          text: 'Hel...',
          isFinal: false,
        });
        // Emit final segment
        onSegment?.({
          id: 'seg-1',
          start: 0,
          end: 2,
          text: 'Hello world',
          isFinal: true,
        });
        return [{ id: 'seg-1', start: 0, end: 2, text: 'Hello world', isFinal: true }];
      }
    );

    const item: BatchQueueItem = {
      id: 'queue-nonfinal-1',
      filename: 'audio.wav',
      filePath: '/path/audio.wav',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      pipelineSnapshot: {
        isProjectPipeline: false,
        enabled: true,
        autoPolish: false,
        autoTranslate: true,
        targetLanguage: 'zh-CN',
        autoSummary: false,
        autoExport: false,
      },
    };

    const config: AppConfig = {
      language: 'auto',
      translationLanguage: 'zh-CN',
    } as unknown as AppConfig;

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
      updateFilePath: vi.fn(),
    };

    await processor.processBatchQueueItem({ item, config, callbacks });

    // Verify draft was not translated; only final text was translated
    expect(translatedTexts).not.toContain('Hel...');
    expect(translatedTexts).toContain('Hello world');
    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        segments: [
          expect.objectContaining({
            id: 'seg-1',
            text: 'Hello world',
            translation: 'Translated: Hello world',
          }),
        ],
      })
    );
  });

  it('retains and drains streaming translation scheduler on graceful URL stream stop', async () => {
    const mockTranslate = vi
      .fn()
      .mockImplementation(async (_cfg, segs: Array<{ id: string; text: string }>) => {
        return segs.map((s) => ({ id: s.id, translation: `Translated: ${s.text}` }));
      });
    mockPorts.translationService = {
      translateSegmentsWithConfig: mockTranslate,
    } as unknown as BatchItemProcessorPorts['translationService'];

    vi.mocked(mockPorts.transcriptionService.transcribeFile).mockImplementationOnce(
      async (_src, _onProgress, onSegment) => {
        onSegment?.({
          id: 'seg-stream-1',
          start: 0,
          end: 3,
          text: 'Stream chunk 1',
          isFinal: true,
        });
        // Graceful stream stop terminates with Task cancelled error
        throw new Error('Task cancelled.');
      }
    );

    const item: BatchQueueItem = {
      id: 'queue-stream-translate',
      filename: 'live.m3u8',
      filePath: 'https://example.com/live.m3u8',
      sourceType: 'url_stream',
      status: 'pending',
      progress: 0,
      segments: [],
      projectId: null,
      pipelineSnapshot: {
        isProjectPipeline: false,
        enabled: true,
        autoPolish: false,
        autoTranslate: true,
        targetLanguage: 'zh-CN',
        autoSummary: false,
        autoExport: false,
      },
    };

    const config: AppConfig = {
      language: 'auto',
      translationLanguage: 'zh-CN',
    } as unknown as AppConfig;

    const callbacks = {
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
      updateFilePath: vi.fn(),
    };

    await expect(
      processor.processBatchQueueItem({ item, config, callbacks })
    ).resolves.toBeUndefined();

    expect(mockTranslate).toHaveBeenCalled();
    expect(mockPorts.pipelineExecutionEngine?.execute).toHaveBeenCalledWith(
      expect.objectContaining({
        segments: [
          expect.objectContaining({
            id: 'seg-stream-1',
            translation: 'Translated: Stream chunk 1',
          }),
        ],
      })
    );
  });
});
