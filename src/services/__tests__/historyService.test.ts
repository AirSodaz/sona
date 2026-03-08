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

describe('historyService.deleteRecordings', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('should delete specified recordings and update index', async () => {
        const mockItems = [
            { id: '1', audioPath: 'audio1.wav', transcriptPath: 'transcript1.json' },
            { id: '2', audioPath: 'audio2.wav', transcriptPath: 'transcript2.json' },
            { id: '3', audioPath: 'audio3.wav', transcriptPath: 'transcript3.json' },
        ];

        // Mock getAll indirectly via readTextFile
        (exists as any).mockResolvedValue(true);
        (readTextFile as any).mockResolvedValue(JSON.stringify(mockItems));
        (remove as any).mockResolvedValue(undefined);
        (writeTextFile as any).mockResolvedValue(undefined);

        await historyService.deleteRecordings(['1', '3']);

        // Check file deletions
        expect(remove).toHaveBeenCalledWith('history/audio1.wav', expect.anything());
        expect(remove).toHaveBeenCalledWith('history/transcript1.json', expect.anything());
        expect(remove).toHaveBeenCalledWith('history/audio3.wav', expect.anything());
        expect(remove).toHaveBeenCalledWith('history/transcript3.json', expect.anything());
        expect(remove).not.toHaveBeenCalledWith('history/audio2.wav', expect.anything());

        // Check index update
        const writeCall = (writeTextFile as any).mock.calls.find((call: any[]) => call[0] === 'history/index.json');
        expect(writeCall).toBeDefined();
        const updatedItems = JSON.parse(writeCall[1]);
        expect(updatedItems).toHaveLength(1);
        expect(updatedItems[0].id).toBe('2');
    });

    it('should handle empty ids array', async () => {
        const mockItems = [{ id: '1', audioPath: 'audio1.wav', transcriptPath: 'transcript1.json' }];
        (readTextFile as any).mockResolvedValue(JSON.stringify(mockItems));
        (exists as any).mockResolvedValue(true);

        await historyService.deleteRecordings([]);

        expect(remove).not.toHaveBeenCalled();
        const writeCall = (writeTextFile as any).mock.calls.find((call: any[]) => call[0] === 'history/index.json');
        const updatedItems = JSON.parse(writeCall[1]);
        expect(updatedItems).toHaveLength(1);
    });
});
