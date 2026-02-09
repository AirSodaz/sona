import { BaseDirectory, readTextFile, writeTextFile, writeFile, remove, exists, mkdir, copyFile } from '@tauri-apps/plugin-fs';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
import { convertFileSrc } from '@tauri-apps/api/core';
import { TranscriptSegment } from '../types/transcript';
import { HistoryItem } from '../types/history';
import { v4 as uuidv4 } from 'uuid';

const HISTORY_DIR = 'history';
const INDEX_FILE = 'index.json';
const HISTORY_FILE = 'history.jsonl';

// In-memory cache
let _cache: HistoryItem[] | null = null;

export const historyService = {

    async init(): Promise<void> {
        try {
            const historyDirExists = await exists(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData });
            if (!historyDirExists) {
                await mkdir(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
            }

            // Migration Logic
            const historyFileExists = await exists(`${HISTORY_DIR}/${HISTORY_FILE}`, { baseDir: BaseDirectory.AppLocalData });

            if (!historyFileExists) {
                const indexExists = await exists(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
                if (indexExists) {
                     console.log('[History] Migrating index.json to history.jsonl');
                     try {
                         const content = await readTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
                         let items: HistoryItem[] = [];
                         try {
                             items = JSON.parse(content);
                         } catch (e) {
                             console.error('[History] Failed to parse old index.json during migration', e);
                         }

                         // Items in index.json are usually [newest, ..., oldest]
                         // We want append-only file to be chronological [oldest, ..., newest] so we can append new items at the end.
                         items.reverse();

                         let newContent = '';
                         for (const item of items) {
                             newContent += JSON.stringify(item) + '\n';
                         }

                         await writeTextFile(`${HISTORY_DIR}/${HISTORY_FILE}`, newContent, { baseDir: BaseDirectory.AppLocalData });
                         // We can delete the old file, or rename it as backup.
                         // Let's delete it to complete migration.
                         await remove(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
                         console.log('[History] Migration complete');
                     } catch (e) {
                         console.error('[History] Migration failed:', e);
                     }
                } else {
                    console.log('[History] Creating history file');
                    await writeTextFile(`${HISTORY_DIR}/${HISTORY_FILE}`, '', { baseDir: BaseDirectory.AppLocalData });
                }
            }
        } catch (error) {
            console.error('[History] Failed to initialize service:', error);
            throw error; // Re-throw to see it upstream
        }
    },

    async getAll(): Promise<HistoryItem[]> {
        try {
            if (_cache) {
                // console.log('[History] Returning cached items');
                return _cache;
            }

            console.log('[History] Getting all items (reading from disk)');
            await this.init();

            const content = await readTextFile(`${HISTORY_DIR}/${HISTORY_FILE}`, { baseDir: BaseDirectory.AppLocalData });
            if (!content) {
                _cache = [];
                return [];
            }

            const items: HistoryItem[] = [];
            const lines = content.split('\n');
            for (const line of lines) {
                if (line.trim()) {
                    try {
                        items.push(JSON.parse(line));
                    } catch (e) {
                        console.error('[History] Failed to parse line:', line);
                    }
                }
            }

            // The file is chronological (oldest first). We want newest first.
            items.reverse();

            _cache = items;
            console.log(`[History] Loaded ${items.length} items`);
            return items;
        } catch (error) {
            console.error('[History] Failed to load history:', error);
            return [];
        }
    },

    async saveRecording(audioBlob: Blob, segments: TranscriptSegment[], duration: number): Promise<HistoryItem | null> {
        console.log('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });
        try {
            // Ensure cache is populated
            if (!_cache) {
                await this.getAll();
            } else {
                 // Ensure init is called at least once
                 await this.init();
            }

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

            // Update Index (Append)
            console.log('[History] Updating index (append)');

            // Update cache
            if (_cache) {
                _cache.unshift(newItem);
            }

            // Append to file
            await writeTextFile(
                `${HISTORY_DIR}/${HISTORY_FILE}`,
                JSON.stringify(newItem) + '\n',
                { baseDir: BaseDirectory.AppLocalData, append: true }
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
             // Ensure cache is populated
            if (!_cache) {
                await this.getAll();
            } else {
                 await this.init();
            }

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

            // Update Index (Append)
            console.log('[History] Updating index (append)');

            if (_cache) {
                _cache.unshift(newItem);
            }

            await writeTextFile(
                `${HISTORY_DIR}/${HISTORY_FILE}`,
                JSON.stringify(newItem) + '\n',
                { baseDir: BaseDirectory.AppLocalData, append: true }
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
            // Ensure cache is populated
            if (!_cache) {
                await this.getAll();
            } else {
                 await this.init();
            }

            const items = _cache || [];
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
            _cache = newItems;

            // Rewrite file
            // _cache is [newest, ..., oldest]
            // We want [oldest, ..., newest] in file
            const itemsToWrite = [...newItems].reverse();
            let newContent = '';
            for (const item of itemsToWrite) {
                newContent += JSON.stringify(item) + '\n';
            }

            await writeTextFile(
                `${HISTORY_DIR}/${HISTORY_FILE}`,
                newContent,
                { baseDir: BaseDirectory.AppLocalData }
            );
        } catch (error) {
            console.error('Failed to delete recording:', error);
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

    async getAudioUrl(filename: string): Promise<string | null> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const fullPath = await join(appDataDirPath, HISTORY_DIR, filename);
            return convertFileSrc(fullPath);
        } catch (e) {
            console.error('Failed to get audio URL:', e);
            return null;
        }
    },

    // For testing purposes
    _resetCache(): void {
        _cache = null;
    }
};
