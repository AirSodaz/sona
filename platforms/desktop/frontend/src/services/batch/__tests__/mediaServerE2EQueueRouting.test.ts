// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useBatchQueueStore } from '../../../stores/batchQueueStore';
import { useMediaServerHistoryStore } from '../../../stores/mediaServerHistoryStore';
import type { AppConfig } from '../../../types/config';
import {
  downloadMediaServerAudio,
  syncMediaServerSubtitles,
} from '../../mediaServer/mediaServerService';
import { BatchItemProcessor, type BatchItemProcessorPorts } from '../batchItemProcessor';

vi.mock('@tauri-apps/api/path', () => ({
  tempDir: vi.fn(() => Promise.resolve('/tmp')),
  join: vi.fn((...args: string[]) => Promise.resolve(args.join('/'))),
}));
vi.mock('../../../services/recoveryService', () => ({
  persistQueueRecoverySnapshot: vi.fn(),
  toBatchQueueItem: vi.fn((x) => x),
}));

vi.mock('../../mediaServer/mediaServerService', () => ({
  downloadMediaServerAudio: vi
    .fn()
    .mockImplementation((itemId: string, _streamIndex?: number, serverId?: string) => {
      return Promise.resolve(`/tmp/downloaded_${serverId}_${itemId}.mp3`);
    }),
  syncMediaServerSubtitles: vi.fn().mockResolvedValue(undefined),
}));

describe('End-to-End Media Server Queue Multi-Profile Routing', () => {
  let mockPorts: BatchItemProcessorPorts;
  let processor: BatchItemProcessor;

  beforeEach(() => {
    vi.clearAllMocks();
    useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
    useMediaServerHistoryStore.setState({ contexts: {} });

    mockPorts = {
      transcriptionService: {
        transcribeFile: vi
          .fn()
          .mockResolvedValue([
            { id: 'seg-1', start: 0, end: 5, text: 'Hello world', isFinal: true },
          ]),
        setModelPath: vi.fn(),
        setEnableITN: vi.fn(),
      } as unknown as BatchItemProcessorPorts['transcriptionService'],
      historyService: {
        saveImportedFile: vi
          .fn()
          .mockImplementation((_source, _segs, _dur, _wav, _proj, queueId) => {
            return Promise.resolve({
              id: `hist-for-${queueId}`,
              title: 'Saved History Item',
              timestamp: Date.now(),
              duration: 5,
              audioPath: '',
              audioStatus: 'ready',
            });
          }),
      } as unknown as BatchItemProcessorPorts['historyService'],
      useHistoryStore: {
        getState: () => ({
          addItem: vi.fn(),
          updateTranscript: vi.fn().mockResolvedValue(undefined),
        }),
      } as unknown as BatchItemProcessorPorts['useHistoryStore'],
      asrConfigService: {
        resolveAsrTranscriptionRequest: vi.fn().mockReturnValue({
          engine: 'online',
          mode: 'batch',
          providerId: 'volcengine-doubao',
        }),
        isAsrRequestConfigured: vi.fn().mockReturnValue(true),
        buildHotwordsWithPipeline: vi.fn().mockReturnValue(undefined),
      } as unknown as BatchItemProcessorPorts['asrConfigService'],
      pipelineExecutionEngine: {
        execute: vi.fn().mockResolvedValue({
          segments: [
            {
              id: 'seg-1',
              start: 0,
              end: 5,
              text: 'Hello world',
              translation: '你好世界',
              isFinal: true,
            },
          ],
        }),
      } as unknown as BatchItemProcessorPorts['pipelineExecutionEngine'],
      summaryService: {
        persistSummary: vi.fn().mockResolvedValue(undefined),
      } as unknown as BatchItemProcessorPorts['summaryService'],
    } as unknown as BatchItemProcessorPorts;

    processor = new BatchItemProcessor(mockPorts);
  });

  it('routes Profile 1 (Emby) item to Profile 1, and Profile 2 (Jellyfin) item to Profile 2', async () => {
    // 1. Enqueue Profile 1 (Emby) item and Profile 2 (Jellyfin) item
    useBatchQueueStore.getState().addMediaServerItems({
      items: [
        {
          serverId: 'server-emby-1',
          serverType: 'emby',
          serverUrl: 'http://192.168.1.100:8096',
          apiKey: 'emby-secret-key',
          itemId: 'movie-emby-101',
          itemName: 'Inception (Emby)',
          audioStreamIndex: 1,
          subtitleMode: 'bilingual',
          targetLanguage: 'zh-CN',
          existingExternalSubtitleIndex: 3,
        },
        {
          serverId: 'server-jellyfin-2',
          serverType: 'jellyfin',
          serverUrl: 'https://jellyfin.domain.com',
          apiKey: 'jellyfin-secret-key',
          itemId: 'movie-jf-202',
          itemName: 'Interstellar (Jellyfin)',
          audioStreamIndex: 0,
          subtitleMode: 'pure_translation',
          targetLanguage: 'en',
          existingExternalSubtitleIndex: undefined,
        },
      ],
    });

    const queue = useBatchQueueStore.getState().queueItems;
    expect(queue).toHaveLength(2);
    const embyItem = queue[0];
    const jellyfinItem = queue[1];

    expect(embyItem.mediaServerContext?.serverId).toBe('server-emby-1');
    expect(embyItem.mediaServerContext?.serverType).toBe('emby');
    expect(jellyfinItem.mediaServerContext?.serverId).toBe('server-jellyfin-2');
    expect(jellyfinItem.mediaServerContext?.serverType).toBe('jellyfin');

    const config: AppConfig = { language: 'auto' } as unknown as AppConfig;
    const createCallbacks = () => ({
      updateStatus: vi.fn(),
      updateSegments: vi.fn(),
      onHistorySaved: vi.fn(),
      onExportComplete: vi.fn(),
      isActiveItem: vi.fn().mockReturnValue(true),
      isCancelRequested: vi.fn().mockReturnValue(false),
      onInstanceIdAssigned: vi.fn(),
      updateFilePath: vi.fn(),
    });

    // 2. Process Emby Item
    // Clear streamUrl to trigger download audio path
    embyItem.streamUrl = undefined;
    const embyCallbacks = createCallbacks();
    await processor.processBatchQueueItem({ item: embyItem, config, callbacks: embyCallbacks });

    // Verify audio download and subtitle upload routed strictly to server-emby-1
    expect(downloadMediaServerAudio).toHaveBeenCalledWith('movie-emby-101', 1, 'server-emby-1');
    expect(syncMediaServerSubtitles).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        serverId: 'server-emby-1',
        serverType: 'emby',
        itemId: 'movie-emby-101',
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
      }),
      { deleteExistingIndex: 3 }
    );
    expect(
      useMediaServerHistoryStore.getState().getContext(`hist-for-${embyItem.id}`)?.serverId
    ).toBe('server-emby-1');

    // 3. Process Jellyfin Item
    jellyfinItem.streamUrl = undefined;
    const jfCallbacks = createCallbacks();
    await processor.processBatchQueueItem({ item: jellyfinItem, config, callbacks: jfCallbacks });

    // Verify audio download and subtitle upload routed strictly to server-jellyfin-2
    expect(downloadMediaServerAudio).toHaveBeenCalledWith('movie-jf-202', 0, 'server-jellyfin-2');
    expect(syncMediaServerSubtitles).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        serverId: 'server-jellyfin-2',
        serverType: 'jellyfin',
        itemId: 'movie-jf-202',
        subtitleMode: 'pure_translation',
        targetLanguage: 'en',
      }),
      { deleteExistingIndex: undefined }
    );
    expect(
      useMediaServerHistoryStore.getState().getContext(`hist-for-${jellyfinItem.id}`)?.serverId
    ).toBe('server-jellyfin-2');
  });
});
