import type { AppConfig } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { TranscriptSegment } from '../../types/transcript';
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

function getAudioTempExtension(mimeType: string): string {
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
    persistSummary: (historyId: string) => Promise<void>;
    annotateSegmentsForFile: (
        filePath: string,
        segments: TranscriptSegment[],
        config: AppConfig,
    ) => Promise<TranscriptSegment[]>;
    syncSavedRecordingMeta: (title: string, historyId: string, icon: string | undefined | null) => void;
    resolveTempFilePath: (extension: string) => Promise<string>;
    writeTempFile: (filePath: string, contents: Uint8Array) => Promise<void>;
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
    persistSummary,
    annotateSegmentsForFile,
    syncSavedRecordingMeta: syncMeta,
    resolveTempFilePath,
    writeTempFile,
    removeFile,
    fileSrcFromPath,
}: CreateRecordingPersistenceArgs) {
    async function persistSavedItem(newItem: HistoryItem): Promise<void> {
        addHistoryItem(newItem);
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

    async function writeRecordedBlobToTempFile(blob: Blob, mimeType: string): Promise<string> {
        const extension = getAudioTempExtension(mimeType);
        const filePath = await resolveTempFilePath(extension);
        const contents = new Uint8Array(await blob.arrayBuffer());
        await writeTempFile(filePath, contents);
        return filePath;
    }

    async function persistBrowserRecording(blob: Blob, duration: number): Promise<void> {
        const transcriptState = getTranscriptState();
        const audioUrl = URL.createObjectURL(blob);
        transcriptState.setAudioUrl(audioUrl);

        let segments = transcriptState.segments;
        let tempAudioPath: string | null = null;

        if (segments.length === 0) {
            return;
        }

        try {
            // Browser fallback still converges onto the same post-recording sink
            // as native capture: annotate segments, save the history item, then
            // reconnect that metadata to the active transcript view.
            try {
                tempAudioPath = await writeRecordedBlobToTempFile(blob, blob.type || 'audio/webm');
                segments = await annotateRecordedSegments(tempAudioPath, segments);
            } catch (error) {
                logger.warn('[useAudioRecorder] Failed to annotate speaker labels for MediaRecorder fallback audio:', error);
            }

            const newItem = await history.saveRecording(
                blob,
                segments,
                duration,
                getActiveProjectId(),
            );
            if (newItem) {
                await persistSavedItem(newItem);
            }
        } finally {
            if (tempAudioPath) {
                try {
                    await removeFile(tempAudioPath);
                } catch (error) {
                    logger.warn('[useAudioRecorder] Failed to remove temporary MediaRecorder audio file:', error);
                }
            }
        }
    }

    async function persistNativeRecording(savedWavPath: string, duration: number): Promise<void> {
        const transcriptState = getTranscriptState();
        let segments = transcriptState.segments;

        if (segments.length > 0) {
            // Native and browser capture ultimately share the same persistence
            // sink: finalized segments, audio asset, history item, and summary
            // metadata.
            try {
                segments = await annotateRecordedSegments(savedWavPath, segments);
            } catch (error) {
                logger.warn('[useAudioRecorder] Failed to annotate speaker labels for native recording:', error);
            }

            transcriptState.setAudioUrl(fileSrcFromPath(savedWavPath));

            const newItem = await history.saveNativeRecording(
                savedWavPath,
                segments,
                duration,
                getActiveProjectId(),
            );
            if (newItem) {
                await persistSavedItem(newItem);
            }
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
        persistBrowserRecording,
        persistNativeRecording,
    };
}
