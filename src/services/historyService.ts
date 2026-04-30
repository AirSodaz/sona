import { convertFileSrc } from '@tauri-apps/api/core';
import { v4 as uuidv4 } from 'uuid';
import { HistoryItem } from '../types/history';
import { HistorySummaryPayload, TranscriptSegment } from '../types/transcript';
import {
    TranscriptSnapshotMetadata,
    TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
} from '../types/transcriptSnapshot';
import { buildHistoryTranscriptMetadata } from '../utils/historyTranscriptMetadata';
import { logger } from '../utils/logger';
import { normalizeTranscriptSegments } from '../utils/transcriptTiming';
import {
    historyCompleteLiveDraft,
    historyCreateLiveDraft,
    historyDeleteItems,
    historyDeleteSummary,
    historyCreateTranscriptSnapshot,
    historyListItems,
    historyLoadSummary,
    historyLoadTranscript,
    historyListTranscriptSnapshots,
    historyLoadTranscriptSnapshot,
    historyOpenFolder,
    historyReassignProject,
    historyResolveAudioPath,
    historySaveImportedFile,
    historySaveRecording,
    historySaveSummary,
    historyUpdateItemMeta,
    historyUpdateProjectAssignments,
    historyUpdateTranscript,
} from './tauri/history';

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

function buildRecordingTitle(timestamp: number): string {
    const dateStr = new Date(timestamp).toISOString().split('T')[0];
    const timeStr = new Date(timestamp).toLocaleTimeString().replace(/:/g, '-');
    return `Recording ${dateStr} ${timeStr}`;
}

function buildDraftAudioFileName(id: string, audioExtension: string): string {
    return `${id}.${audioExtension.replace(/^\./, '')}`;
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
        const id = uuidv4();
        const timestamp = Date.now();
        const item: HistoryItem = {
            id,
            timestamp,
            duration: 0,
            audioPath: buildDraftAudioFileName(id, audioExtension),
            transcriptPath: `${id}.json`,
            title: buildRecordingTitle(timestamp),
            previewText: '',
            icon: icon || undefined,
            type: 'recording',
            searchContent: '',
            projectId,
            status: 'draft',
            draftSource: 'live_record',
        };

        const result = await historyCreateLiveDraft(item);

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
        const normalizedSegments = normalizeTranscriptSegments(segments);
        const { previewText, searchContent } = buildHistoryTranscriptMetadata(normalizedSegments);
        const item = await historyCompleteLiveDraft(
            historyId,
            normalizedSegments,
            previewText,
            searchContent,
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
            const id = uuidv4();
            const timestamp = Date.now();
            const filename = absoluteWavPath.split(/[/\\]/).pop() || `${id}.wav`;
            const normalizedSegments = normalizeTranscriptSegments(segments);
            const { previewText, searchContent } = buildHistoryTranscriptMetadata(normalizedSegments);

            const item = await historySaveRecording({
                item: {
                    id,
                    timestamp,
                    duration,
                    audioPath: filename,
                    transcriptPath: `${id}.json`,
                    title: buildRecordingTitle(timestamp),
                    previewText,
                    type: 'recording',
                    searchContent,
                    projectId,
                    status: 'complete',
                },
                segments: normalizedSegments,
                nativeAudioPath: absoluteWavPath,
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
            const id = uuidv4();
            const timestamp = Date.now();
            const normalizedSegments = normalizeTranscriptSegments(segments);
            const { previewText, searchContent } = buildHistoryTranscriptMetadata(normalizedSegments);
            const audioBytes = Array.from(new Uint8Array(await audioBlob.arrayBuffer()));

            const item = await historySaveRecording({
                item: {
                    id,
                    timestamp,
                    duration,
                    audioPath: `${id}.webm`,
                    transcriptPath: `${id}.json`,
                    title: buildRecordingTitle(timestamp),
                    previewText,
                    type: 'recording',
                    searchContent,
                    projectId,
                    status: 'complete',
                },
                segments: normalizedSegments,
                audioBytes,
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
            const id = uuidv4();
            const timestamp = Date.now();
            const filename = filePath.split(/[/\\]/).pop() || 'Imported File';
            const targetExt = convertedFilePath ? 'wav' : (filename.split('.').pop() || 'wav');
            const normalizedSegments = normalizeTranscriptSegments(segments);
            const { previewText, searchContent } = buildHistoryTranscriptMetadata(normalizedSegments);

            const item = await historySaveImportedFile({
                item: {
                    id,
                    timestamp,
                    duration,
                    audioPath: `${id}.${targetExt}`,
                    transcriptPath: `${id}.json`,
                    title: `Batch ${filename}`,
                    previewText,
                    type: 'batch',
                    searchContent,
                    projectId,
                    status: 'complete',
                },
                segments: normalizedSegments,
                sourcePath: convertedFilePath || filePath,
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
            const raw = await historyLoadTranscript(filename);
            if (!Array.isArray(raw)) {
                return raw === null ? null : normalizeTranscriptSegments([]);
            }

            return normalizeTranscriptSegments(raw as TranscriptSegment[]);
        } catch (error) {
            logger.error('Failed to load transcript:', error);
            return null;
        }
    },

    async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<void> {
        try {
            const normalizedSegments = normalizeTranscriptSegments(segments);
            const { previewText, searchContent } = buildHistoryTranscriptMetadata(normalizedSegments);
            await historyUpdateTranscript(historyId, normalizedSegments, previewText, searchContent);
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
        const normalizedSegments = normalizeTranscriptSegments(segments);
        return historyCreateTranscriptSnapshot(historyId, reason, normalizedSegments);
    },

    async listTranscriptSnapshots(historyId: string): Promise<TranscriptSnapshotMetadata[]> {
        return historyListTranscriptSnapshots(historyId);
    },

    async loadTranscriptSnapshot(
        historyId: string,
        snapshotId: string,
    ): Promise<TranscriptSnapshotRecord | null> {
        const record = await historyLoadTranscriptSnapshot(historyId, snapshotId);
        if (!record) {
            return null;
        }

        return {
            ...record,
            segments: normalizeTranscriptSegments(record.segments),
        };
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
