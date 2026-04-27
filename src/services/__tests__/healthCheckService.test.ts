import { describe, it, expect, vi, beforeEach } from 'vitest';
import { healthCheckService } from '../healthCheckService';
import { historyService } from '../historyService';
import { projectService } from '../projectService';
import { useConfigStore } from '../../stores/configStore';
import { exists } from '@tauri-apps/plugin-fs';
import { settingsStore } from '../storageService';

vi.mock('@tauri-apps/plugin-fs', () => ({
    exists: vi.fn(),
    BaseDirectory: { AppLocalData: 3 },
}));

vi.mock('../historyService', () => ({
    historyService: {
        getAll: vi.fn(),
        deleteRecordings: vi.fn(),
    },
}));

vi.mock('../projectService', () => ({
    projectService: {
        getAll: vi.fn(),
    },
}));

vi.mock('../storageService', () => ({
    settingsStore: {
        set: vi.fn(),
        save: vi.fn(),
    },
    STORE_KEY_CONFIG: 'sona-config',
}));

describe('healthCheckService', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('checkHistory', () => {
        it('should remove items where both audio and transcript are missing', async () => {
            const mockItems = [
                { id: '1', audioPath: 'a1.wav', transcriptPath: 't1.json' }, // valid (audio exists)
                { id: '2', audioPath: 'a2.wav', transcriptPath: 't2.json' }, // valid (transcript exists)
                { id: '3', audioPath: 'a3.wav', transcriptPath: 't3.json' }, // invalid (both missing)
                { id: '4', audioPath: 'a4.wav', transcriptPath: 't4.json' }, // valid (both exist)
            ];

            (historyService.getAll as any).mockResolvedValue(mockItems);
            
            (exists as any).mockImplementation((path: string) => {
                if (path.includes('a1.wav')) return Promise.resolve(true);
                if (path.includes('t2.json')) return Promise.resolve(true);
                if (path.includes('a3.wav')) return Promise.resolve(false);
                if (path.includes('t3.json')) return Promise.resolve(false);
                if (path.includes('a4.wav')) return Promise.resolve(true);
                if (path.includes('t4.json')) return Promise.resolve(true);
                return Promise.resolve(false);
            });

            await healthCheckService.checkHistory();

            expect(historyService.deleteRecordings).toHaveBeenCalledWith(['3']);
        });

        it('should do nothing if all items are valid', async () => {
            const mockItems = [
                { id: '1', audioPath: 'a1.wav', transcriptPath: 't1.json' },
            ];

            (historyService.getAll as any).mockResolvedValue(mockItems);
            (exists as any).mockResolvedValue(true);

            await healthCheckService.checkHistory();

            expect(historyService.deleteRecordings).not.toHaveBeenCalled();
        });
    });

    describe('checkModels', () => {
        it('should clear invalid model paths from config', async () => {
            useConfigStore.getState().setConfig({
                offlineModelPath: '/valid/path',
                streamingModelPath: '/invalid/path',
            });

            (exists as any).mockImplementation((path: string) => {
                if (path === '/valid/path') return Promise.resolve(true);
                if (path === '/invalid/path') return Promise.resolve(false);
                return Promise.resolve(false);
            });

            await healthCheckService.checkModels();

            expect(useConfigStore.getState().config.streamingModelPath).toBe('');
            expect(useConfigStore.getState().config.offlineModelPath).toBe('/valid/path');
            expect(settingsStore.set).toHaveBeenCalled();
            expect(settingsStore.save).toHaveBeenCalled();
        });
    });

    describe('checkProjects', () => {
        it('should call projectService.getAll', async () => {
            await healthCheckService.checkProjects();
            expect(projectService.getAll).toHaveBeenCalled();
        });
    });
});
