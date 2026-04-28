import type { AppConfig } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { TranscriptSegment } from '../../types/transcript';
import type { LiveRecordingDraftHandle } from '../../services/historyService';
import type {
    AudioRecorderLogger,
    RecordingHistorySaver,
    RecordingMetaState,
    RecordingPersistenceTranscriptState,
} from './types';

export function syncSavedRecordingMeta(
    transcriptState: RecordingMetaState,
    title: string,
    historyId: string,
    icon: string | undefined | null,
): void {
    transcriptState.setSourceHistoryId(historyId);
    transcriptState.setTitle(title);
    transcriptState.setIcon(icon || null);
}

export function getRecordedAudioExtension(mimeType: string): string {
    if (mimeType.includes('mp4')) return 'm4a';
    if (mimeType.includes('aac')) return 'aac';
    if (mimeType.includes('ogg')) return 'ogg';
    if (mimeType.includes('wav')) return 'wav';
    return 'webm';
}

interface CreateRecordingPersistenceArgs {
    logger: AudioRecorderLogger;
    history: RecordingHistorySaver;
    getTranscriptState: () => RecordingPersistenceTranscriptState;
    getActiveProjectId: () => string | null;
    setActiveProjectId: (projectId: string | null) => Promise<void> | void;
    addHistoryItem: (item: HistoryItem) => void;
    upsertHistoryItem: (item: HistoryItem) => void;
    deleteHistoryItem: (id: string) => Promise<void>;
    persistSummary: (historyId: string) => Promise<void>;
    annotateSegmentsForFile: (
        filePath: string,
        segments: TranscriptSegment[],
        config: AppConfig,
    ) => Promise<TranscriptSegment[]>;
    syncSavedRecordingMeta: (title: string, historyId: string, icon: string | undefined | null) => void;
    writeFile: (filePath: string, contents: Uint8Array) => Promise<void>;
    removeFile: (filePath: string) => Promise<void>;
    fileSrcFromPath: (filePath: string) => string;
}

export function createRecordingPersistence({
    logger,
    history,
    getTranscriptState,
    getActiveProjectId,
    setActiveProjectId,
    addHistoryItem,
    upsertHistoryItem,
    deleteHistoryItem,
    persistSummary,
    annotateSegmentsForFile,
    syncSavedRecordingMeta: syncMeta,
    writeFile,
    removeFile,
    fileSrcFromPath,
}: CreateRecordingPersistenceArgs) {
    async function persistSavedItem(newItem: HistoryItem, strategy: 'add' | 'upsert' = 'upsert'): Promise<void> {
        if (strategy === 'add') {
            addHistoryItem(newItem);
        } else {
            upsertHistoryItem(newItem);
        }
        syncMeta(newItem.title, newItem.id, newItem.icon);
        void setActiveProjectId(newItem.projectId);
        await persistSummary(newItem.id);
    }

    async function annotateRecordedSegments(
        filePath: string,
        segments: TranscriptSegment[],
    ): Promise<TranscriptSegment[]> {
        const transcriptState = getTranscriptState();
        const annotatedSegments = await annotateSegmentsForFile(filePath, segments, transcriptState.config);
        transcriptState.setSegments(annotatedSegments);
        return annotatedSegments;
    }

    async function writeRecordedBlobToPath(blob: Blob, filePath: string): Promise<void> {
        const contents = new Uint8Array(await blob.arrayBuffer());
        await writeFile(filePath, contents);
    }

    async function createLiveRecordingDraft(audioExtension: string): Promise<LiveRecordingDraftHandle> {
        const draft = await history.createLiveRecordingDraft(
            audioExtension,
            getActiveProjectId(),
            'system:mic',
        );
        await persistSavedItem(draft.item, 'add');
        return draft;
    }

    async function discardLiveRecordingDraft(draft: LiveRecordingDraftHandle): Promise<void> {
        await deleteHistoryItem(draft.item.id);
    }

    async function persistBrowserRecording(
        draft: LiveRecordingDraftHandle,
        blob: Blob,
        duration: number,
    ): Promise<void> {
        const transcriptState = getTranscriptState();
        let segments = transcriptState.segments;

        if (segments.length === 0) {
            return;
        }

        try {
            await writeRecordedBlobToPath(blob, draft.audioAbsolutePath);

            try {
                segments = await annotateRecordedSegments(draft.audioAbsolutePath, segments);
            } catch (error) {
                logger.warn('[useAudioRecorder] Failed to annotate speaker labels for MediaRecorder fallback audio:', error);
            }

            transcriptState.setAudioUrl(fileSrcFromPath(draft.audioAbsolutePath));

            const newItem = await history.completeLiveRecordingDraft(
                draft.item.id,
                segments,
                duration,
            );
            await persistSavedItem(newItem);
        } catch (error) {
            logger.error('[useAudioRecorder] Failed to persist browser recording draft:', error);
            throw error;
        }
    }

    async function persistNativeRecording(
        draft: LiveRecordingDraftHandle,
        savedWavPath: string,
        duration: number,
    ): Promise<void> {
        const transcriptState = getTranscriptState();
        let segments = transcriptState.segments;

        if (segments.length > 0) {
            try {
                segments = await annotateRecordedSegments(savedWavPath, segments);
            } catch (error) {
                logger.warn('[useAudioRecorder] Failed to annotate speaker labels for native recording:', error);
            }

            transcriptState.setAudioUrl(fileSrcFromPath(savedWavPath));

            const newItem = await history.completeLiveRecordingDraft(
                draft.item.id,
                segments,
                duration,
            );
            await persistSavedItem(newItem);
            return;
        }

        logger.info('[useAudioRecorder] Empty transcript, deleting unsaved WAV file:', savedWavPath);
        try {
            await removeFile(savedWavPath);
        } catch (error) {
            logger.error('[useAudioRecorder] Failed to delete empty WAV file:', error);
        }
    }

    return {
        createLiveRecordingDraft,
        discardLiveRecordingDraft,
        persistBrowserRecording,
        persistNativeRecording,
    };
}
