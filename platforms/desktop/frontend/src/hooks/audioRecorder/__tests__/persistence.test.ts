import { describe, expect, it, vi } from 'vitest';
import type { LiveRecordingDraftHandle } from '../../../services/historyService';
import type { HistoryItem } from '../../../types/history';
import { createRecordingPersistence } from '../persistence';
import type { RecordingPersistenceTranscriptState } from '../types';

function createHistoryItem(id: string): HistoryItem {
  return {
    id,
    timestamp: 1,
    duration: 3,
    audioPath: `${id}.wav`,
    transcriptPath: `${id}.json`,
    title: `Recording ${id}`,
    previewText: 'preview',
    projectId: 'project-1',
    icon: 'system:mic',
    type: 'recording',
    searchContent: 'preview',
    status: 'complete',
  };
}

function createDraftHandle(id: string, extension = 'wav'): LiveRecordingDraftHandle {
  return {
    item: {
      ...createHistoryItem(id),
      audioPath: `${id}.${extension}`,
      transcriptPath: `${id}.json`,
      duration: 0,
      previewText: '',
      searchContent: '',
      status: 'draft',
      draftSource: 'live_record',
    },
    audioAbsolutePath: `C:/history/${id}.${extension}`,
  };
}

describe('createRecordingPersistence', () => {
  it('creates live drafts immediately and finalizes the same history ids for browser and native recordings', async () => {
    const transcriptState: RecordingPersistenceTranscriptState = {
      config: {} as RecordingPersistenceTranscriptState['config'],
      segments: [{ id: 'seg-1', text: 'hello', start: 0, end: 1, isFinal: true }],
      setAudioUrl: vi.fn(),
      setSegments: vi.fn(),
    };

    const addHistoryItem = vi.fn();
    const upsertHistoryItem = vi.fn();
    const removeHistoryItem = vi.fn();
    const syncSavedRecordingMeta = vi.fn();
    const persistSummary = vi.fn().mockResolvedValue(undefined);
    const createLiveRecordingDraft = vi
      .fn()
      .mockResolvedValueOnce(createDraftHandle('web-history', 'webm'))
      .mockResolvedValueOnce(createDraftHandle('native-history', 'wav'));
    const completeLiveRecordingDraft = vi
      .fn()
      .mockResolvedValueOnce(createHistoryItem('web-history'))
      .mockResolvedValueOnce(createHistoryItem('native-history'));
    const removeFile = vi.fn().mockResolvedValue(undefined);
    const writeFile = vi.fn().mockResolvedValue(undefined);

    const discardLiveRecordingDraft = vi.fn().mockResolvedValue(undefined);
    const persistence = createRecordingPersistence({
      logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
      },
      history: {
        createLiveRecordingDraft,
        completeLiveRecordingDraft,
        discardLiveRecordingDraft,
        saveRecording: vi.fn(),
        saveNativeRecording: vi.fn(),
      },
      getTranscriptState: () => transcriptState,
      getActiveProjectId: () => 'project-1',
      addHistoryItem,
      upsertHistoryItem,
      removeHistoryItem,
      persistSummary,
      annotateSegmentsForFile: vi.fn(async (_filePath, segments) => segments),
      syncSavedRecordingMeta,
      writeFile,
      removeFile,
      fileSrcFromPath: (filePath) => `file://${filePath}`,
    });

    const webDraft = await persistence.createLiveRecordingDraft('webm');
    const nativeDraft = await persistence.createLiveRecordingDraft('wav');

    await persistence.persistBrowserRecording(
      webDraft,
      new Blob(['web'], { type: 'audio/webm' }),
      3
    );
    await persistence.persistNativeRecording(nativeDraft, 'native.wav', 3);

    expect(createLiveRecordingDraft).toHaveBeenCalledTimes(2);
    expect(completeLiveRecordingDraft).toHaveBeenNthCalledWith(
      1,
      'web-history',
      transcriptState.segments,
      3
    );
    expect(completeLiveRecordingDraft).toHaveBeenNthCalledWith(
      2,
      'native-history',
      transcriptState.segments,
      3
    );
    expect(addHistoryItem).toHaveBeenNthCalledWith(1, webDraft.item);
    expect(addHistoryItem).toHaveBeenNthCalledWith(2, nativeDraft.item);
    expect(upsertHistoryItem).toHaveBeenNthCalledWith(1, createHistoryItem('web-history'));
    expect(upsertHistoryItem).toHaveBeenNthCalledWith(2, createHistoryItem('native-history'));
    expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(
      1,
      'Recording web-history',
      'web-history',
      'system:mic'
    );
    expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(
      2,
      'Recording native-history',
      'native-history',
      'system:mic'
    );
    expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(
      3,
      'Recording web-history',
      'web-history',
      'system:mic'
    );
    expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(
      4,
      'Recording native-history',
      'native-history',
      'system:mic'
    );
    expect(persistSummary).toHaveBeenNthCalledWith(1, 'web-history');
    expect(persistSummary).toHaveBeenNthCalledWith(2, 'native-history');
    expect(persistSummary).toHaveBeenNthCalledWith(3, 'web-history');
    expect(persistSummary).toHaveBeenNthCalledWith(4, 'native-history');
    expect(createLiveRecordingDraft).toHaveBeenNthCalledWith(
      1,
      'webm',
      'project-1',
      'system:mic',
      undefined
    );
    expect(createLiveRecordingDraft).toHaveBeenNthCalledWith(
      2,
      'wav',
      'project-1',
      'system:mic',
      undefined
    );
    expect(writeFile).toHaveBeenCalledWith('C:/history/web-history.webm', expect.any(Uint8Array));
    expect(discardLiveRecordingDraft).not.toHaveBeenCalled();
    expect(removeHistoryItem).not.toHaveBeenCalled();
    expect(removeFile).not.toHaveBeenCalled();
    expect(transcriptState.setAudioUrl).toHaveBeenNthCalledWith(
      1,
      'file://C:/history/web-history.webm'
    );
    expect(transcriptState.setAudioUrl).toHaveBeenNthCalledWith(2, 'file://native.wav');
  });

  it('purges empty live recording drafts before removing their workspace badges', async () => {
    const transcriptState: RecordingPersistenceTranscriptState = {
      config: {} as RecordingPersistenceTranscriptState['config'],
      segments: [],
      setAudioUrl: vi.fn(),
      setSegments: vi.fn(),
    };

    const addHistoryItem = vi.fn();
    const upsertHistoryItem = vi.fn();
    const removeHistoryItem = vi.fn();
    const syncSavedRecordingMeta = vi.fn();
    const persistSummary = vi.fn().mockResolvedValue(undefined);
    const createLiveRecordingDraft = vi
      .fn()
      .mockResolvedValueOnce(createDraftHandle('empty-web-history', 'webm'))
      .mockResolvedValueOnce(createDraftHandle('empty-native-history', 'wav'));
    const completeLiveRecordingDraft = vi.fn();
    const removeFile = vi.fn().mockResolvedValue(undefined);
    const writeFile = vi.fn().mockResolvedValue(undefined);

    const discardLiveRecordingDraft = vi.fn().mockResolvedValue(undefined);
    const persistence = createRecordingPersistence({
      logger: {
        info: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
      },
      history: {
        createLiveRecordingDraft,
        completeLiveRecordingDraft,
        discardLiveRecordingDraft,
        saveRecording: vi.fn(),
        saveNativeRecording: vi.fn(),
      },
      getTranscriptState: () => transcriptState,
      getActiveProjectId: () => 'project-1',
      addHistoryItem,
      upsertHistoryItem,
      removeHistoryItem,
      persistSummary,
      annotateSegmentsForFile: vi.fn(async (_filePath, segments) => segments),
      syncSavedRecordingMeta,
      writeFile,
      removeFile,
      fileSrcFromPath: (filePath) => `file://${filePath}`,
    });

    const webDraft = await persistence.createLiveRecordingDraft('webm');
    const nativeDraft = await persistence.createLiveRecordingDraft('wav');

    await persistence.persistBrowserRecording(
      webDraft,
      new Blob(['web'], { type: 'audio/webm' }),
      3
    );
    await persistence.persistNativeRecording(nativeDraft, 'native.wav', 3);

    expect(discardLiveRecordingDraft).toHaveBeenNthCalledWith(1, 'empty-web-history');
    expect(discardLiveRecordingDraft).toHaveBeenNthCalledWith(2, 'empty-native-history');
    expect(removeHistoryItem).toHaveBeenNthCalledWith(1, 'empty-web-history');
    expect(removeHistoryItem).toHaveBeenNthCalledWith(2, 'empty-native-history');
    expect(completeLiveRecordingDraft).not.toHaveBeenCalled();
    expect(upsertHistoryItem).not.toHaveBeenCalled();
    expect(writeFile).not.toHaveBeenCalled();
    expect(removeFile).toHaveBeenCalledWith('native.wav');
  });

  it('detaches pipeline and background speaker annotation after draft archival under quitGuard while merging speaker labels without clobbering edits', async () => {
    const { hasActiveBackgroundQuitTasks } = await import('../../../services/quitGuard');
    const { selectIsDocumentEditorLocked, useDocumentMutationLockStore } = await import(
      '../../../services/task/documentMutationCoordinator'
    );
    const { resetTranscriptStores, useTranscriptStore } = await import(
      '../../../test-utils/transcriptStoreTestUtils'
    );

    resetTranscriptStores();
    const initialSegments = [
      { id: 'seg-1', start: 0, end: 3, text: 'Initial raw text', isFinal: true },
    ];
    useTranscriptStore.setState({
      activeSessionId: 'native-history',
      sessions: {
        default: { ...useTranscriptStore.getState().sessions.default },
        'native-history': {
          ...useTranscriptStore.getState().sessions.default,
          sourceHistoryId: 'native-history',
          segments: initialSegments,
        },
      },
    });

    const postProcessDeferred = Promise.withResolvers<void>();
    const annotateDeferred = Promise.withResolvers<any[]>();
    const commitTranscriptUpdate = vi.fn().mockResolvedValue(undefined);

    const draftItem: HistoryItem = {
      id: 'native-history',
      timestamp: 1000,
      duration: 5,
      audioPath: 'history/native-history.wav',
      transcriptPath: 'transcripts/native-history.json',
      previewText: '',
      title: 'Recording native-history',
      type: 'recording',
      icon: 'system:mic',
      status: 'complete',
      projectId: 'proj-1',
      tagIds: ['proj-1'],
    };

    const persistence = createRecordingPersistence({
      logger: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
      history: {
        createLiveRecordingDraft: vi.fn(),
        completeLiveRecordingDraft: vi.fn().mockResolvedValue({
          ...draftItem,
          status: 'complete',
        }),
        discardLiveRecordingDraft: vi.fn(),
        saveNativeRecordingAudio: vi.fn().mockResolvedValue({
          ...draftItem,
          status: 'complete',
        }),
        loadTranscript: vi.fn().mockResolvedValue(null),
      } as any,
      getTranscriptState: () =>
        useTranscriptStore.getState() as unknown as RecordingPersistenceTranscriptState,
      getActiveProjectId: () => 'proj-1',
      addHistoryItem: vi.fn(),
      upsertHistoryItem: vi.fn(),
      removeHistoryItem: vi.fn(),
      persistSummary: vi.fn().mockResolvedValue(undefined),
      postProcessSavedItem: vi.fn(async (_id, segments) => {
        await postProcessDeferred.promise;
        return segments;
      }),
      commitTranscriptUpdate,
      annotateSegmentsForFile: vi.fn(() => annotateDeferred.promise),
      syncSavedRecordingMeta: vi.fn(),
      writeFile: vi.fn().mockResolvedValue(undefined),
      removeFile: vi.fn().mockResolvedValue(undefined),
      fileSrcFromPath: (p) => `file://${p}`,
    });

    const draft: LiveRecordingDraftHandle = {
      item: draftItem,
      audioAbsolutePath: 'C:/audio/native-history.wav',
    };

    // persistNativeRecording settles immediately after archival even while postProcess & annotate are pending
    await persistence.persistNativeRecording(draft, 'temp-native.wav', 5);

    expect(hasActiveBackgroundQuitTasks()).toBe(true);
    expect(
      selectIsDocumentEditorLocked(
        useDocumentMutationLockStore.getState().locks,
        'native-history',
        'native-history'
      )
    ).toBe(false);

    // Simulate manual/post-process text edit while slow speaker inference runs outside lock
    useTranscriptStore
      .getState()
      .setSegmentsForSession('native-history', [
        { id: 'seg-1', start: 0, end: 3, text: 'Manually edited hello', isFinal: true },
      ]);

    annotateDeferred.resolve([
      {
        id: 'seg-1',
        start: 0,
        end: 3,
        text: 'Initial raw text',
        isFinal: true,
        speakerAttribution: {
          speakerId: 'spk-1',
          speakerName: 'Alice',
          source: 'auto',
        },
      },
    ]);
    postProcessDeferred.resolve();

    await vi.waitFor(() => {
      expect(commitTranscriptUpdate).toHaveBeenCalledWith('native-history', [
        expect.objectContaining({
          id: 'seg-1',
          text: 'Manually edited hello',
          speakerAttribution: expect.objectContaining({
            speakerId: 'spk-1',
            speakerName: 'Alice',
          }),
        }),
      ]);
      expect(hasActiveBackgroundQuitTasks()).toBe(false);
    });
  });
});
