import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { createRecordingPersistence } from '../persistence';
import type { RecordingPersistenceTranscriptState } from '../types';
import type { HistoryItem } from '../../../types/history';

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
    };
}

describe('createRecordingPersistence', () => {
    const originalCreateObjectUrl = URL.createObjectURL;

    beforeEach(() => {
        URL.createObjectURL = vi.fn(() => 'blob:mock-url');
    });

    afterEach(() => {
        URL.createObjectURL = originalCreateObjectUrl;
    });

    it('runs the shared metadata sync and summary persist path for both browser and native saves', async () => {
        const transcriptState: RecordingPersistenceTranscriptState = {
            config: {} as RecordingPersistenceTranscriptState['config'],
            segments: [{ id: 'seg-1', text: 'hello', start: 0, end: 1, isFinal: true }],
            setAudioUrl: vi.fn(),
            setSegments: vi.fn(),
        };

        const addHistoryItem = vi.fn();
        const syncSavedRecordingMeta = vi.fn();
        const persistSummary = vi.fn().mockResolvedValue(undefined);
        const setActiveProjectId = vi.fn().mockResolvedValue(undefined);
        const saveRecording = vi.fn().mockResolvedValue(createHistoryItem('web-history'));
        const saveNativeRecording = vi.fn().mockResolvedValue(createHistoryItem('native-history'));
        const removeFile = vi.fn().mockResolvedValue(undefined);
        const writeTempFile = vi.fn().mockResolvedValue(undefined);

        const persistence = createRecordingPersistence({
            logger: {
                info: vi.fn(),
                warn: vi.fn(),
                error: vi.fn(),
            },
            history: {
                saveRecording,
                saveNativeRecording,
            },
            getTranscriptState: () => transcriptState,
            getActiveProjectId: () => 'project-1',
            setActiveProjectId,
            addHistoryItem,
            persistSummary,
            annotateSegmentsForFile: vi.fn(async (_filePath, segments) => segments),
            syncSavedRecordingMeta,
            resolveTempFilePath: vi.fn(async (extension) => `temp.${extension}`),
            writeTempFile,
            removeFile,
            fileSrcFromPath: (filePath) => `file://${filePath}`,
        });

        await persistence.persistBrowserRecording(new Blob(['web'], { type: 'audio/webm' }), 3);
        await persistence.persistNativeRecording('native.wav', 3);

        expect(saveRecording).toHaveBeenCalledTimes(1);
        expect(saveNativeRecording).toHaveBeenCalledTimes(1);
        expect(addHistoryItem).toHaveBeenCalledTimes(2);
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(1, 'Recording web-history', 'web-history', 'system:mic');
        expect(syncSavedRecordingMeta).toHaveBeenNthCalledWith(2, 'Recording native-history', 'native-history', 'system:mic');
        expect(persistSummary).toHaveBeenNthCalledWith(1, 'web-history');
        expect(persistSummary).toHaveBeenNthCalledWith(2, 'native-history');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(1, 'project-1');
        expect(setActiveProjectId).toHaveBeenNthCalledWith(2, 'project-1');
        expect(writeTempFile).toHaveBeenCalledTimes(1);
        expect(removeFile).toHaveBeenCalledWith('temp.webm');
    });
});
