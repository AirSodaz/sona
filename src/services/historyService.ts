import { BaseDirectory, readTextFile, writeTextFile, writeFile, remove, exists, mkdir, copyFile, stat, rename } from '@tauri-apps/plugin-fs';
import { appLocalDataDir, join } from '@tauri-apps/api/path';
import { openPath } from '@tauri-apps/plugin-opener';
import { convertFileSrc } from '@tauri-apps/api/core';
import Database from '@tauri-apps/plugin-sql';
import { TranscriptSegment } from '../types/transcript';
import { HistoryItem } from '../types/history';
import { v4 as uuidv4 } from 'uuid';

const HISTORY_DIR = 'history';
const INDEX_FILE = 'index.json';
const DB_FILE = 'history.db';

let db: Database | null = null;
let isInitialized = false;

async function getDb(): Promise<Database> {
    if (!db) {
        // Load the database. This creates the file in AppLocalData if it doesn't exist.
        // We use 'sqlite:' prefix as required by the plugin.
        db = await Database.load(`sqlite:${DB_FILE}`);
    }
    return db;
}

export const historyService = {

    async init(): Promise<void> {
        if (isInitialized) return;

        try {
            // Ensure history directory exists for audio/transcript files
            const historyExists = await exists(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData });
            if (!historyExists) {
                await mkdir(HISTORY_DIR, { baseDir: BaseDirectory.AppLocalData, recursive: true });
            }

            const database = await getDb();

            // Create table
            // Note: SQLite types are flexible. REAL for duration, INTEGER for timestamp.
            await database.execute(`
                CREATE TABLE IF NOT EXISTS history (
                    id TEXT PRIMARY KEY,
                    timestamp INTEGER NOT NULL,
                    duration REAL,
                    audio_path TEXT,
                    transcript_path TEXT,
                    title TEXT,
                    preview_text TEXT,
                    type TEXT,
                    search_content TEXT
                )
            `);

            // Migration: Check if legacy index.json exists
            const indexExists = await exists(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
            if (indexExists) {
                console.log('[History] Found legacy index.json, migrating to SQLite...');
                try {
                    const content = await readTextFile(`${HISTORY_DIR}/${INDEX_FILE}`, { baseDir: BaseDirectory.AppLocalData });
                    const items: HistoryItem[] = JSON.parse(content);

                    if (items.length > 0) {
                        // Begin transaction
                        await database.execute('BEGIN TRANSACTION');

                        for (const item of items) {
                            // Check if ID exists to avoid duplicates during partial migration re-run
                            const existing = await database.select<any[]>('SELECT id FROM history WHERE id = $1', [item.id]);
                            if (existing.length === 0) {
                                await database.execute(
                                    `INSERT INTO history (id, timestamp, duration, audio_path, transcript_path, title, preview_text, type, search_content)
                                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`,
                                    [
                                        item.id,
                                        item.timestamp,
                                        item.duration,
                                        item.audioPath,
                                        item.transcriptPath,
                                        item.title,
                                        item.previewText || '',
                                        item.type || 'recording',
                                        item.searchContent || ''
                                    ]
                                );
                            }
                        }

                        await database.execute('COMMIT');
                    }

                    console.log('[History] Migration complete. Renaming index.json to index.json.bak');
                    await rename(
                        `${HISTORY_DIR}/${INDEX_FILE}`,
                        `${HISTORY_DIR}/${INDEX_FILE}.bak`,
                        { oldPathBaseDir: BaseDirectory.AppLocalData, newPathBaseDir: BaseDirectory.AppLocalData }
                    );

                } catch (migrationError) {
                    console.error('[History] Migration failed:', migrationError);
                    try {
                        await database.execute('ROLLBACK');
                    } catch (rollbackError) {
                         console.error('[History] Rollback failed:', rollbackError);
                    }
                }
            }

            isInitialized = true;
        } catch (error) {
            console.error('[History] Failed to initialize service:', error);
            throw error;
        }
    },

    async getAll(): Promise<HistoryItem[]> {
        try {
            await this.init(); // Ensure DB is ready
            const database = await getDb();
            const rows = await database.select<any[]>('SELECT * FROM history ORDER BY timestamp DESC');

            return rows.map(row => ({
                id: row.id,
                timestamp: row.timestamp,
                duration: row.duration,
                audioPath: row.audio_path,
                transcriptPath: row.transcript_path,
                title: row.title,
                previewText: row.preview_text,
                type: row.type as 'recording' | 'batch',
                searchContent: row.search_content
            }));
        } catch (error) {
            console.error('[History] Failed to load history:', error);
            return [];
        }
    },

    // Used internally by saveRecording/saveImportedFile to save the transcript FILE and return metadata
    // We keep saving the transcript file for backup/portability reasons,
    // but the source of truth for the list is now the DB.
    async saveTranscriptFile(id: string, segments: TranscriptSegment[]) {
        const transcriptFileName = `${id}.json`;
        const transcriptPathDisplay = `${HISTORY_DIR}/${transcriptFileName}`;
        console.log('[History] Writing transcript file:', transcriptPathDisplay);
        await writeTextFile(
            transcriptPathDisplay,
            JSON.stringify(segments, null, 2),
            { baseDir: BaseDirectory.AppLocalData }
        );

        const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
        const searchContent = segments.map(s => s.text).join(' ');

        return { transcriptFileName, previewText, searchContent };
    },

    async saveRecording(audioBlob: Blob, segments: TranscriptSegment[], duration: number): Promise<HistoryItem | null> {
        console.log('[History] Saving recording...', { blobSize: audioBlob.size, segments: segments.length, duration });

        if (!segments || segments.length === 0) {
            console.log('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const database = await getDb();
            const id = uuidv4();
            const timestamp = Date.now();
            const dateStr = new Date(timestamp).toISOString().split('T')[0];
            const timeStr = new Date(timestamp).toLocaleTimeString().replace(/:/g, '-');
            const title = `Recording ${dateStr} ${timeStr}`;

            // Save Audio File
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

            // Save Transcript File and get metadata
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

            // Insert into DB
            await database.execute(
                `INSERT INTO history (id, timestamp, duration, audio_path, transcript_path, title, preview_text, type, search_content)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`,
                [
                    newItem.id,
                    newItem.timestamp,
                    newItem.duration,
                    newItem.audioPath,
                    newItem.transcriptPath,
                    newItem.title,
                    newItem.previewText,
                    newItem.type,
                    newItem.searchContent
                ]
            );

            console.log('[History] Save complete:', newItem);
            return newItem;
        } catch (error) {
            console.error('[History] Failed to save recording:', error);
            return null;
        }
    },

    async saveImportedFile(filePath: string, segments: TranscriptSegment[], duration: number = 0, convertedFilePath?: string): Promise<HistoryItem | null> {
        console.log('[History] Saving imported file...', { filePath, segments: segments.length });

        if (!segments || segments.length === 0) {
            console.log('[History] Empty transcript, skipping save.');
            return null;
        }

        try {
            await this.init();
            const database = await getDb();
            const id = uuidv4();
            const timestamp = Date.now();

            const filename = filePath.split(/[/\\]/).pop() || 'Imported File';
            const title = `Batch ${filename}`;

            const targetExt = convertedFilePath ? 'wav' : (filename.split('.').pop() || 'wav');
            const audioFileName = `${id}.${targetExt}`;
            const audioPathDisplay = `${HISTORY_DIR}/${audioFileName}`;

            console.log('[History] Copying audio file to:', audioPathDisplay);
            await copyFile(
                convertedFilePath || filePath,
                audioPathDisplay,
                { toPathBaseDir: BaseDirectory.AppLocalData }
            );

            const { transcriptFileName, previewText, searchContent } = await this.saveTranscriptFile(id, segments);

            const newItem: HistoryItem = {
                id,
                timestamp,
                duration,
                audioPath: audioFileName,
                transcriptPath: transcriptFileName,
                title,
                previewText,
                type: 'batch',
                searchContent
            };

            await database.execute(
                `INSERT INTO history (id, timestamp, duration, audio_path, transcript_path, title, preview_text, type, search_content)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`,
                [
                    newItem.id,
                    newItem.timestamp,
                    newItem.duration,
                    newItem.audioPath,
                    newItem.transcriptPath,
                    newItem.title,
                    newItem.previewText,
                    newItem.type,
                    newItem.searchContent
                ]
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
            await this.init();
            const database = await getDb();

            // Get file paths to delete
            const result = await database.select<any[]>('SELECT audio_path, transcript_path FROM history WHERE id = $1', [id]);
            const itemToDelete = result[0];

            if (itemToDelete) {
                const audioPath = `${HISTORY_DIR}/${itemToDelete.audio_path}`;
                const transcriptPath = `${HISTORY_DIR}/${itemToDelete.transcript_path}`;

                if (await exists(audioPath, { baseDir: BaseDirectory.AppLocalData })) {
                    await remove(audioPath, { baseDir: BaseDirectory.AppLocalData });
                }

                if (await exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData })) {
                    await remove(transcriptPath, { baseDir: BaseDirectory.AppLocalData });
                }
            }

            await database.execute('DELETE FROM history WHERE id = $1', [id]);
        } catch (error) {
            console.error('Failed to delete recording:', error);
        }
    },

    async deleteRecordings(ids: string[]): Promise<void> {
        try {
            if (ids.length === 0) return;

            await this.init();
            const database = await getDb();

            // Get file paths to delete
            const placeholders = ids.map((_, i) => `$${i + 1}`).join(',');
            const itemsToDelete = await database.select<any[]>(`SELECT id, audio_path, transcript_path FROM history WHERE id IN (${placeholders})`, ids);

            for (const item of itemsToDelete) {
                const audioPath = `${HISTORY_DIR}/${item.audio_path}`;
                const transcriptPath = `${HISTORY_DIR}/${item.transcript_path}`;

                try {
                    if (await exists(audioPath, { baseDir: BaseDirectory.AppLocalData })) {
                        await remove(audioPath, { baseDir: BaseDirectory.AppLocalData });
                    }
                    if (await exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData })) {
                        await remove(transcriptPath, { baseDir: BaseDirectory.AppLocalData });
                    }
                } catch (e) {
                    console.error(`Failed to delete files for item ${item.id}`, e);
                }
            }

            // Batch delete
            await database.execute(`DELETE FROM history WHERE id IN (${placeholders})`, ids);

        } catch (error) {
            console.error('Failed to delete recordings:', error);
            throw error;
        }
    },

    async loadTranscript(filename: string): Promise<TranscriptSegment[] | null> {
        try {
            const path = `${HISTORY_DIR}/${filename}`;
            const content = await readTextFile(path, { baseDir: BaseDirectory.AppLocalData });
            return JSON.parse(content);
        } catch (error) {
            console.error('Failed to load transcript:', error);
            return null;
        }
    },

    async updateTranscript(historyId: string, segments: TranscriptSegment[]): Promise<void> {
        try {
            const database = await getDb();
            const result = await database.select<any[]>('SELECT * FROM history WHERE id = $1', [historyId]);
            const item = result[0];

            if (!item) {
                console.error('[History] updateTranscript: item not found:', historyId);
                return;
            }

            // Overwrite transcript file
            await writeTextFile(
                `${HISTORY_DIR}/${item.transcript_path}`,
                JSON.stringify(segments, null, 2),
                { baseDir: BaseDirectory.AppLocalData }
            );

            const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
            const searchContent = segments.map(s => s.text).join(' ');

            await database.execute(
                'UPDATE history SET preview_text = $1, search_content = $2 WHERE id = $3',
                [previewText, searchContent, historyId]
            );
        } catch (error) {
            console.error('[History] Failed to update transcript:', error);
        }
    },

    async getAudioUrl(filename: string): Promise<string | null> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const fullPath = await join(appDataDirPath, HISTORY_DIR, filename);

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
    },

    async openHistoryFolder(): Promise<void> {
        try {
            const appDataDirPath = await appLocalDataDir();
            const historyPath = await join(appDataDirPath, HISTORY_DIR);
            console.log('[History] Opening folder:', historyPath);
            await openPath(historyPath);
        } catch (error) {
            console.error('[History] Failed to open history folder:', error);
        }
    }
};
