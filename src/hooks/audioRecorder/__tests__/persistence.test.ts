import { describe, expect, it, vi } from 'vitest';
import { createRecordingPersistence } from '../persistence';
import type { RecordingPersistenceTranscriptState } from '../types';
import type { HistoryItem } from '../../../types/history';
import type { LiveRecordingDraftHandle } from '../../../services/historyService';

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
        const deleteHistoryItem = vi.fn().mockResolvedValue(undefined);
        const syncSavedRecordingMeta = vi.fn();
        const persistSummary = vi.fn().mockResolvedValue(undefined);
        const setActiveProjectId = vi.fn().mockResolvedValue(undefined);
        const createLiveRecordingDraft = vi.fn()
            .mockResolvedValueOnce(createDraftHandle('web-history', 'webm'))
            .mockResolvedValueOnce(createDraftHandle('native-history', 'wav'));
        const completeLiveRecordingDraft = vi.fn()
            .mockResolvedValueOnce(createHistoryItem('web-history'))
            .mockResolvedValueOnce(createHistoryItem('native-history'));
        const removeFile = vi.fn().mockResolvedValue(undefined);
        const writeFile = vi.fn().mockResolvedValue(undefined);

        const persistence = createRecordingPersistence({
            logger: {
                info: vi.fn(),
                warn: vi.fn(),
                error: vi.fn(),
            },
            history: {
                createLiveRecordingDraft,
                completeLiveRecordingDraft,
                discardLiveRecordingDraft: vi.fn().mockResolvedValue(undefined),
                saveRecording: vi.fn(),
                saveNativeRecording: vi.fn(),
            },
            getTranscriptState: () => transcriptState,
            getActiveProjectId: () => 'project-1',
            setActiveProjectId,
            addHistoryItem,
            upsertHistoryItem,
            deleteHistoryItem,
            persistSummary,
            annotateSegmentsForFile: vi.fn(async (_filePath, segments) => segments),
            syncSavedRecordingMeta,
            writeFile,
            removeFile,
            fileSrcFromPath: (filePath) => `file://${filePath}`,
        });

        const webDraft = await persistence.createLiveRecordingDraft('webm');
        const nativeDraft = await persistence.createLiveRecordingDraft('wav');

        await persistence.persistBrowserRecording(webDraft, new Blob(['web'], { type: 'audio/webm' }), 3);
        await persistence.persistNativeRecording(nativeDraft, 'native.wav', 3);

        expect(createLiveRecordingDraft).toHaveBeenCalledTimes(2);
        expect(completeLiveRecordingDraft).toHaveBeenNthCalledWith(1, 'web-history', transcriptState.segments, 3);
        expect(completeLiveRecordingDraft).toHaveBeenNthCalledWith(2, 'native-history', transcriptState.segments, 3);
        expect(addHistoryItem).toHaveBeenNthCalledWith(1, webDraft.item);
        expect(addHistoryItem).toHaveBeenNthCalledWith(2, nativeDraft.item);
        expect(upsertHistoryItem).toHaveBeenNthCalledWith(1, createHistoryItem('web-history'));
        expect(upsertHistoryItem).toHaveBeenNthCalledWith(2, createHistoryItem('native-history'));
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(1, 'Recording web-history', 'web-history', 'system:mic');
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(2, 'Recording native-history', 'native-history', 'system:mic');
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(3, 'Recording web-history', 'web-history', 'system:mic');
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(4, 'Recording native-history', 'native-history', 'system:mic');
        expect(persistSummary).toHaveBeenNthCalledWith(1, 'web-history');
        expect(persistSummary).toHaveBeenNthCalledWith(2, 'native-history');
        expect(persistSummary).toHaveBeenNthCalledWith(3, 'web-history');
        expect(persistSummary).toHaveBeenNthCalledWith(4, 'native-history');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(1, 'project-1');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(2, 'project-1');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(3, 'project-1');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(4, 'project-1');
        expect(writeFile).toHaveBeenCalledWith('C:/history/web-history.webm', expect.any(Uint8Array));
        expect(deleteHistoryItem).not.toHaveBeenCalled();
        expect(removeFile).not.toHaveBeenCalled();
        expect(transcriptState.setAudioUrl).toHaveBeenNthCalledWith(1, 'file://C:/history/web-history.webm');
        expect(transcriptState.setAudioUrl).toHaveBeenNthCalledWith(2, 'file://native.wav');
    });
});
