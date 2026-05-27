import { historyService } from './historyService';
import { useConfigStore } from '../stores/configStore';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import { projectService } from './projectService';
import { getPathStatusMap } from './pathStatusService';
import type { AppConfig } from '../types/config';
import { isHistoryItemDraft } from '../types/history';
import { logger } from '../utils/logger';
import { BaseDirectory, exists } from './tauri/platform/fs';

const HISTORY_DIR = 'history';
type ModelConfigKey = 'offlineModelPath' | 'streamingModelPath' | 'punctuationModelPath' | 'vadModelPath';
type ConfiguredModelField = { key: ModelConfigKey; path: string };

export interface HealthCheckServicePorts {
  historyService: typeof historyService;
  useConfigStore: typeof useConfigStore;
  settingsStore: typeof settingsStore;
  projectService: typeof projectService;
  getPathStatusMap: typeof getPathStatusMap;
  exists: typeof exists;
}

/**
 * Service to perform background health checks on application data.
 * Verifies consistency between database records and files on disk.
 */
export class HealthCheckService {
  constructor(private readonly ports: HealthCheckServicePorts) {}

  /**
   * Runs all health checks. Should be called at app startup.
   */
  runHealthCheck = async (): Promise<void> => {
    logger.info('[HealthCheck] Starting data consistency check...');
    try {
      // We run these sequentially or in parallel depending on their impact
      // History check can be slow if there are many items, so we check existence.
      await this.checkHistory();
      await this.checkModels();
      await this.checkProjects();

      logger.info('[HealthCheck] Health check completed.');
    } catch (error) {
      logger.error('[HealthCheck] Error during health check:', error);
    }
  }

  /**
   * Verifies that history items have at least one valid file (audio or transcript).
   * If both are missing, the record is considered "dead" and removed.
   */
  checkHistory = async (): Promise<void> => {
    const items = await this.ports.historyService.getAll();
    if (items.length === 0) return;

    const invalidIds: string[] = [];

    for (const item of items) {
      const audioPath = `${HISTORY_DIR}/${item.audioPath}`;
      const transcriptPath = `${HISTORY_DIR}/${item.transcriptPath}`;

      try {
        const transcriptExists = await this.ports.exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData });
        if (isHistoryItemDraft(item)) {
          if (!transcriptExists) {
            invalidIds.push(item.id);
          }
          continue;
        }

        const audioExists = await this.ports.exists(audioPath, { baseDir: BaseDirectory.AppLocalData });

        if (!audioExists && !transcriptExists) {
          invalidIds.push(item.id);
        }
      } catch (e) {
        // If checking fails (e.g. permission), we don't assume it's invalid
        logger.warn(`[HealthCheck] Failed to verify files for history item ${item.id}:`, e);
      }
    }

    if (invalidIds.length > 0) {
      logger.info(`[HealthCheck] Found ${invalidIds.length} invalid history records. Removing silently...`);
      try {
        // historyService.deleteRecordings handles index update and any remaining file cleanup
        await this.ports.historyService.deleteRecordings(invalidIds);
      } catch (e) {
        logger.error('[HealthCheck] Failed to remove invalid history records:', e);
      }
    }
  }

  /**
   * Verifies that configured model paths still exist on disk.
   * Clears invalid paths from the configuration to prevent engine errors.
   */
  checkModels = async (): Promise<void> => {
    const config = this.ports.useConfigStore.getState().config;
    const setConfig = this.ports.useConfigStore.getState().setConfig;
    const patch: Partial<Pick<AppConfig, ModelConfigKey>> = {};
    let changed = false;

    const modelFields = [
      { key: 'offlineModelPath', path: config.offlineModelPath },
      { key: 'streamingModelPath', path: config.streamingModelPath },
      { key: 'punctuationModelPath', path: config.punctuationModelPath },
      { key: 'vadModelPath', path: config.vadModelPath }
    ] as const;

    const configuredModelFields: ConfiguredModelField[] = modelFields
      .map((model) => ({
        key: model.key,
        path: model.path?.trim() || '',
      }))
      .filter((model) => model.path.length > 0);
    const pathStatusMap = await this.ports.getPathStatusMap(
      configuredModelFields.map((model) => model.path),
    );

    for (const model of configuredModelFields) {
      if (pathStatusMap[model.path]?.kind === 'missing') {
        logger.warn(`[HealthCheck] Model path not found: ${model.path}. Clearing field ${model.key}.`);
        patch[model.key] = '';
        changed = true;
      }
    }

    if (changed) {
      setConfig(patch);
      const updatedConfig = { ...this.ports.useConfigStore.getState().config, ...patch };
      await this.ports.settingsStore.set(STORE_KEY_CONFIG, updatedConfig);
      await this.ports.settingsStore.save();
    }
  }

  /**
   * Triggers project data normalization and migration.
   */
  checkProjects = async (): Promise<void> => {
    try {
      // projectService.getAll() automatically runs normalization and backfills
      await this.ports.projectService.getAll();
    } catch (e) {
      logger.error('[HealthCheck] Failed to check projects:', e);
    }
  }
}

export function createHealthCheckService(ports: HealthCheckServicePorts): HealthCheckService {
  return new HealthCheckService(ports);
}

export const healthCheckService = createHealthCheckService({
  historyService,
  useConfigStore,
  settingsStore,
  projectService,
  getPathStatusMap,
  exists,
});
