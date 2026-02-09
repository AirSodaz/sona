import { describe, it, expect, vi, beforeEach } from 'vitest';
import { historyService } from '../historyService';

// Mock data store
let mockFiles: Record<string, string | Uint8Array> = {};

// Mock @tauri-apps/plugin-fs
vi.mock('@tauri-apps/plugin-fs', () => ({
    BaseDirectory: {
        AppLocalData: 'AppLocalData',
    },
    exists: vi.fn(async (path: string) => {
        return path in mockFiles;
    }),
    mkdir: vi.fn(async () => {}),
    readTextFile: vi.fn(async (path: string) => {
        if (path in mockFiles) return mockFiles[path] as string;
        throw new Error(`File not found: ${path}`);
    }),
    writeTextFile: vi.fn(async (path: string, content: string, options?: { append?: boolean }) => {
        if (options?.append) {
            mockFiles[path] = ((mockFiles[path] as string) || '') + content;
        } else {
            mockFiles[path] = content;
        }
    }),
    writeFile: vi.fn(async (path: string, content: Uint8Array) => {
        mockFiles[path] = content;
    }),
    remove: vi.fn(async (path: string) => {
        delete mockFiles[path];
    }),
    copyFile: vi.fn(async () => {}),
}));

// Mock @tauri-apps/api/path
vi.mock('@tauri-apps/api/path', () => ({
    appLocalDataDir: async () => '/mock/app/data',
    join: async (...args: string[]) => args.join('/'),
}));

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string) => `asset://${path}`,
}));

// Mock uuid
vi.mock('uuid', () => ({
    v4: () => 'mock-uuid-' + Math.random().toString(36).substring(7),
}));

describe('History Service Functional Tests', () => {
    beforeEach(() => {
        mockFiles = {};
        historyService._resetCache();
        vi.clearAllMocks();
    });

    it('should initialize empty history if no files exist', async () => {
        const items = await historyService.getAll();
        expect(items).toEqual([]);
        expect(mockFiles['history/history.jsonl']).toBeDefined();
        expect(mockFiles['history/history.jsonl']).toBe('');
    });

    it('should migrate from index.json to history.jsonl correctly', async () => {
        // Setup old format: [newest, older, oldest]
        const oldItems = [
            { id: '3', title: 'Newest' },
            { id: '2', title: 'Older' },
            { id: '1', title: 'Oldest' }
        ];
        mockFiles['history/index.json'] = JSON.stringify(oldItems);

        // Call getAll to trigger migration
        const items = await historyService.getAll();

        // Check returned items (should be Newest first)
        expect(items.length).toBe(3);
        expect(items[0].id).toBe('3');
        expect(items[2].id).toBe('1');

        // Check file storage (should be history.jsonl, chronological order: Oldest -> Newest)
        expect(mockFiles['history/index.json']).toBeUndefined();
        const content = mockFiles['history/history.jsonl'] as string;
        const lines = content.split('\n').filter(l => l.trim());
        expect(lines.length).toBe(3);
        expect(JSON.parse(lines[0]).id).toBe('1'); // Oldest first in file
        expect(JSON.parse(lines[2]).id).toBe('3'); // Newest last in file
    });

    it('should save a new recording efficiently (append)', async () => {
        // Setup existing history.jsonl with 1 item
        const existingItem = { id: '1', title: 'Existing' };
        mockFiles['history/history.jsonl'] = JSON.stringify(existingItem) + '\n';

        // Pre-load cache
        await historyService.getAll();

        // Save new recording
        const blob = {
            size: 100,
            arrayBuffer: async () => new Uint8Array(100).buffer
        } as unknown as Blob;

        const segments = [{
            id: 'seg-1',
            start: 0,
            end: 1,
            text: 'New',
            isFinal: true
        }];

        // @ts-ignore
        await historyService.saveRecording(blob, segments, 10);

        // Check file content
        const content = mockFiles['history/history.jsonl'] as string;
        const lines = content.split('\n').filter(l => l.trim());
        expect(lines.length).toBe(2);
        expect(JSON.parse(lines[0]).id).toBe('1');
        // New item should be last (appended)
        expect(JSON.parse(lines[1]).title).toContain('Recording');

        // Check cache (should be updated and reversed: New -> Old)
        const items = await historyService.getAll();
        expect(items.length).toBe(2);
        expect(items[0].title).toContain('Recording'); // Newest first
        expect(items[1].id).toBe('1');
    });

    it('should delete a recording correctly', async () => {
        // Setup 3 items in history.jsonl: [Oldest, Middle, Newest]
        const itemsInFile = [
            { id: '1', title: 'Oldest', audioPath: '1.webm', transcriptPath: '1.json' },
            { id: '2', title: 'Middle', audioPath: '2.webm', transcriptPath: '2.json' },
            { id: '3', title: 'Newest', audioPath: '3.webm', transcriptPath: '3.json' }
        ];
        mockFiles['history/history.jsonl'] = itemsInFile.map(i => JSON.stringify(i)).join('\n') + '\n';
        // Mock files existence
        mockFiles['history/1.webm'] = 'audio1';
        mockFiles['history/1.json'] = 'trans1';
        mockFiles['history/2.webm'] = 'audio2';
        mockFiles['history/2.json'] = 'trans2';
        mockFiles['history/3.webm'] = 'audio3';
        mockFiles['history/3.json'] = 'trans3';

        // Load cache
        await historyService.getAll();

        // Delete middle item
        await historyService.deleteRecording('2');

        // Verify file content rewritten (should contain 1 and 3)
        const content = mockFiles['history/history.jsonl'] as string;
        const lines = content.split('\n').filter(l => l.trim());
        expect(lines.length).toBe(2);
        expect(JSON.parse(lines[0]).id).toBe('1');
        expect(JSON.parse(lines[1]).id).toBe('3');

        // Verify files deleted
        expect(mockFiles['history/2.webm']).toBeUndefined();
        expect(mockFiles['history/2.json']).toBeUndefined();
        // Others remain
        expect(mockFiles['history/1.webm']).toBeDefined();
        expect(mockFiles['history/3.webm']).toBeDefined();

        // Verify cache updated
        const items = await historyService.getAll();
        expect(items.length).toBe(2);
        expect(items.find(i => i.id === '2')).toBeUndefined();
    });

    it('should update a recording efficiently (append updated version)', async () => {
        // Setup existing history.jsonl with 1 item
        const existingItem = { id: '1', title: 'Original Title' };
        mockFiles['history/history.jsonl'] = JSON.stringify(existingItem) + '\n';

        // Pre-load cache
        await historyService.getAll();

        // Update item
        await historyService.updateHistoryItem('1', { title: 'Updated Title' });

        // Check file content (should have 2 lines: Original, then Updated)
        const content = mockFiles['history/history.jsonl'] as string;
        const lines = content.split('\n').filter(l => l.trim());
        expect(lines.length).toBe(2);
        expect(JSON.parse(lines[0]).title).toBe('Original Title');
        expect(JSON.parse(lines[1]).title).toBe('Updated Title');
        expect(JSON.parse(lines[1]).id).toBe('1');

        // Check cache (should reflect update)
        const items = await historyService.getAll();
        expect(items.length).toBe(1); // Should only have 1 item (deduplicated)
        expect(items[0].title).toBe('Updated Title');
        expect(items[0].id).toBe('1');
    });

    it('should handle multiple updates correctly', async () => {
        const item = { id: '1', title: 'v1' };
        mockFiles['history/history.jsonl'] = JSON.stringify(item) + '\n';
        await historyService.getAll();

        await historyService.updateHistoryItem('1', { title: 'v2' });
        await historyService.updateHistoryItem('1', { title: 'v3' });

        const items = await historyService.getAll();
        expect(items.length).toBe(1);
        expect(items[0].title).toBe('v3');

        // File should have 3 lines
        const content = mockFiles['history/history.jsonl'] as string;
        const lines = content.split('\n').filter(l => l.trim());
        expect(lines.length).toBe(3);
        expect(JSON.parse(lines[2]).title).toBe('v3');
    });
});
