import { logger } from "../utils/logger";
import { BaseDirectory, readTextFile, writeTextFile, writeFile, remove, exists, mkdir, copyFile, stat } from '@tauri-apps/plugin-fs';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
import { openPath } from '@tauri-apps/plugin-opener';
import { convertFileSrc } from '@tauri-apps/api/core';
import { HistorySummaryPayload, TranscriptSegment } from '../types/transcript';
import { HistoryItem } from '../types/history';
import { buildHistoryTranscriptMetadata } from '../utils/historyTranscriptMetadata';
import { extractErrorMessage } from '../utils/errorUtils';
import { v4 as uuidv4 } from 'uuid';

const HISTORY_DIR = 'history';
const INDEX_FILE = 'index.json';

export interface LiveRecordingDraftHandle {
    item: HistoryItem;
    audioAbsolutePath: string;
}

function normalizeHistoryItem(item: Partial<HistoryItem>): HistoryItem {
    return {
        id: item.id || '',
        timestamp: item.timestamp || 0,
        duration: item.duration || 0,
        audioPath: item.audioPath || '',
        transcriptPath: item.transcriptPath || '',
        title: item.title || '',
        previewText: item.previewText || '',
        icon: item.icon,
        type: item.type || 'recording',
        searchContent: item.searchContent || '',
        projectId: typeof item.projectId === 'string' ? item.projectId : null,
        status: item.status === 'draft' ? 'draft' : 'complete',
        draftSource: item.draftSource === 'live_record' ? 'live_record' : undefined,
    };
}

async function writeHistoryIndex(items: HistoryItem[]): Promise<void> {
    await writeTextFile(
        `${HISTORY_DIR}/${INDEX_FILE}`,
        JSON.stringify(items, null, 2),
        { baseDir: BaseDirectory.AppLocalData }
    );
}

function buildRecordingTitle(timestamp: number): string {
    const dateStr = new Date(timestamp).toISOString().split('T')[0];
    const timeStr = new Date(timestamp).toLocaleTimeString().replace(/:/g, '-');
    return `Recording ${dateStr} ${timeStr}`;
}

function buildDraftAudioFileName(id: string, audioExtension: string): string {
    return `${id}.${audioExtension.replace(/^\./, '')}`;
}

function hasErrorCode(error: unknown): error is { code?: string } {
    return typeof error === 'object' && error !== null && 'code' in error;
}

function isMissingHistoryFileError(error: unknown): boolean {
    const errMsg = extractErrorMessage(error);
    return errMsg.includes('No such file or directory')
        || (hasErrorCode(error) && error.code === 'ENOENT');
}

async function safeRemoveHistoryPath(path: string): Promise<void> {
    try {
        await remove(path, { baseDir: BaseDirectory.AppLocalData });
    } catch (error) {
        if (!isMissingHistoryFileError(error)) {
            logger.error(`[History] Failed to remove file at ${path}:`, error);
            throw error;
        }
    }
}

export const historyService = {

    async init(): Promise<void> {
        try {
            const historyExists = await exists(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData });
            if (!historyExists) {
                await mkdir(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
            }

            const indexExists = await exists(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
            if (!indexExists) {
                logger.info('[History] Creating index file');
                await writeTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, '[]', { baseDir: BaseDirectory.AppLocalData });
            }
        } catch (error) {
            logger.error('[History] Failed to initialize service:', error);
            throw error; // Re-throw to see it upstream
        }
    },

    async getAll(): Promise<HistoryItem[]> {
        try {
            logger.info('[History] Getting all items');
            await this.init();
            const content = await readTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
            logger.info('[History] Loaded index:', content?.substring(0, 50));
            return (JSON.parse(content) as Partial<HistoryItem>[]).map((item) => normalizeHistoryItem(item));
        } catch (error) {
            logger.error('[History] Failed to load history:', error);
            return [];
        }
    },

    async saveTranscriptFile(id: string, segments: TranscriptSegment[]) {
        // Save Transcript
        const transcriptFileName = `${id}.json`;
        const transcriptPathDisplay = `${HISTORY_DIR}/${transcriptFileName}`;
        logger.info('[History] Writing transcript file:', transcriptPathDisplay);
        await writeTextFile(
            transcriptPathDisplay,
            JSON.stringify(segments, null, 2),
            { baseDir: BaseDirectory.AppLocalData }
        );

        const { previewText, searchContent } = buildHistoryTranscriptMetadata(segments);

        return { transcriptFileName, previewText, searchContent };
    },

    async createLiveRecordingDraft(
        audioExtension: string,
        projectId: string | null = null,
        icon: string | null = 'system:mic',
    ): Promise<LiveRecordingDraftHandle> {
        await this.init();

        const id = uuidv4();
        const timestamp = Date.now();
        const title = buildRecordingTitle(timestamp);
        const audioFileName = buildDraftAudioFileName(id, audioExtension);
        const appDataDirPath = await appLocalDataDir();
        const audioAbsolutePath = await join(appDataDirPath, HISTORY_DIR, audioFileName);

        await writeTextFile(
            `${HISTORY_DIR}/${id}.json`,
            '[]',
            { baseDir: BaseDirectory.AppLocalData },
        );

        const newItem: HistoryItem = {
            id,
            timestamp,
            duration: 0,
            audioPath: audioFileName,
            transcriptPath: `${id}.json`,
            title,
            previewText: '',
            icon: icon || undefined,
            type: 'recording',
            searchContent: '',
            projectId,
            status: 'draft',
            draftSource: 'live_record',
        };

        const items = await this.getAll();
        items.unshift(newItem);
        await writeHistoryIndex(items);

        return {
            item: newItem,
            audioAbsolutePath,
        };
    },

    async completeLiveRecordingDraft(
        historyId: string,
        segments: TranscriptSegment[],
        duration: number,
    ): Promise<HistoryItem> {
        const items = await this.getAll();
        const item = items.find((entry) => entry.id === historyId);
        if (!item) {
            throw new Error(`History item not found: ${historyId}`);
        }

        await writeTextFile(
            `${HISTORY_DIR}/${item.transcriptPath}`,
            JSON.stringify(segments, null, 2),
            { baseDir: BaseDirectory.AppLocalData },
        );

        const { previewText, searchContent } = buildHistoryTranscriptMetadata(segments);
        item.previewText = previewText;
        item.searchContent = searchContent;
        item.duration = duration;
        item.status = 'complete';
        delete item.draftSource;

        await writeHistoryIndex(items);
        return { ...item };
    },

    async discardLiveRecordingDraft(historyId: string): Promise<void> {
        await this.deleteRecording(historyId);
    },

    async saveNativeRecording(absoluteWavPath: string, segments: TranscriptSegment[], duration: number, projectId: string | null = null): Promise<HistoryItem | null> {
        logger.info('[History] Saving native recording...', { absoluteWavPath, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();
            const title = buildRecordingTitle(timestamp);

            const filename = absoluteWavPath.split(/[/\\]/).pop() || `${id}.wav`;
            const audioFileName = filename;

            // Save Transcript and generate metadata
            const { transcriptFileName, previewText, searchContent } = await this.saveTranscriptFile(id, segments);

            const newItem: HistoryItem = {
                id,
                timestamp,
                duration,
                audioPath: audioFileName,
                transcriptPath: transcriptFileName,
                title,
                previewText,
                type: 'recording',
                searchContent,
                projectId,
                status: 'complete',
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeHistoryIndex(items);

            logger.info('[History] Native save complete:', newItem);
            return newItem;
        } catch (error) {
            logger.error('[History] Failed to save native recording:', error);
            return null;
        }
    },

    async saveRecording(audioBlob: Blob, segments: TranscriptSegment[], duration: number, projectId: string | null = null): Promise<HistoryItem | null> {
        logger.info('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();
            const title = buildRecordingTitle(timestamp);

            // Save Audio
            const audioBuffer = await audioBlob.arrayBuffer();
            const uint8Array = new Uint8Array(audioBuffer);
            const audioFileName = `${id}.webm`;
            const audioPathDisplay = `${HISTORY_DIR}/${audioFileName}`;

            logger.info('[History] Writing audio file:', audioPathDisplay);
            await writeFile(
                audioPathDisplay,
                uint8Array,
                { baseDir: BaseDirectory.AppLocalData }
            );

            // Save Transcript and generate metadata
            const { transcriptFileName, previewText, searchContent } = await this.saveTranscriptFile(id, segments);

            const newItem: HistoryItem = {
                id,
                timestamp,
                duration,
                audioPath: audioFileName, // Store relative path
                transcriptPath: transcriptFileName, // Store relative path
                title,
                previewText,
                type: 'recording',
                searchContent,
                projectId,
                status: 'complete',
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeHistoryIndex(items);

            logger.info('[History] Save complete:', newItem);
            return newItem;
        } catch (error) {
            logger.error('[History] Failed to save recording:', error);
            return null;
        }
    },

    async saveImportedFile(filePath: string, segments: TranscriptSegment[], duration: number = 0, convertedFilePath?: string, projectId: string | null = null): Promise<HistoryItem | null> {
        logger.info('[History] Saving imported file...', { filePath, segments: segments.length });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();

            // Get filename from path
            const filename = filePath.split(/[/\\]/).pop() || 'Imported File';

            // Generate title with Batch prefix
            const title = `Batch ${filename}`;

            // Save Audio (Copy)
            // If convertedFilePath is present, we save as .wav
            const targetExt = convertedFilePath ? 'wav' : (filename.split('.').pop() || 'wav');
            const audioFileName = `${id}.${targetExt}`;
            const audioPathDisplay = `${HISTORY_DIR}/${audioFileName}`;

            logger.info('[History] Copying audio file to:', audioPathDisplay);
            // Copy the file to the history directory
            await copyFile(
                convertedFilePath || filePath,
                audioPathDisplay,
                { toPathBaseDir: BaseDirectory.AppLocalData }
            );

            // Save Transcript and generate metadata
            const { transcriptFileName, previewText, searchContent } = await this.saveTranscriptFile(id, segments);

            const newItem: HistoryItem = {
                id,
                timestamp,
                duration, // This might be 0 if we don't have it, but that's okay for now
                audioPath: audioFileName,
                transcriptPath: transcriptFileName,
                title,
                previewText,
                type: 'batch',
                searchContent,
                projectId,
                status: 'complete',
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeHistoryIndex(items);

            logger.info('[History] Import save complete:', newItem);
            return newItem;

        } catch (error) {
            logger.error('[History] Failed to save imported file:', error);
            return null;
        }
    },

    async deleteRecording(id: string): Promise<void> {
        try {
            const items = await this.getAll();
            const itemToDelete = items.find(item => item.id === id);

            if (itemToDelete) {
                // Delete files
                const audioPath = `${HISTORY_DIR}/${itemToDelete.audioPath}`;
                const transcriptPath = `${HISTORY_DIR}/${itemToDelete.transcriptPath}`;
                const summaryPath = `${HISTORY_DIR}/${id}.summary.json`;

                await Promise.all([
                    safeRemoveHistoryPath(audioPath),
                    safeRemoveHistoryPath(transcriptPath),
                    safeRemoveHistoryPath(summaryPath)
                ]);
            }

            const newItems = items.filter(item => item.id !== id);
            await writeHistoryIndex(newItems);
        } catch (error) {
            logger.error('Failed to delete recording:', error);
        }
    },

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            const items = await this.getAll();
            const idSet = new Set(ids);
            const itemsToDelete = items.filter(item => idSet.has(item.id));

            // Process deletions in batches to avoid overwhelming Tauri IPC
            const BATCH_SIZE = 50;
            for (let i = 0; i < itemsToDelete.length; i += BATCH_SIZE) {
                const batch = itemsToDelete.slice(i, i + BATCH_SIZE);

                await Promise.allSettled(batch.map(async (item) => {
                    const audioPath = `${HISTORY_DIR}/${item.audioPath}`;
                    const transcriptPath = `${HISTORY_DIR}/${item.transcriptPath}`;
                    const summaryPath = `${HISTORY_DIR}/${item.id}.summary.json`;

                    await Promise.all([
                        safeRemoveHistoryPath(audioPath),
                        safeRemoveHistoryPath(transcriptPath),
                        safeRemoveHistoryPath(summaryPath)
                    ]);
                }));
            }

            const newItems = items.filter(item => !idSet.has(item.id));
            await writeHistoryIndex(newItems);
        } catch (error) {
            logger.error('Failed to delete recordings:', error);
            throw error;
        }
    },

    async loadTranscript(filename: string): Promise<TranscriptSegment[] | null> {
        try {
            const path = `${HISTORY_DIR}/${filename}`;
            const content = await readTextFile(path, { baseDir: BaseDirectory.AppLocalData });
            return JSON.parse(content);
        } catch (error) {
            logger.error('Failed to load transcript:', error);
            return null;
        }
    },

    /**
     * Updates an existing transcript file and its index metadata.
     *
     * @param historyId The ID of the history item to update.
     * @param segments The updated transcript segments.
     */
    async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<void> {
        try {
            const items = await this.getAll();
            const item = items.find(i => i.id === historyId);
            if (!item) {
                logger.error('[History] updateTranscript: item not found:', historyId);
                throw new Error(`History item not found: ${historyId}`);
            }

            // Overwrite transcript file
            await writeTextFile(
                `${HISTORY_DIR}/${item.transcriptPath}`,
                JSON.stringify(segments, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            const { previewText, searchContent } = buildHistoryTranscriptMetadata(segments);

            // Update item in index
            item.previewText = previewText;
            item.searchContent = searchContent;
            await writeHistoryIndex(items);
        } catch (error) {
            logger.error('[History] Failed to update transcript:', error);
            throw error;
        }
    },

    async updateItemMeta(id: string, updates: Partial<HistoryItem>): Promise<void> {
        const items = await this.getAll();
        let updated = false;
        
        const nextItems = items.map((item) => {
            if (item.id === id) {
                updated = true;
                return { ...item, ...updates };
            }
            return item;
        });

        if (updated) {
            await writeHistoryIndex(nextItems);
        } else {
            logger.warn(`[History] updateItemMeta: item not found for ID ${id}`);
        }
    },

    async updateProjectAssignments(ids: string[], projectId: string | null): Promise<void> {
        if (ids.length === 0) {
            return;
        }

        const targetIds = new Set(ids);
        const items = await this.getAll();
        const nextItems = items.map((item) => (
            targetIds.has(item.id) ? { ...item, projectId } : item
        ));
        await writeHistoryIndex(nextItems);
    },

    async updateProjectAssignmentsByCurrentProject(currentProjectId: string, nextProjectId: string | null): Promise<void> {
        const items = await this.getAll();
        const nextItems = items.map((item) => (
            item.projectId === currentProjectId ? { ...item, projectId: nextProjectId } : item
        ));
        await writeHistoryIndex(nextItems);
    },

    async loadSummary(historyId: string): Promise<HistorySummaryPayload | null> {
        try {
            const content = await readTextFile(`${HISTORY_DIR}/${historyId}.summary.json`, {
                baseDir: BaseDirectory.AppLocalData,
            });
            return JSON.parse(content);
        } catch (error) {
            if (!isMissingHistoryFileError(error)) {
                logger.error('[History] Failed to load summary sidecar:', error);
            }
            return null;
        }
    },

    async saveSummary(historyId: string, summaryPayload: HistorySummaryPayload): Promise<void> {
        try {
            await this.init();
            await writeTextFile(
                `${HISTORY_DIR}/${historyId}.summary.json`,
                JSON.stringify(summaryPayload, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            logger.error('[History] Failed to save summary sidecar:', error);
            throw error;
        }
    },

    async deleteSummary(historyId: string): Promise<void> {
        try {
            await safeRemoveHistoryPath(`${HISTORY_DIR}/${historyId}.summary.json`);
        } catch (error) {
            logger.error('[History] Failed to delete summary sidecar:', error);
        }
    },

    async getAudioAbsolutePath(filename: string): Promise<string | null> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const fullPath = await join(appDataDirPath, HISTORY_DIR, filename);

            // Check if file exists and has content
            try {
                const fileStat = await stat(`${HISTORY_DIR}/${filename}`, { baseDir: BaseDirectory.AppLocalData });
                if (fileStat.size === 0) {
                    logger.warn?.('[History] Audio file is empty:', filename);
                    return null;
                }
            } catch (e) {
                if (!isMissingHistoryFileError(e)) {
                    logger.error('[History] Audio file not found or inaccessible:', filename, e);
                }
                return null;
            }

            return fullPath;
        } catch (e) {
            logger.error('Failed to get audio absolute path:', e);
            return null;
        }
    },

    async getAudioUrl(filename: string): Promise<string | null> {
        try {
            const fullPath = await this.getAudioAbsolutePath(filename);
            if (!fullPath) return null;
            return convertFileSrc(fullPath);
        } catch (e) {
            logger.error('Failed to get audio URL:', e);
            return null;
        }
    },

    async openHistoryFolder(): Promise<void> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const historyPath = await join(appDataDirPath, HISTORY_DIR);
            logger.info('[History] Opening folder:', historyPath);
            await openPath(historyPath);
        } catch (error) {
            logger.error('[History] Failed to open history folder:', error);
        }
    }
};
