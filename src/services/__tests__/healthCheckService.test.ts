import { describe, it, expect, vi, beforeEach } from 'vitest';
import { healthCheckService } from '../healthCheckService';
import { historyService } from '../historyService';
import { projectService } from '../projectService';
import { useConfigStore } from '../../stores/configStore';
import { exists } from '@tauri-apps/plugin-fs';
import { settingsStore } from '../storageService';
import { getPathStatusMap } from '../pathStatusService';

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

vi.mock('../pathStatusService', () => ({
    getPathStatusMap: vi.fn(),
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

            (getPathStatusMap as any).mockResolvedValue({
                '/valid/path': { path: '/valid/path', kind: 'directory', error: null },
                '/invalid/path': { path: '/invalid/path', kind: 'missing', error: null },
            });

            await healthCheckService.checkModels();

            expect(useConfigStore.getState().config.streamingModelPath).toBe('');
            expect(useConfigStore.getState().config.offlineModelPath).toBe('/valid/path');
            expect(settingsStore.set).toHaveBeenCalled();
            expect(settingsStore.save).toHaveBeenCalled();
        });

        it('should keep configured model paths when runtime validation is unknown', async () => {
            useConfigStore.getState().setConfig({
                offlineModelPath: '/unknown/path',
            });

            (getPathStatusMap as any).mockResolvedValue({
                '/unknown/path': { path: '/unknown/path', kind: 'unknown', error: 'Scope denied' },
            });

            await healthCheckService.checkModels();

            expect(useConfigStore.getState().config.offlineModelPath).toBe('/unknown/path');
            expect(settingsStore.set).not.toHaveBeenCalled();
            expect(settingsStore.save).not.toHaveBeenCalled();
        });
    });

    describe('checkProjects', () => {
        it('should call projectService.getAll', async () => {
            await healthCheckService.checkProjects();
            expect(projectService.getAll).toHaveBeenCalled();
        });
    });
});
