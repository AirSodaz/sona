import { describe, it, expect, vi, beforeEach } from 'vitest';
import { historyService } from '../historyService';
import { exists, remove, writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(),
    remove: vi.fn(),
    mkdir: vi.fn(),
    writeTextFile: vi.fn(),
    readTextFile: vi.fn(),
    copyFile: vi.fn(),
    writeFile: vi.fn(),
    stat: vi.fn(),
    BaseDirectory: { AppLocalData: 3 },
}));

vi.mock('@tauri-apps/api/path', () => ({
    appLocalDataDir: vi.fn().mockResolvedValue('/mock/app/data'),
    join: vi.fn().mockImplementation((...args) => Promise.resolve(args.join('/'))),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
    openPath: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: vi.fn().mockImplementation((path) => `asset://${path}`),
}));

vi.mock('../utils/logger', () => ({
    logger: {
        info: vi.fn(),
        error: vi.fn(),
    },
}));

describe('historyService Performance', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('benchmarks deleteRecordings for large number of items', async () => {
        const itemCount = 500;
        const mockItems = Array.from({ length: itemCount }).map((_, i) => ({
            id: String(i),
            audioPath: `audio${i}.wav`,
            transcriptPath: `transcript${i}.json`,
            projectId: null,
        }));
        const idsToDelete = mockItems.map(item => item.id);

        // Simulate async Tauri IPC calls with slight delay
        (exists as any).mockImplementation(async () => {
            await new Promise(r => setTimeout(r, 1));
            return true;
        });
        (remove as any).mockImplementation(async () => {
             await new Promise(r => setTimeout(r, 1));
             return;
        });
        (readTextFile as any).mockResolvedValue(JSON.stringify(mockItems));
        (writeTextFile as any).mockResolvedValue(undefined);

        const startTime = performance.now();
        await historyService.deleteRecordings(idsToDelete);
        const endTime = performance.now();

        console.log(`deleteRecordings for ${itemCount} items took: ${(endTime - startTime).toFixed(2)}ms`);

        expect(remove).toHaveBeenCalledTimes(itemCount * 3);
    });
});
