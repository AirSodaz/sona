import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import Database from '@tauri-apps/plugin-sql';
import { exists, readTextFile, writeTextFile, remove, rename } from '@tauri-apps/plugin-fs';

// Mock Dependencies
vi.mock('@tauri-apps/plugin-sql', () => {
    const mockDb = {
        execute: vi.fn().mockResolvedValue(undefined),
        select: vi.fn().mockResolvedValue([]),
    };
    return {
        default: {
            load: vi.fn().mockResolvedValue(mockDb),
        }
    };
});

vi.mock('@tauri-apps/plugin-fs', () => ({
    BaseDirectory: { AppLocalData: 'AppLocalData' },
    exists: vi.fn(),
    mkdir: vi.fn(),
    readTextFile: vi.fn(),
    writeTextFile: vi.fn(),
    writeFile: vi.fn(),
    remove: vi.fn(),
    copyFile: vi.fn(),
    stat: vi.fn().mockResolvedValue({ size: 100 }),
    rename: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
    appLocalDataDir: vi.fn().mockResolvedValue('/mock/app/data'),
    join: vi.fn((...args) => args.join('/')),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openPath: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: vi.fn((path) => `asset://${path}`),
}));

vi.mock('uuid', () => ({
    v4: () => 'mock-uuid'
}));

describe('HistoryService', () => {
    let mockDb: any;
    let historyService: any;

    beforeEach(async () => {
        vi.resetModules();
        const mod = await import('../historyService');
        historyService = mod.historyService;

        vi.clearAllMocks();
        // Reset DB mock
        mockDb = await Database.load('sqlite:test.db');
        (mockDb.execute as any).mockClear();
        (mockDb.select as any).mockClear();
    });

    it('initializes the database and creates table', async () => {
        (exists as any).mockResolvedValue(false); // No existing history dir or index

        await historyService.init();

        expect(Database.load).toHaveBeenCalledWith('sqlite:history.db');
        expect(mockDb.execute).toHaveBeenCalledWith(expect.stringContaining('CREATE TABLE IF NOT EXISTS history'));
    });

    it('migrates from index.json if it exists', async () => {
        (exists as any).mockImplementation((path: string) => {
            if (path.includes('index.json')) return Promise.resolve(true);
            return Promise.resolve(true);
        });

        const legacyItems = [
            { id: '1', timestamp: 100, title: 'Test 1', audioPath: '1.webm', transcriptPath: '1.json' }
        ];
        (readTextFile as any).mockResolvedValue(JSON.stringify(legacyItems));
        (mockDb.select as any).mockResolvedValue([]); // ID does not exist

        await historyService.init();

        expect(mockDb.execute).toHaveBeenCalledWith('BEGIN TRANSACTION');
        expect(mockDb.execute).toHaveBeenCalledWith(expect.stringContaining('INSERT INTO history'), expect.any(Array));
        expect(mockDb.execute).toHaveBeenCalledWith('COMMIT');
        expect(rename).toHaveBeenCalled();
    });

    it('saves a recording', async () => {
        (exists as any).mockResolvedValue(false); // No migration needed
        const audioBlob = {
            size: 10,
            arrayBuffer: vi.fn().mockResolvedValue(new ArrayBuffer(10))
        } as any;
        const segments = [{ text: 'Hello', start: 0, end: 1 }];

        await historyService.saveRecording(audioBlob, segments as any, 10);

        expect(mockDb.execute).toHaveBeenCalledWith(
            expect.stringContaining('INSERT INTO history'),
            expect.arrayContaining(['mock-uuid'])
        );
        expect(writeTextFile).toHaveBeenCalled(); // Transcripts saved to file
    });

    it('gets all items', async () => {
        const mockRows = [
            { id: '1', title: 'Test', timestamp: 100, audio_path: '1.webm', transcript_path: '1.json', type: 'recording' }
        ];
        (mockDb.select as any).mockResolvedValue(mockRows);

        const items = await historyService.getAll();

        expect(mockDb.select).toHaveBeenCalledWith(expect.stringContaining('SELECT * FROM history'));
        expect(items).toHaveLength(1);
        expect(items[0].id).toBe('1');
        expect(items[0].audioPath).toBe('1.webm'); // CamelCase conversion
    });

    it('deletes a recording', async () => {
        (exists as any).mockResolvedValue(true); // Files exist
        const mockRows = [
            { id: '1', title: 'Test', audio_path: '1.webm', transcript_path: '1.json' }
        ];
        (mockDb.select as any).mockResolvedValue(mockRows);

        await historyService.deleteRecording('1');

        expect(mockDb.execute).toHaveBeenCalledWith('DELETE FROM history WHERE id = $1', ['1']);
        expect(remove).toHaveBeenCalledTimes(2); // Audio + Transcript
    });
});
