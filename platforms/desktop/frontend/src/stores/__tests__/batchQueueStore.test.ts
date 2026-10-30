import { beforeEach, describe, expect, it, vi } from 'vitest';
import type * as RecognizerModule from '../../services/tauri/recognizer';
import { useTranscriptStore } from '../../test-utils/transcriptStoreTestUtils';
import type { AppConfig } from '../../types/config';
import { buildEmbyStreamUrl, deriveTitleFromUrl, useBatchQueueStore } from '../batchQueueStore';
import { useTranscriptStore as useRealTranscriptStore } from '../transcriptStore';

const taskLedgerContext = vi.hoisted(() => ({
  upsertTaskLedgerRecord: vi.fn(),
  patchTaskLedgerRecord: vi.fn(),
  isTaskLedgerCancelRequested: vi.fn(() => false),
}));

// Mock dependencies
vi.mock('uuid', () => ({
  v4: () => 'test-uuid-123',
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
  tempDir: vi.fn(() => Promise.resolve('/tmp')),
  join: vi.fn((...args) => Promise.resolve(args.join('/'))),
}));

vi.mock('@tauri-apps/plugin-fs', () => ({
  exists: vi.fn(() => Promise.resolve(false)),
  remove: vi.fn(() => Promise.resolve()),
  mkdir: vi.fn(() => Promise.resolve()),
  writeTextFile: vi.fn(() => Promise.resolve()),
  readTextFile: vi.fn(() => Promise.resolve('')),
  BaseDirectory: { AppData: 1, Resource: 2, AppLocalData: 3 },
}));

vi.mock('../../services/transcriptionService', () => ({
  transcriptionService: {
    setModelPath: vi.fn(),
    setEnableITN: vi.fn(),
    setITNModelPaths: vi.fn(),

    transcribeFile: vi.fn(),
  },
}));

vi.mock('../../services/modelService', () => ({
  PRESET_MODELS: [],
  PRESET_MODELS_MAP: new Map(),
  modelService: {
    getEnabledITNModelPaths: vi.fn(),
  },
}));

vi.mock('../projectStore', () => ({
  useProjectStore: {
    getState: () => ({
      activeProjectId: null,
      getActiveProject: vi.fn(() => null),
      getProjectById: vi.fn(() => null),
      setActiveProjectId: vi.fn().mockResolvedValue(undefined),
    }),
  },
}));

vi.mock('../../services/taskLedgerBuilders', () => ({
  buildBatchTaskLedgerRecord: (item: any, status = 'pending') => ({
    id: `batch-${item.id}`,
    kind: item.origin === 'automation' ? 'automation' : 'batchImport',
    status,
    title: item.filename,
    progress: item.progress,
    createdAt: 100,
    updatedAt: 100,
    retryable: true,
    cancelable: true,
    recoverable: false,
    filePath: item.filePath,
  }),
  createBatchTaskLedgerId: (id: string) => `batch-${id}`,
  upsertTaskLedgerRecord: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.upsertTaskLedgerRecord, undefined, args),
  patchTaskLedgerRecord: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.patchTaskLedgerRecord, undefined, args),
  isTaskLedgerCancelRequested: (...args: unknown[]) =>
    Reflect.apply(taskLedgerContext.isTaskLedgerCancelRequested, undefined, args),
}));

vi.mock('../taskLedgerStore', () => ({
  useTaskLedgerStore: {
    getState: () => ({
      requestCancel: vi.fn().mockResolvedValue(undefined),
    }),
  },
}));

vi.mock('../../services/tauri/recognizer', async (importOriginal) => {
  const actual = await importOriginal<typeof RecognizerModule>();
  return {
    ...actual,
    cancelBatchTask: vi.fn().mockResolvedValue(undefined),
  };
});

describe('batchQueueStore', () => {
  beforeEach(() => {
    useBatchQueueStore.getState().clearQueue();
    useTranscriptStore.getState().setAudioUrl(null);
    useTranscriptStore.getState().clearSegments();
    vi.clearAllMocks();
    taskLedgerContext.isTaskLedgerCancelRequested.mockReturnValue(false);
  });

  it('does not expose external batch files through the asset protocol when adding files', () => {
    const files = ['/path/to/test.wav'];

    // Action
    useBatchQueueStore.getState().addFiles(files);

    // Assert Queue State
    const queueState = useBatchQueueStore.getState();
    expect(queueState.queueItems).toHaveLength(1);
    expect(queueState.activeItemId).toBe('test-uuid-123');

    // Assert Transcript Store State
    const transcriptState = useTranscriptStore.getState();
    expect(queueState.queueItems[0].audioUrl).toBeNull();
    expect(transcriptState.audioUrl).toBeNull();
    expect(transcriptState.sourceHistoryId).toBeNull();
    expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'batch-test-uuid-123',
        kind: 'batchImport',
        status: 'pending',
        title: 'test.wav',
        progress: 0,
        filePath: '/path/to/test.wav',
      })
    );
  });

  it('keeps the active batch editor on the queue session while segments arrive', () => {
    useBatchQueueStore.getState().addFiles(['/path/to/live-result.wav']);

    const itemId = useBatchQueueStore.getState().queueItems[0].id;
    expect(useTranscriptStore.getState().activeSessionId).toBe(itemId);

    useBatchQueueStore
      .getState()
      .updateItemSegments(itemId, [
        { id: 'seg-1', text: 'Visible immediately', start: 0, end: 1, isFinal: false },
      ]);

    expect(useTranscriptStore.getState().activeSessionId).toBe(itemId);
    expect(useTranscriptStore.getState().segments).toEqual([
      expect.objectContaining({ id: 'seg-1', text: 'Visible immediately' }),
    ]);
  });

  it('records task ledger progress and failures for queue items', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: '1',
          filename: '1.wav',
          filePath: '/1.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///1.wav',
          projectId: null,
        },
      ],
      activeItemId: '1',
    });

    useBatchQueueStore.getState().updateItemStatus('1', 'processing', 55, 'transcribing');
    useBatchQueueStore.getState().setItemError('1', 'broken');

    expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenNthCalledWith(
      1,
      'batch-1',
      expect.objectContaining({
        status: 'running',
        progress: 55,
        stage: 'transcribing',
      })
    );
    expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenNthCalledWith(
      2,
      'batch-1',
      expect.objectContaining({
        status: 'failed',
        errorMessage: 'broken',
        retryable: true,
      })
    );
  });

  it('creates new batch and automation ledger records when recovered items are enqueued', () => {
    useBatchQueueStore.getState().enqueueRecoveredItems([
      {
        id: 'recovery-batch-1',
        filename: 'batch.wav',
        filePath: '/batch.wav',
        source: 'batch_import',
        resolution: 'pending',
        progress: 25,
        segments: [],
        projectId: null,
        lastKnownStage: 'transcribing',
        updatedAt: 100,
        hasSourceFile: true,
        canResume: true,
        attemptCount: 0,
        lastError: null,
        retryable: false,
      },
      {
        id: 'recovery-automation-1',
        filename: 'automation.wav',
        filePath: '/automation.wav',
        source: 'automation',
        resolution: 'pending',
        progress: 60,
        segments: [],
        projectId: 'project-1',
        lastKnownStage: 'transcribing',
        updatedAt: 101,
        hasSourceFile: true,
        canResume: true,
        attemptCount: 0,
        lastError: null,
        retryable: false,
        automationRuleId: 'rule-1',
        sourceFingerprint: 'fp-1',
      },
    ]);

    expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'batch-recovery-batch-1',
        kind: 'batchImport',
        status: 'pending',
        title: 'batch.wav',
      })
    );
    expect(taskLedgerContext.upsertTaskLedgerRecord).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'batch-recovery-automation-1',
        kind: 'automation',
        status: 'pending',
        title: 'automation.wav',
      })
    );
  });

  it('records recovered item failures on the new batch ledger record', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'recovery-automation-1',
          recoveryId: 'recovery-automation-1',
          filename: 'automation.wav',
          filePath: '/automation.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///automation.wav',
          projectId: null,
          origin: 'automation',
        },
      ],
      activeItemId: 'recovery-automation-1',
    });

    useBatchQueueStore.getState().setItemError('recovery-automation-1', 'still broken');

    expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenCalledWith(
      'batch-recovery-automation-1',
      expect.objectContaining({
        status: 'failed',
        errorMessage: 'still broken',
        retryable: true,
      })
    );
  });

  it('processes queue items when the batch ASR slot is Volcengine Doubao with no local model path', async () => {
    const { transcriptionService } = await import('../../services/transcriptionService');
    const { processBatchQueueItem } = await import('../../services/batch/batchItemProcessor');
    vi.mocked(transcriptionService.transcribeFile).mockResolvedValue([
      {
        id: 'volc-1',
        text: '云端结果',
        start: 0,
        end: 1,
        isFinal: true,
      } as any,
    ]);
    const item = {
      id: '1',
      filename: 'cloud.wav',
      filePath: '/cloud.wav',
      status: 'pending',
      progress: 0,
      segments: [],
      audioUrl: 'asset:///cloud.wav',
      projectId: null,
    } as any;
    const config = {
      language: 'auto',
      enableITN: true,
      asr: {
        selections: {
          live: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
          caption: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
          voiceTyping: { engine: 'local', mode: 'streaming', modelId: null, modelPath: '' },
          batch: {
            engine: 'online',
            mode: 'batch',
            modelId: null,
            modelPath: '',
            providerId: 'volcengine-doubao',
            profileId: 'volcengine-doubao-default',
          },
        },
        providers: {
          online: {
            'volcengine-doubao': {
              apiKey: 'volc-test-key',
              streamingEndpoint: 'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async',
              streamingResourceId: 'volc.seedasr.sauc.duration',
              batchEndpoint: 'https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash',
              batchResourceId: 'volc.bigasr.auc_turbo',
            },
          },
        },
      },
    } as any;
    const updateStatus = vi.fn();
    const updateSegments = vi.fn();

    await processBatchQueueItem({
      item,
      config,
      callbacks: {
        updateStatus,
        updateSegments,
        onHistorySaved: vi.fn(),
        onExportComplete: vi.fn(),
        isActiveItem: () => false,
        isCancelRequested: () => false,
        onInstanceIdAssigned: vi.fn(),
      },
    });

    expect(transcriptionService.setModelPath).not.toHaveBeenCalledWith('');
    expect(transcriptionService.transcribeFile).toHaveBeenCalledWith(
      '/cloud.wav',
      expect.any(Function),
      expect.any(Function),
      undefined,
      undefined,
      config,
      expect.any(Function),
      null
    );
    expect(updateSegments).toHaveBeenCalledWith([expect.objectContaining({ text: '云端结果' })]);
    expect(updateStatus).toHaveBeenCalledWith('processing', 0, 'transcribing');
  });

  it('should sync to transcript store when removing the active item', () => {
    // Setup
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: '1',
          filename: '1.wav',
          filePath: '/1.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///1.wav',
          projectId: null,
        },
        {
          id: '2',
          filename: '2.wav',
          filePath: '/2.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///2.wav',
          projectId: null,
        },
      ],
      activeItemId: '1',
    });

    // Simulate what happens when 1 is active (manually sync for setup)
    useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

    // Action: Remove active item
    useBatchQueueStore.getState().removeItem('1');

    // Assert Queue State
    const queueState = useBatchQueueStore.getState();
    expect(queueState.activeItemId).toBe('2');

    // Assert Transcript Store State
    const transcriptState = useTranscriptStore.getState();
    expect(transcriptState.audioUrl).toBe('asset:///2.wav');
  });

  it('should NOT sync to transcript store when removing a non-active item', () => {
    // Setup
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: '1',
          filename: '1.wav',
          filePath: '/1.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///1.wav',
          projectId: null,
        },
        {
          id: '2',
          filename: '2.wav',
          filePath: '/2.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///2.wav',
          projectId: null,
        },
      ],
      activeItemId: '1',
    });

    // Simulate what happens when 1 is active
    useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

    // Action: Remove non-active item
    useBatchQueueStore.getState().removeItem('2');

    // Assert Queue State
    const queueState = useBatchQueueStore.getState();
    expect(queueState.activeItemId).toBe('1'); // Should still be 1
    expect(queueState.queueItems).toHaveLength(1);
    expect(queueState.queueItems[0].id).toBe('1');

    // Assert Transcript Store State (Should NOT change)
    const transcriptState = useTranscriptStore.getState();
    expect(transcriptState.audioUrl).toBe('asset:///1.wav');
  });

  it('should clear transcript store when removing the last active item', () => {
    // Setup
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: '1',
          filename: '1.wav',
          filePath: '/1.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: 'asset:///1.wav',
          projectId: null,
        },
      ],
      activeItemId: '1',
    });

    // Simulate what happens when 1 is active
    useTranscriptStore.getState().setAudioUrl('asset:///1.wav');

    // Action: Remove active item (the only one)
    useBatchQueueStore.getState().removeItem('1');

    // Assert Queue State
    const queueState = useBatchQueueStore.getState();
    expect(queueState.activeItemId).toBeNull();
    expect(queueState.queueItems).toHaveLength(0);

    // Assert Transcript Store State (Should be cleared)
    const transcriptState = useTranscriptStore.getState();
    expect(transcriptState.audioUrl).toBeNull();
    expect(transcriptState.segments).toHaveLength(0);
  });

  it('flushes session segments back to queue cache when switching to a different item', () => {
    const seg = { id: 'seg-1', text: 'original', start: 0, end: 1, isFinal: true };

    useBatchQueueStore.setState({
      queueItems: [
        {
          id: '1',
          filename: '1.wav',
          filePath: '/1.wav',
          status: 'complete',
          progress: 100,
          segments: [seg],
          audioUrl: null,
          projectId: null,
        },
        {
          id: '2',
          filename: '2.wav',
          filePath: '/2.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: null,
          projectId: null,
        },
      ],
      activeItemId: null,
    });

    // Open session matching item 1
    useBatchQueueStore.getState().setActiveItem('1');

    // Edit the segments in the session
    useTranscriptStore
      .getState()
      .setSegments([{ id: 'seg-1', text: 'edited text', start: 0, end: 1, isFinal: true }]);

    // Switch to item 2
    useBatchQueueStore.getState().setActiveItem('2');

    // Item 1's segments in the queue cache should reflect the edit
    const item1 = useBatchQueueStore.getState().queueItems.find((i) => i.id === '1');
    expect(item1?.segments).toEqual([expect.objectContaining({ text: 'edited text' })]);
  });

  it('preserves existing switch and clear behavior regardless of active item state', () => {
    // Verifies that setActiveItem and clearQueue continue to work correctly
    // when no queue items exist (edge case)
    useBatchQueueStore.getState().setActiveItem(null);
    expect(useBatchQueueStore.getState().activeItemId).toBeNull();

    useBatchQueueStore.getState().clearQueue();
    expect(useBatchQueueStore.getState().queueItems).toHaveLength(0);
  });

  it('writes segments to dedicated session when item is not active', () => {
    // Manually set up two items with distinct IDs (uuid mock always returns the same value)
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.mp3',
          filePath: '/file1.mp3',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: null,
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'file2.mp3',
          filePath: '/file2.mp3',
          status: 'pending',
          progress: 0,
          segments: [],
          audioUrl: null,
          projectId: null,
        },
      ],
      activeItemId: 'item-1',
    });

    // Simulate: open session for the active item
    useRealTranscriptStore.getState().openSession({
      segments: [],
      sourceHistoryId: 'item-1',
      title: 'file1.mp3',
    });

    // Switch to the other item
    useBatchQueueStore.getState().setActiveItem('item-2');

    // Now updateItemSegments for item-1 while item-2 is active
    const mockSegments = [
      { id: 'seg1', text: 'Hello', start: 0, end: 1, isFinal: true },
      { id: 'seg2', text: 'World', start: 1, end: 2, isFinal: true },
    ];

    useBatchQueueStore.getState().updateItemSegments('item-1', mockSegments);

    // The item's session should have segments regardless of active status
    const session = useRealTranscriptStore.getState().sessions['item-1'];
    expect(session).toBeDefined();
    expect(session.segments).toEqual(mockSegments.map((s) => expect.objectContaining(s)));
  });

  it('activates correct transcript session when switching between processed items', () => {
    const seg1 = { id: 'seg-1', text: 'First item', start: 0, end: 1, isFinal: true };
    const seg2 = { id: 'seg-2', text: 'Second item', start: 0, end: 1, isFinal: true };

    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'file1.mp3',
          filePath: '/file1.mp3',
          status: 'complete',
          progress: 100,
          segments: [seg1],
          audioUrl: null,
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'file2.mp3',
          filePath: '/file2.mp3',
          status: 'complete',
          progress: 100,
          segments: [seg2],
          audioUrl: null,
          projectId: null,
        },
      ],
      activeItemId: null,
    });

    // Simulate processing: updateItemSegments writes to dedicated sessions
    useBatchQueueStore.getState().updateItemSegments('item-1', [seg1]);
    useBatchQueueStore.getState().updateItemSegments('item-2', [seg2]);

    // Click item 1
    useBatchQueueStore.getState().setActiveItem('item-1');
    expect(useTranscriptStore.getState().activeSessionId).toBe('item-1');
    expect(useTranscriptStore.getState().segments).toEqual([expect.objectContaining(seg1)]);

    // Click item 2
    useBatchQueueStore.getState().setActiveItem('item-2');
    expect(useTranscriptStore.getState().activeSessionId).toBe('item-2');
    expect(useTranscriptStore.getState().segments).toEqual([expect.objectContaining(seg2)]);

    // Click back to item 1
    useBatchQueueStore.getState().setActiveItem('item-1');
    expect(useTranscriptStore.getState().activeSessionId).toBe('item-1');
    expect(useTranscriptStore.getState().segments).toEqual([expect.objectContaining(seg1)]);
  });

  it('pauses and resumes queue processing', async () => {
    useBatchQueueStore.setState({
      isQueueProcessing: true,
      isQueuePaused: false,
    });

    useBatchQueueStore.getState().pauseQueue();
    expect(useBatchQueueStore.getState().isQueuePaused).toBe(true);
    expect(useBatchQueueStore.getState().isQueueProcessing).toBe(false);

    const processQueueSpy = vi.spyOn(useBatchQueueStore.getState(), 'processQueue');
    useBatchQueueStore.getState().resumeQueue();
    expect(useBatchQueueStore.getState().isQueuePaused).toBe(false);
    expect(processQueueSpy).toHaveBeenCalled();
  });

  it('retries a single failed item and triggers queue processing', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'failed-1',
          filename: 'fail.wav',
          filePath: '/fail.wav',
          status: 'error',
          progress: 30,
          errorMessage: 'Bad audio format',
          segments: [],
          audioUrl: null,
          projectId: null,
        },
      ],
      isQueueProcessing: false,
      isQueuePaused: false,
    });

    const processQueueSpy = vi
      .spyOn(useBatchQueueStore.getState(), 'processQueue')
      .mockImplementation(async () => {});
    useBatchQueueStore.getState().retryItem('failed-1');

    const updated = useBatchQueueStore.getState().queueItems[0];
    expect(updated.status).toBe('pending');
    expect(updated.progress).toBe(0);
    expect(updated.errorMessage).toBeUndefined();
    expect(processQueueSpy).toHaveBeenCalled();
  });

  it('retries all failed and cancelled items in the queue', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'f1.wav',
          filePath: '/f1.wav',
          status: 'error',
          progress: 50,
          errorMessage: 'Error 1',
          segments: [],
          audioUrl: null,
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'f2.wav',
          filePath: '/f2.wav',
          status: 'cancelled',
          progress: 20,
          segments: [],
          audioUrl: null,
          projectId: null,
        },
        {
          id: 'item-3',
          filename: 'f3.wav',
          filePath: '/f3.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          audioUrl: null,
          projectId: null,
        },
      ],
      isQueueProcessing: false,
      isQueuePaused: false,
    });
    const processQueueSpy = vi
      .spyOn(useBatchQueueStore.getState(), 'processQueue')
      .mockImplementation(async () => {});
    useBatchQueueStore.getState().retryAllFailed();
    expect(processQueueSpy).toHaveBeenCalled();

    const items = useBatchQueueStore.getState().queueItems;
    expect(items[0].status).toBe('pending');
    expect(items[0].progress).toBe(0);
    expect(items[0].errorMessage).toBeUndefined();
    expect(items[1].status).toBe('pending');
    expect(items[1].progress).toBe(0);
    expect(items[2].status).toBe('complete');
  });

  it('clears only completed items and switches active item when active item was completed', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'f1.wav',
          filePath: '/f1.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'f2.wav',
          filePath: '/f2.wav',
          status: 'pending',
          progress: 0,
          segments: [],
          projectId: null,
        },
      ],
      activeItemId: 'item-1',
      isQueueProcessing: false,
      isQueuePaused: false,
    });

    useBatchQueueStore.getState().clearCompleted();

    const state = useBatchQueueStore.getState();
    expect(state.queueItems).toHaveLength(1);
    expect(state.queueItems[0].id).toBe('item-2');
    expect(state.activeItemId).toBe('item-2');
  });

  it('retains activeItemId if active item was not completed', () => {
    useBatchQueueStore.setState({
      queueItems: [
        {
          id: 'item-1',
          filename: 'f1.wav',
          filePath: '/f1.wav',
          status: 'complete',
          progress: 100,
          segments: [],
          projectId: null,
        },
        {
          id: 'item-2',
          filename: 'f2.wav',
          filePath: '/f2.wav',
          status: 'processing',
          progress: 40,
          segments: [],
          projectId: null,
        },
      ],
      activeItemId: 'item-2',
      isQueueProcessing: true,
      isQueuePaused: false,
    });

    useBatchQueueStore.getState().clearCompleted();

    const state = useBatchQueueStore.getState();
    expect(state.queueItems).toHaveLength(1);
    expect(state.queueItems[0].id).toBe('item-2');
    expect(state.activeItemId).toBe('item-2');
  });

  describe('stream url support', () => {
    it('deriveTitleFromUrl extracts clean title from urls', () => {
      expect(deriveTitleFromUrl('https://example.com/audio/podcast.mp3?token=123')).toBe(
        'podcast.mp3'
      );
      expect(deriveTitleFromUrl('https://live.example.com/stream-live')).toBe('stream-live');
      expect(deriveTitleFromUrl('https://example.com/')).toBe('example.com');
      expect(deriveTitleFromUrl('not-a-url')).toBe('not-a-url');
      expect(deriveTitleFromUrl('https://example.com/%20/')).toBe('example.com');
      expect(deriveTitleFromUrl('https://example.com/episode%2042.mp3')).toBe('episode 42.mp3');
      expect(deriveTitleFromUrl('https://')).toBe('Network Stream');
      expect(deriveTitleFromUrl('http://192.168.1.1:8000/live#stream')).toBe('live');
    });

    it('addStreamItems enqueues url_stream items with options', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

      useBatchQueueStore
        .getState()
        .addStreamItems(['https://example.com/audio/podcast.mp3?token=123'], {
          alias: 'My Custom Podcast',
          retainAudio: true,
        });

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(1);
      expect(items[0].sourceType).toBe('url_stream');
      expect(items[0].streamUrl).toBe('https://example.com/audio/podcast.mp3?token=123');
      expect(items[0].filename).toBe('My Custom Podcast');
      expect(items[0].retainAudio).toBe(true);
      expect(items[0].status).toBe('pending');
    });

    it('addStreamItems enqueues multiple items and ignores alias for multi-item batches', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

      useBatchQueueStore
        .getState()
        .addStreamItems(
          ['https://example.com/audio/podcast1.mp3', 'https://example.com/audio/podcast2.mp3'],
          {
            alias: 'Ignored Alias',
            retainAudio: false,
          }
        );

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(2);
      expect(items[0].filename).toBe('podcast1.mp3');
      expect(items[1].filename).toBe('podcast2.mp3');
      expect(items[0].retainAudio).toBe(false);
    });
  });

  describe('emby queue and errorStage support', () => {
    it('addEmbyItems sets sourceType url_stream and streamUrl', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

      useBatchQueueStore.getState().addEmbyItems({
        items: [
          {
            serverId: 'server-1',
            serverUrl: 'http://192.168.1.10:8096',
            itemId: 'video-999',
            itemName: 'Episode 1',
            audioStreamIndex: 1,
            subtitleMode: 'bilingual',
            targetLanguage: 'zh-CN',
          },
        ],
      });

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(1);
      expect(items[0].sourceType).toBe('url_stream');
      expect(items[0].streamUrl).toBe(
        'http://192.168.1.10:8096/Videos/video-999/stream.mkv?Static=true&MediaSourceId=mediasource_video-999&AudioStreamIndex=1'
      );
      expect(items[0].filename).toBe('Episode 1');
      expect(items[0].embyContext?.itemId).toBe('video-999');
    });
    it('addEmbyItems sets streamUrl with apiKey, container, and custom mediaSourceId', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });

      useBatchQueueStore.getState().addEmbyItems({
        items: [
          {
            serverId: 'server-1',
            serverUrl: 'http://192.168.1.10:8096/emby',
            apiKey: 'token123',
            mediaSourceId: 'ms-custom-456',
            container: 'mp4',
            itemId: 'video-888',
            itemName: 'Episode 2',
            audioStreamIndex: 0,
            subtitleMode: 'pure_translation',
            targetLanguage: 'zh-CN',
          },
        ],
      });

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(1);
      expect(items[0].sourceType).toBe('url_stream');
      expect(items[0].streamUrl).toBe(
        'http://192.168.1.10:8096/emby/Videos/video-888/stream.mp4?api_key=token123&Static=true&MediaSourceId=ms-custom-456&AudioStreamIndex=0'
      );
      expect(items[0].embyContext?.mediaSourceId).toBe('ms-custom-456');
      expect(items[0].embyContext?.container).toBe('mp4');
      expect(items[0].streamUrl).toContain('api_key=token123');
      expect(items[0].embyContext && 'apiKey' in items[0].embyContext).toBe(false);
    });

    it('addEmbyItems applies sourceLanguage to resolvedConfigSnapshot', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
      useBatchQueueStore.getState().addEmbyItems({
        items: [
          {
            serverId: 'server-1',
            serverUrl: 'http://localhost:8096',
            itemId: 'video-lang',
            itemName: 'Episode with Japanese Audio',
            audioStreamIndex: 1,
            subtitleMode: 'pure_translation',
            targetLanguage: 'zh-CN',
            sourceLanguage: 'ja',
          },
        ],
        resolvedConfigSnapshot: { language: 'auto' } as unknown as AppConfig,
      });

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(1);
      expect(items[0].resolvedConfigSnapshot?.language).toBe('ja');
      expect(items[0].embyContext?.sourceLanguage).toBe('ja');
    });

    it('addEmbyItems sanitizes serverUrl in embyContext and deduplicates existing items', () => {
      useBatchQueueStore.setState({ queueItems: [], activeItemId: null });
      useBatchQueueStore.getState().addEmbyItems({
        items: [
          {
            serverId: 'server-1',
            serverUrl: 'http://localhost:8096/?api_key=token#hash',
            itemId: 'video-dup',
            itemName: 'Episode Duplicate',
            audioStreamIndex: 0,
            subtitleMode: 'pure_translation',
            targetLanguage: 'zh-CN',
          },
          {
            serverId: 'server-1',
            serverUrl: 'http://localhost:8096/',
            itemId: 'video-dup',
            itemName: 'Episode Duplicate 2',
            audioStreamIndex: 0,
            subtitleMode: 'pure_translation',
            targetLanguage: 'zh-CN',
          },
        ],
      });

      const items = useBatchQueueStore.getState().queueItems;
      expect(items).toHaveLength(1);
      expect(items[0].embyContext?.serverUrl).toBe('http://localhost:8096');
    });

    it('buildEmbyStreamUrl encodes itemId and container and strips trailing query from serverUrl', () => {
      const streamUrl = buildEmbyStreamUrl({
        serverUrl: 'http://localhost:8096/?foo=bar',
        itemId: 'item with spaces/slash',
        container: '.mkv',
      });
      expect(streamUrl).toBe(
        'http://localhost:8096/Videos/item%20with%20spaces%2Fslash/stream.mkv?Static=true&MediaSourceId=mediasource_item+with+spaces%2Fslash'
      );
    });
    it('setItemError sets errorMessage and errorStage', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'item-1',
            filename: 'Ep 1',
            filePath: '/tmp/test.mp3',
            status: 'processing',
            progress: 50,
            segments: [],
            projectId: null,
          },
        ],
      });

      useBatchQueueStore.getState().setItemError('item-1', 'Upload failed', 'upload');

      const item = useBatchQueueStore.getState().queueItems[0];
      expect(item.status).toBe('error');
      expect(item.errorMessage).toBe('Upload failed');
      expect(item.errorStage).toBe('upload');
    });

    it('retryItem preserves errorStage when moving item back to pending', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'item-failed',
            filename: 'Ep 1',
            filePath: '/tmp/test.mp3',
            status: 'error',
            progress: 95,
            segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hi', isFinal: true }],
            projectId: null,
            errorStage: 'upload',
            errorMessage: 'Emby error',
          },
        ],
      });

      useBatchQueueStore.getState().retryItem('item-failed');

      const item = useBatchQueueStore.getState().queueItems[0];
      expect(item.status).toBe('pending');
      expect(item.progress).toBe(95);
      expect(item.errorMessage).toBeUndefined();
      expect(item.errorStage).toBe('upload');
    });

    it('retryAllFailed preserves errorStage on all failed items', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'item-failed-upload',
            filename: 'Ep 1',
            filePath: '/tmp/test.mp3',
            status: 'error',
            progress: 95,
            segments: [{ id: 'seg-1', start: 0, end: 1, text: 'hi', isFinal: true }],
            projectId: null,
            errorStage: 'upload',
          },
          {
            id: 'item-failed-transcribe',
            filename: 'Ep 2',
            filePath: '/tmp/test2.mp3',
            status: 'error',
            progress: 10,
            segments: [],
            projectId: null,
            errorStage: 'transcribe',
          },
        ],
      });

      useBatchQueueStore.getState().retryAllFailed();

      const items = useBatchQueueStore.getState().queueItems;
      expect(items[0].status).toBe('pending');
      expect(items[0].errorStage).toBe('upload');
      expect(items[0].progress).toBe(95);
      expect(items[1].status).toBe('pending');
      expect(items[1].errorStage).toBe('transcribe');
      expect(items[1].progress).toBe(0);
    });
    it('updateItemFilePath updates state and task ledger', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'item-1',
            filename: 'Ep 1',
            filePath: '',
            status: 'pending',
            progress: 0,
            segments: [],
            projectId: null,
          },
        ],
      });

      useBatchQueueStore.getState().updateItemFilePath('item-1', '/downloads/ep1.mp3');

      const item = useBatchQueueStore.getState().queueItems[0];
      expect(item.filePath).toBe('/downloads/ep1.mp3');
      expect(taskLedgerContext.patchTaskLedgerRecord).toHaveBeenCalledWith(
        'batch-item-1',
        expect.objectContaining({
          filePath: '/downloads/ep1.mp3',
        })
      );
    });

    it('updateItemStatus to complete clears errorMessage and errorStage', () => {
      useBatchQueueStore.setState({
        queueItems: [
          {
            id: 'item-failed',
            filename: 'Ep 1',
            filePath: '/downloads/ep1.mp3',
            status: 'error',
            errorMessage: 'Upload failed',
            errorStage: 'upload',
            progress: 50,
            segments: [],
            projectId: null,
          },
        ],
      });

      useBatchQueueStore.getState().updateItemStatus('item-failed', 'complete', 100);

      const item = useBatchQueueStore.getState().queueItems[0];
      expect(item.status).toBe('complete');
      expect(item.errorMessage).toBeUndefined();
      expect(item.errorStage).toBeUndefined();
      expect(item.progress).toBe(100);
    });
  });
});
