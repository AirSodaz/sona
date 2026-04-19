import { logger } from "../utils/logger";
import { BaseDirectory, readTextFile, writeTextFile, writeFile, remove, exists, mkdir, copyFile, stat } from '@tauri-apps/plugin-fs';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
import { openPath } from '@tauri-apps/plugin-opener';
import { convertFileSrc } from '@tauri-apps/api/core';
import { TranscriptSegment } from '../types/transcript';
import { HistoryItem } from '../types/history';
import { v4 as uuidv4 } from 'uuid';

const HISTORY_DIR = 'history';
const INDEX_FILE = 'index.json';

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
            return JSON.parse(content);
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

        // Create metadata
        const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
        const searchContent = segments.map(s => s.text).join(' ');

        return { transcriptFileName, previewText, searchContent };
    },

    async saveNativeRecording(absoluteWavPath: string, segments: TranscriptSegment[], duration: number): Promise<HistoryItem | null> {
        logger.info('[History] Saving native recording...', { absoluteWavPath, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();
            const dateStr = new Date(timestamp).toISOString().split('T')[0];
            const timeStr = new Date(timestamp).toLocaleTimeString().replace(/:/g, '-');
            const title = `Recording ${dateStr} ${timeStr}`;

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
                searchContent
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            logger.info('[History] Native save complete:', newItem);
            return newItem;
        } catch (error) {
            logger.error('[History] Failed to save native recording:', error);
            return null;
        }
    },

    async saveRecording(audioBlob: Blob, segments: TranscriptSegment[], duration: number): Promise<HistoryItem | null> {
        logger.info('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            logger.info('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();
            const dateStr = new Date(timestamp).toISOString().split('T')[0];
            const timeStr = new Date(timestamp).toLocaleTimeString().replace(/:/g, '-');
            const title = `Recording ${dateStr} ${timeStr}`;

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
                searchContent
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            logger.info('[History] Save complete:', newItem);
            return newItem;
        } catch (error) {
            logger.error('[History] Failed to save recording:', error);
            return null;
        }
    },

    async saveImportedFile(filePath: string, segments: TranscriptSegment[], duration: number = 0, convertedFilePath?: string): Promise<HistoryItem | null> {
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
                searchContent
            };

            // Add to Index
            logger.info('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

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

                const safeRemove = async (path: string) => {
                    try {
                        await remove(path, { baseDir: BaseDirectory.AppLocalData });
                    } catch (e: any) {
                        // Ignore file not found errors, but log others
                        const errMsg = String(e);
                        if (!errMsg.includes('No such file or directory') && e?.code !== 'ENOENT') {
                            logger.error(`[History] Failed to remove file at ${path}:`, e);
                            throw e;
                        }
                    }
                };

                await Promise.all([
                    safeRemove(audioPath),
                    safeRemove(transcriptPath)
                ]);
            }

            const newItems = items.filter(item => item.id !== id);
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(newItems, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            logger.error('Failed to delete recording:', error);
        }
    },

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            // console.time('deleteRecordings');
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

                    const safeRemove = async (path: string) => {
                        try {
                            await remove(path, { baseDir: BaseDirectory.AppLocalData });
                        } catch (e: any) {
                            // Ignore file not found errors, but log others
                            const errMsg = String(e);
                            if (!errMsg.includes('No such file or directory') && e?.code !== 'ENOENT') {
                                logger.error(`[History] Failed to remove file at ${path}:`, e);
                                throw e; // Rethrow so Promise.allSettled can catch it
                            }
                        }
                    };

                    await Promise.all([
                        safeRemove(audioPath),
                        safeRemove(transcriptPath)
                    ]);
                }));
            }

            const newItems = items.filter(item => !idSet.has(item.id));
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(newItems, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
            console.timeEnd('deleteRecordings');
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
                return;
            }

            // Overwrite transcript file
            await writeTextFile(
                `${HISTORY_DIR}/${item.transcriptPath}`,
                JSON.stringify(segments, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            // Regenerate metadata
            const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
            const searchContent = segments.map(s => s.text).join(' ');

            // Update item in index
            item.previewText = previewText;
            item.searchContent = searchContent;
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            logger.error('[History] Failed to update transcript:', error);
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
                    logger.error('[History] Audio file is empty:', filename);
                    return null;
                }
            } catch (e) {
                logger.error('[History] Audio file not found or inaccessible:', filename, e);
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
