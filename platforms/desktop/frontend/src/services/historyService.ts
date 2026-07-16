import {
    HistoryAudioCleanupReport,
    HistoryItem,
    normalizeHistoryItemRecord as normalizeHistoryItem,
} from '../types/history';
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
    historyCleanupAudio,
    historyDeleteItems,
    historyPurgeItems,
    historyRestoreItems,
    historyTrashItems,
    historyDeleteSummary,
    historyCreateTranscriptSnapshot,
    historyBuildTranscriptDiff,
    historyListItems,
    historyLoadSummary,
    historyLoadTranscript,
    historyListTranscriptSnapshots,
    historyLoadTranscriptSnapshot,
    historyOpenFolder,
    historyPreviewAudioCleanup,
    historyReassignProject,
    historyRestoreTranscriptDiffRows,
    historyResolveAudioPath,
    historySaveImportedFile,
    historySaveRecording,
    historySaveSummary,
    historyUpdateItemMeta,
    historyUpdateProjectAssignments,
    historyReplaceTagAssignments,
    historyUpdateTagAssignments,
    historyUpdateTranscript,
} from './tauri/history';
import { convertManagedAudioFileSrc } from './tauri/platform/assets';

export interface LiveRecordingDraftHandle {
    item: HistoryItem;
    audioAbsolutePath: string;
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

function normalizeTagIds(value: string[] | string | null | undefined): string[] {
    if (Array.isArray(value)) {
        return Array.from(new Set(value.filter((tagId) => typeof tagId === 'string' && tagId.trim())));
    }
    return typeof value === 'string' && value.trim() ? [value] : [];
}

export interface HistoryServicePorts {
    historyListItems: typeof historyListItems;
    historyCreateLiveDraft: typeof historyCreateLiveDraft;
    historyCompleteLiveDraft: typeof historyCompleteLiveDraft;
    historyCleanupAudio: typeof historyCleanupAudio;
    historyDeleteItems: typeof historyDeleteItems;
    historyPurgeItems: typeof historyPurgeItems;
    historyRestoreItems: typeof historyRestoreItems;
    historyTrashItems: typeof historyTrashItems;
    historyDeleteSummary: typeof historyDeleteSummary;
    historyCreateTranscriptSnapshot: typeof historyCreateTranscriptSnapshot;
    historyBuildTranscriptDiff: typeof historyBuildTranscriptDiff;
    historyLoadSummary: typeof historyLoadSummary;
    historyLoadTranscript: typeof historyLoadTranscript;
    historyListTranscriptSnapshots: typeof historyListTranscriptSnapshots;
    historyLoadTranscriptSnapshot: typeof historyLoadTranscriptSnapshot;
    historyOpenFolder: typeof historyOpenFolder;
    historyPreviewAudioCleanup: typeof historyPreviewAudioCleanup;
    historyReassignProject: typeof historyReassignProject;
    historyRestoreTranscriptDiffRows: typeof historyRestoreTranscriptDiffRows;
    historyResolveAudioPath: typeof historyResolveAudioPath;
    historySaveImportedFile: typeof historySaveImportedFile;
    historySaveRecording: typeof historySaveRecording;
    historySaveSummary: typeof historySaveSummary;
    historyUpdateItemMeta: typeof historyUpdateItemMeta;
    historyUpdateProjectAssignments: typeof historyUpdateProjectAssignments;
    historyReplaceTagAssignments: typeof historyReplaceTagAssignments;
    historyUpdateTagAssignments: typeof historyUpdateTagAssignments;
    historyUpdateTranscript: typeof historyUpdateTranscript;
    convertManagedAudioFileSrc: typeof convertManagedAudioFileSrc;
}

export class HistoryService {
    constructor(private readonly ports: HistoryServicePorts) {}

    async getAll(): Promise<HistoryItem[]> {
        const items = await this.ports.historyListItems();
        return Array.isArray(items) ? items.map((item) => normalizeHistoryItem(item)) : [];
    }

    async createLiveRecordingDraft(
        audioExtension: string,
        tagIds: string[] | string | null = [],
        icon: string | null = 'system:mic',
        id?: string,
    ): Promise<LiveRecordingDraftHandle> {
        const result = await this.ports.historyCreateLiveDraft(
            id ?? null,
            audioExtension,
            normalizeTagIds(tagIds),
            icon,
        );

        return {
            item: normalizeHistoryItem(result?.item),
            audioAbsolutePath: result?.audioAbsolutePath || '',
        };
    }

    async completeLiveRecordingDraft(
        historyId: string,
        segments: TranscriptSegment[],
        duration: number,
    ): Promise<HistoryItem> {
        const item = await this.ports.historyCompleteLiveDraft(
            historyId,
            segments,
            duration,
        );

        return normalizeHistoryItem(item);
    }

    async discardLiveRecordingDraft(historyId: string): Promise<void> {
        await this.ports.historyPurgeItems([historyId]);
    }

    async saveNativeRecording(
        absoluteWavPath: string,
        segments: TranscriptSegment[],
        duration: number,
        tagIds: string[] | string | null = [],
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving native recording...', { absoluteWavPath, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        const item = await this.ports.historySaveRecording({
            segments,
            duration,
            tagIds: normalizeTagIds(tagIds),
            nativeAudioPath: absoluteWavPath,
            audioExtension: inferAudioExtensionFromPath(absoluteWavPath, 'wav'),
        });

        return normalizeHistoryItem(item);
    }

    async saveRecording(
        audioBlob: Blob,
        segments: TranscriptSegment[],
        duration: number,
        tagIds: string[] | string | null = [],
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        const audioBytes = Array.from(new Uint8Array(await audioBlob.arrayBuffer()));

        const item = await this.ports.historySaveRecording({
            segments,
            duration,
            tagIds: normalizeTagIds(tagIds),
            audioBytes,
            audioExtension: inferAudioExtensionFromBlob(audioBlob),
        });

        return normalizeHistoryItem(item);
    }

    async saveImportedFile(
        filePath: string,
        segments: TranscriptSegment[],
        duration: number = 0,
        convertedFilePath?: string,
        tagIds: string[] | string | null = [],
        id?: string,
    ): Promise<HistoryItem | null> {
        logger.info('[History] Saving imported file...', { filePath, segments: segments.length });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        const item = await this.ports.historySaveImportedFile({
            sourcePath: filePath,
            segments,
            duration,
            tagIds: normalizeTagIds(tagIds),
            convertedSourcePath: convertedFilePath,
            id: id ?? null,
        });

        return normalizeHistoryItem(item);
    }

    async deleteRecording(id: string): Promise<void> {
        await this.ports.historyDeleteItems([id]);
    }

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            await this.ports.historyDeleteItems(ids);
        } catch (error) {
            logger.error('Failed to delete recordings:', error);
            throw error;
        }
    }

    async trashRecordings(ids: string[]): Promise<void> {
        if (ids.length > 0) {
            await this.ports.historyTrashItems(ids);
        }
    }

    async restoreRecordings(ids: string[]): Promise<void> {
        if (ids.length > 0) {
            await this.ports.historyRestoreItems(ids);
        }
    }

    async purgeRecordings(ids: string[]): Promise<void> {
        if (ids.length > 0) {
            await this.ports.historyPurgeItems(ids);
        }
    }

    async loadTranscript(historyId: string): Promise<TranscriptSegment[] | null> {
        return await this.ports.historyLoadTranscript(historyId);
    }

    async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<HistoryItem> {
        try {
            const item = await this.ports.historyUpdateTranscript(historyId, segments);
            return normalizeHistoryItem(item);
        } catch (error) {
            logger.error('[History] Failed to update transcript:', error);
            throw error;
        }
    }

    async createTranscriptSnapshot(
        historyId: string,
        reason: TranscriptSnapshotReason,
        segments: TranscriptSegment[],
    ): Promise<TranscriptSnapshotMetadata> {
        return this.ports.historyCreateTranscriptSnapshot(historyId, reason, segments);
    }

    async listTranscriptSnapshots(historyId: string): Promise<TranscriptSnapshotMetadata[]> {
        return this.ports.historyListTranscriptSnapshots(historyId);
    }

    async loadTranscriptSnapshot(
        historyId: string,
        snapshotId: string,
    ): Promise<TranscriptSnapshotRecord | null> {
        return this.ports.historyLoadTranscriptSnapshot(historyId, snapshotId);
    }

    async buildTranscriptDiff(
        snapshotSegments: TranscriptSegment[],
        currentSegments: TranscriptSegment[],
    ): Promise<TranscriptDiffResult> {
        return this.ports.historyBuildTranscriptDiff(snapshotSegments, currentSegments);
    }

    async restoreTranscriptDiffRows(
        rows: TranscriptDiffRow[],
        selectedRowIds: Iterable<string>,
    ): Promise<TranscriptSegment[]> {
        return this.ports.historyRestoreTranscriptDiffRows(rows, selectedRowIds);
    }

    async updateItemMeta(id: string, updates: Partial<HistoryItem>): Promise<void> {
        await this.ports.historyUpdateItemMeta(id, updates);
    }

    async updateProjectAssignments(ids: string[], projectId: string | null): Promise<void> {
        if (ids.length === 0) {
            return;
        }

        await this.ports.historyUpdateProjectAssignments(ids, projectId);
    }

    async updateTagAssignments(
        ids: string[],
        addTagIds: string[],
        removeTagIds: string[],
    ): Promise<void> {
        if (ids.length > 0) {
            await this.ports.historyUpdateTagAssignments(ids, addTagIds, removeTagIds);
        }
    }

    async replaceTagAssignments(ids: string[], tagIds: string[]): Promise<void> {
        if (ids.length > 0) {
            await this.ports.historyReplaceTagAssignments(ids, tagIds);
        }
    }

    async updateProjectAssignmentsByCurrentProject(
        currentProjectId: string,
        nextProjectId: string | null,
    ): Promise<void> {
        await this.ports.historyReassignProject(currentProjectId, nextProjectId);
    }

    async loadSummary(historyId: string): Promise<HistorySummaryPayload | null> {
        return await this.ports.historyLoadSummary(historyId);
    }

    async saveSummary(historyId: string, summaryPayload: HistorySummaryPayload): Promise<void> {
        try {
            await this.ports.historySaveSummary(historyId, summaryPayload);
        } catch (error) {
            logger.error('[History] Failed to save summary sidecar:', error);
            throw error;
        }
    }

    async deleteSummary(historyId: string): Promise<void> {
        await this.ports.historyDeleteSummary(historyId);
    }

    async getAudioAbsolutePath(historyId: string): Promise<string | null> {
        return await this.ports.historyResolveAudioPath(historyId);
    }

    async getAudioUrl(historyId: string): Promise<string | null> {
        const fullPath = await this.getAudioAbsolutePath(historyId);
        return fullPath ? this.ports.convertManagedAudioFileSrc(fullPath) : null;
    }

    async previewAudioCleanup(
        retentionDays: number | null,
        excludeHistoryId: string | null = null,
    ): Promise<HistoryAudioCleanupReport> {
        return this.ports.historyPreviewAudioCleanup({ retentionDays, excludeHistoryId });
    }

    async cleanupAudio(
        retentionDays: number | null,
        excludeHistoryId: string | null = null,
    ): Promise<HistoryAudioCleanupReport> {
        return this.ports.historyCleanupAudio({ retentionDays, excludeHistoryId });
    }

    async openHistoryFolder(): Promise<void> {
        await this.ports.historyOpenFolder();
    }
}

export function createHistoryService(ports: HistoryServicePorts): HistoryService {
    return new HistoryService(ports);
}

export const historyService = createHistoryService({
    historyListItems,
    historyCreateLiveDraft,
    historyCompleteLiveDraft,
    historyCleanupAudio,
    historyDeleteItems,
    historyPurgeItems,
    historyRestoreItems,
    historyTrashItems,
    historyDeleteSummary,
    historyCreateTranscriptSnapshot,
    historyBuildTranscriptDiff,
    historyLoadSummary,
    historyLoadTranscript,
    historyListTranscriptSnapshots,
    historyLoadTranscriptSnapshot,
    historyOpenFolder,
    historyPreviewAudioCleanup,
    historyReassignProject,
    historyRestoreTranscriptDiffRows,
    historyResolveAudioPath,
    historySaveImportedFile,
    historySaveRecording,
    historySaveSummary,
    historyUpdateItemMeta,
    historyUpdateProjectAssignments,
    historyReplaceTagAssignments,
    historyUpdateTagAssignments,
    historyUpdateTranscript,
    convertManagedAudioFileSrc,
});
