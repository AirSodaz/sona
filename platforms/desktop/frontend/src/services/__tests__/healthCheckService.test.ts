import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfigStore } from '../../stores/configStore';
import { healthCheckService } from '../healthCheckService';
import { getPathStatusMap } from '../pathStatusService';
import { projectService } from '../projectService';
import { settingsStore } from '../storageService';

vi.mock('../projectService', () => ({
  projectService: {
    getAll: vi.fn(),
  },
}));

vi.mock('../storageService', () => ({
  settingsStore: {
    set: vi.fn(),
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

  describe('runHealthCheck', () => {
    it('should run models and projects checks without touching history records', async () => {
      (projectService.getAll as any).mockResolvedValue([]);
      (getPathStatusMap as any).mockResolvedValue({});

      await healthCheckService.runHealthCheck();

      expect(projectService.getAll).toHaveBeenCalled();
    });
  });

  describe('checkModels', () => {
    it('should clear invalid model paths from config', async () => {
      useConfigStore.getState().setConfig({
        batchModelPath: '/valid/path',
        streamingModelPath: '/invalid/path',
      });

      (getPathStatusMap as any).mockResolvedValue({
        '/valid/path': { path: '/valid/path', kind: 'directory', error: null },
        '/invalid/path': { path: '/invalid/path', kind: 'missing', error: null },
      });

      await healthCheckService.checkModels();

      expect(useConfigStore.getState().config.streamingModelPath).toBe('');
      expect(useConfigStore.getState().config.batchModelPath).toBe('/valid/path');
      expect(settingsStore.set).toHaveBeenCalled();
    });

    it('should keep configured model paths when runtime validation is unknown', async () => {
      useConfigStore.getState().setConfig({
        batchModelPath: '/unknown/path',
      });

      (getPathStatusMap as any).mockResolvedValue({
        '/unknown/path': { path: '/unknown/path', kind: 'unknown', error: 'Scope denied' },
      });

      await healthCheckService.checkModels();

      expect(useConfigStore.getState().config.batchModelPath).toBe('/unknown/path');
      expect(settingsStore.set).not.toHaveBeenCalled();
    });
  });

  describe('checkProjects', () => {
    it('should call projectService.getAll', async () => {
      await healthCheckService.checkProjects();
      expect(projectService.getAll).toHaveBeenCalled();
    });
  });
});
