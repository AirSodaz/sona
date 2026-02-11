import { BaseDirectory, readTextFile, writeTextFile, writeFile, remove, exists, mkdir, copyFile, stat } from '@tauri-apps/plugin-fs';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
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
                console.log('[History] Creating index file');
                await writeTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, '[]', { baseDir: BaseDirectory.AppLocalData });
            }
        } catch (error) {
            console.error('[History] Failed to initialize service:', error);
            throw error; // Re-throw to see it upstream
        }
    },

    async getAll(): Promise<HistoryItem[]> {
        try {
            console.log('[History] Getting all items');
            await this.init();
            const content = await readTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
            console.log('[History] Loaded index:', content?.substring(0, 50));
            return JSON.parse(content);
        } catch (error) {
            console.error('[History] Failed to load history:', error);
            return [];
        }
    },

    async saveRecording(audioBlob: Blob, segments: TranscriptSegment[], duration: number): Promise<HistoryItem | null> {
        console.log('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });
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

            console.log('[History] Writing audio file:', audioPathDisplay);
            await writeFile(
                audioPathDisplay,
                uint8Array,
                { baseDir: BaseDirectory.AppLocalData }
            );

            // Save Transcript
            const transcriptFileName = `${id}.json`;
            const transcriptPathDisplay = `${HISTORY_DIR}/${transcriptFileName}`;
            console.log('[History] Writing transcript file:', transcriptPathDisplay);
            await writeTextFile(
                transcriptPathDisplay,
                JSON.stringify(segments, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            // Create Item
            const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
            const searchContent = segments.map(s => s.text).join(' ');

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
            console.log('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            console.log('[History] Save complete:', newItem);
            return newItem;
        } catch (error) {
            console.error('[History] Failed to save recording:', error);
            return null;
        }
    },

    async saveImportedFile(filePath: string, segments: TranscriptSegment[], duration: number = 0): Promise<HistoryItem | null> {
        console.log('[History] Saving imported file...', { filePath, segments: segments.length });
        try {
            await this.init();
            const id = uuidv4();
            const timestamp = Date.now();

            // Get filename from path
            const filename = filePath.split(/[/\\]/).pop() || 'Imported File';
            const ext = filename.split('.').pop() || 'wav'; // Default fallback

            // Generate title with Batch prefix
            const title = `Batch ${filename}`;

            // Save Audio (Copy)
            const audioFileName = `${id}.${ext}`;
            const audioPathDisplay = `${HISTORY_DIR}/${audioFileName}`;

            console.log('[History] Copying audio file to:', audioPathDisplay);
            // Copy the file to the history directory
            // We use copyFile from plugin-fs to copy from specific path to AppLocalData
            await copyFile(
                filePath,
                audioPathDisplay,
                { toPathBaseDir: BaseDirectory.AppLocalData }
            );

            // Save Transcript
            const transcriptFileName = `${id}.json`;
            const transcriptPathDisplay = `${HISTORY_DIR}/${transcriptFileName}`;
            console.log('[History] Writing transcript file:', transcriptPathDisplay);
            await writeTextFile(
                transcriptPathDisplay,
                JSON.stringify(segments, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            // Create Item
            const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
            const searchContent = segments.map(s => s.text).join(' ');

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
            console.log('[History] Updating index');
            const items = await this.getAll();
            items.unshift(newItem); // Add to beginning
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(items, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            console.log('[History] Import save complete:', newItem);
            return newItem;

        } catch (error) {
            console.error('[History] Failed to save imported file:', error);
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

                if (await exists(audioPath, { baseDir: BaseDirectory.AppLocalData })) {
                    await remove(audioPath, { baseDir: BaseDirectory.AppLocalData });
                }

                if (await exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData })) {
                    await remove(transcriptPath, { baseDir: BaseDirectory.AppLocalData });
                }
            }

            const newItems = items.filter(item => item.id !== id);
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(newItems, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            console.error('Failed to delete recording:', error);
        }
    },

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            const items = await this.getAll();
            const itemsToDelete = items.filter(item => ids.includes(item.id));

            for (const item of itemsToDelete) {
                const audioPath = `${HISTORY_DIR}/${item.audioPath}`;
                const transcriptPath = `${HISTORY_DIR}/${item.transcriptPath}`;

                try {
                    if (await exists(audioPath, { baseDir: BaseDirectory.AppLocalData })) {
                        await remove(audioPath, { baseDir: BaseDirectory.AppLocalData });
                    }
                    if (await exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData })) {
                        await remove(transcriptPath, { baseDir: BaseDirectory.AppLocalData });
                    }
                } catch (e) {
                    console.error(`Failed to delete files for item ${item.id}`, e);
                    // Continue deleting others even if one fails
                }
            }

            const newItems = items.filter(item => !ids.includes(item.id));
            await writeTextFile(
                `${HISTORY_DIR}/${INDEX_FILE}`,
                JSON.stringify(newItems, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            console.error('Failed to delete recordings:', error);
            throw error;
        }
    },

    async loadTranscript(filename: string): Promise<TranscriptSegment[]> {
        try {
            const path = `${HISTORY_DIR}/${filename}`;
            const content = await readTextFile(path, { baseDir: BaseDirectory.AppLocalData });
            return JSON.parse(content);
        } catch (error) {
            console.error('Failed to load transcript:', error);
            return [];
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
                console.error('[History] updateTranscript: item not found:', historyId);
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
            console.error('[History] Failed to update transcript:', error);
        }
    },

    async getAudioUrl(filename: string): Promise<string | null> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const fullPath = await join(appDataDirPath, HISTORY_DIR, filename);

            // Check if file exists and has content
            try {
                const fileStat = await stat(`${HISTORY_DIR}/${filename}`, { baseDir: BaseDirectory.AppLocalData });
                if (fileStat.size === 0) {
                    console.error('[History] Audio file is empty:', filename);
                    return null;
                }
            } catch (e) {
                console.error('[History] Audio file not found or inaccessible:', filename, e);
                return null;
            }

            return convertFileSrc(fullPath);
        } catch (e) {
            console.error('Failed to get audio URL:', e);
            return null;
        }
    }
};
