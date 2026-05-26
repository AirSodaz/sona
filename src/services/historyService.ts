import { HistoryItem } from '../types/history';
import { HistorySummaryPayload, TranscriptSegment } from '../types/transcript';
import {
    TranscriptDiffResult,
    TranscriptDiffRow,
    TranscriptSnapshotMetadata,
    TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
} from '../types/transcriptSnapshot';
import { logger } from '../utils/logger';
import {
    historyCompleteLiveDraft,
    historyCreateLiveDraft,
    historyDeleteItems,
    historyDeleteSummary,
    historyCreateTranscriptSnapshot,
    historyBuildTranscriptDiff,
    historyListItems,
    historyLoadSummary,
    historyLoadTranscript,
    historyListTranscriptSnapshots,
    historyLoadTranscriptSnapshot,
    historyOpenFolder,
    historyReassignProject,
    historyRestoreTranscriptDiffRows,
    historyResolveAudioPath,
    historySaveImportedFile,
    historySaveRecording,
    historySaveSummary,
    historyUpdateItemMeta,
    historyUpdateProjectAssignments,
    historyUpdateTranscript,
} from './tauri/history';
import { convertFileSrc } from './tauri/platform/assets';

export interface LiveRecordingDraftHandle {
    item: HistoryItem;
    audioAbsolutePath: string;
}

function normalizeHistoryItem(item: Partial<HistoryItem> | null | undefined): HistoryItem {
    return {
        id: item?.id || '',
        timestamp: item?.timestamp || 0,
        duration: item?.duration || 0,
        audioPath: item?.audioPath || '',
        transcriptPath: item?.transcriptPath || '',
        title: item?.title || '',
        previewText: item?.previewText || '',
        icon: typeof item?.icon === 'string' ? item.icon : undefined,
        type: item?.type === 'batch' ? 'batch' : 'recording',
        searchContent: item?.searchContent || '',
        projectId: typeof item?.projectId === 'string' ? item.projectId : null,
        status: item?.status === 'draft' ? 'draft' : 'complete',
        draftSource: item?.draftSource === 'live_record' ? 'live_record' : undefined,
    };
}

function inferAudioExtensionFromPath(filePath: string, fallback: string): string {
    const fileName = filePath.split(/[/\\]/).pop() || '';
    const extensionIndex = fileName.lastIndexOf('.');
    const extension = extensionIndex >= 0 ? fileName.slice(extensionIndex + 1).trim() : '';
    return extension || fallback;
}

function inferAudioExtensionFromBlob(blob: Blob): string {
    const mimeType = blob.type.toLowerCase();
    if (mimeType.includes('mp4')) return 'm4a';
    if (mimeType.includes('aac')) return 'aac';
    if (mimeType.includes('ogg')) return 'ogg';
    if (mimeType.includes('wav')) return 'wav';
    return 'webm';
}

export const historyService = {
    async getAll(): Promise<HistoryItem[]> {
        try {
            const items = await historyListItems();
            return Array.isArray(items) ? items.map((item) => normalizeHistoryItem(item)) : [];
        } catch (error) {
            logger.error('[History] Failed to load history:', error);
            return [];
        }
    },

    async createLiveRecordingDraft(
        audioExtension: string,
        projectId: string | null = null,
        icon: string | null = 'system:mic',
    ): Promise<LiveRecordingDraftHandle> {
        const result = await historyCreateLiveDraft(audioExtension, projectId, icon);

        return {
            item: normalizeHistoryItem(result?.item),
            audioAbsolutePath: result?.audioAbsolutePath || '',
        };
    },

    async completeLiveRecordingDraft(
        historyId: string,
        segments: TranscriptSegment[],
        duration: number,
    ): Promise<HistoryItem> {
        const item = await historyCompleteLiveDraft(
            historyId,
            segments,
            duration,
        );

        return normalizeHistoryItem(item);
    },

    async discardLiveRecordingDraft(historyId: string): Promise<void> {
        await this.deleteRecording(historyId);
    },

    async saveNativeRecording(
        absoluteWavPath: string,
        segments: TranscriptSegment[],
        duration: number,
        projectId: string | null = null,
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving native recording...', { absoluteWavPath, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            const item = await historySaveRecording({
                segments,
                duration,
                projectId,
                nativeAudioPath: absoluteWavPath,
                audioExtension: inferAudioExtensionFromPath(absoluteWavPath, 'wav'),
            });

            return normalizeHistoryItem(item);
        } catch (error) {
            logger.error('[History] Failed to save native recording:', error);
            return null;
        }
    },

    async saveRecording(
        audioBlob: Blob,
        segments: TranscriptSegment[],
        duration: number,
        projectId: string | null = null,
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            const audioBytes = Array.from(new Uint8Array(await audioBlob.arrayBuffer()));

            const item = await historySaveRecording({
                segments,
                duration,
                projectId,
                audioBytes,
                audioExtension: inferAudioExtensionFromBlob(audioBlob),
            });

            return normalizeHistoryItem(item);
        } catch (error) {
            logger.error('[History] Failed to save recording:', error);
            return null;
        }
    },

    async saveImportedFile(
        filePath: string,
        segments: TranscriptSegment[],
        duration: number = 0,
        convertedFilePath?: string,
        projectId: string | null = null,
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving imported file...', { filePath, segments: segments.length });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            const item = await historySaveImportedFile({
                sourcePath: filePath,
                segments,
                duration,
                projectId,
                convertedSourcePath: convertedFilePath,
            });

            return normalizeHistoryItem(item);
        } catch (error) {
            logger.error('[History] Failed to save imported file:', error);
            return null;
        }
    },

    async deleteRecording(id: string): Promise<void> {
        try {
            await historyDeleteItems([id]);
        } catch (error) {
            logger.error('Failed to delete recording:', error);
        }
    },

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            await historyDeleteItems(ids);
        } catch (error) {
            logger.error('Failed to delete recordings:', error);
            throw error;
        }
    },

    async loadTranscript(filename: string): Promise<TranscriptSegment[] | null> {
        try {
            return await historyLoadTranscript(filename);
        } catch (error) {
            logger.error('Failed to load transcript:', error);
            return null;
        }
    },

    async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<HistoryItem> {
        try {
            const item = await historyUpdateTranscript(historyId, segments);
            return normalizeHistoryItem(item);
        } catch (error) {
            logger.error('[History] Failed to update transcript:', error);
            throw error;
        }
    },

    async createTranscriptSnapshot(
        historyId: string,
        reason: TranscriptSnapshotReason,
        segments: TranscriptSegment[],
    ): Promise<TranscriptSnapshotMetadata> {
        return historyCreateTranscriptSnapshot(historyId, reason, segments);
    },

    async listTranscriptSnapshots(historyId: string): Promise<TranscriptSnapshotMetadata[]> {
        return historyListTranscriptSnapshots(historyId);
    },

    async loadTranscriptSnapshot(
        historyId: string,
        snapshotId: string,
    ): Promise<TranscriptSnapshotRecord | null> {
        return historyLoadTranscriptSnapshot(historyId, snapshotId);
    },

    async buildTranscriptDiff(
        snapshotSegments: TranscriptSegment[],
        currentSegments: TranscriptSegment[],
    ): Promise<TranscriptDiffResult> {
        return historyBuildTranscriptDiff(snapshotSegments, currentSegments);
    },

    async restoreTranscriptDiffRows(
        rows: TranscriptDiffRow[],
        selectedRowIds: Iterable<string>,
    ): Promise<TranscriptSegment[]> {
        return historyRestoreTranscriptDiffRows(rows, selectedRowIds);
    },

    async updateItemMeta(id: string, updates: Partial<HistoryItem>): Promise<void> {
        await historyUpdateItemMeta(id, updates);
    },

    async updateProjectAssignments(ids: string[], projectId: string | null): Promise<void> {
        if (ids.length === 0) {
            return;
        }

        await historyUpdateProjectAssignments(ids, projectId);
    },

    async updateProjectAssignmentsByCurrentProject(
        currentProjectId: string,
        nextProjectId: string | null,
    ): Promise<void> {
        await historyReassignProject(currentProjectId, nextProjectId);
    },

    async loadSummary(historyId: string): Promise<HistorySummaryPayload | null> {
        try {
            return await historyLoadSummary(historyId);
        } catch (error) {
            logger.error('[History] Failed to load summary sidecar:', error);
            return null;
        }
    },

    async saveSummary(historyId: string, summaryPayload: HistorySummaryPayload): Promise<void> {
        try {
            await historySaveSummary(historyId, summaryPayload);
        } catch (error) {
            logger.error('[History] Failed to save summary sidecar:', error);
            throw error;
        }
    },

    async deleteSummary(historyId: string): Promise<void> {
        try {
            await historyDeleteSummary(historyId);
        } catch (error) {
            logger.error('[History] Failed to delete summary sidecar:', error);
        }
    },

    async getAudioAbsolutePath(filename: string): Promise<string | null> {
        try {
            return await historyResolveAudioPath(filename);
        } catch (error) {
            logger.error('Failed to get audio absolute path:', error);
            return null;
        }
    },

    async getAudioUrl(filename: string): Promise<string | null> {
        try {
            const fullPath = await this.getAudioAbsolutePath(filename);
            return fullPath ? convertFileSrc(fullPath) : null;
        } catch (error) {
            logger.error('Failed to get audio URL:', error);
            return null;
        }
    },

    async openHistoryFolder(): Promise<void> {
        try {
            await historyOpenFolder();
        } catch (error) {
            logger.error('[History] Failed to open history folder:', error);
        }
    },
};
