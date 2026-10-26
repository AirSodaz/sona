import { useConfigStore } from '../stores/configStore';
import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';
import { getPathStatusMap } from './pathStatusService';
import { projectService } from './projectService';
import { STORE_KEY_CONFIG, settingsStore } from './storageService';

type ModelConfigKey =
  | 'batchModelPath'
  | 'streamingModelPath'
  | 'livePunctuationModelPath'
  | 'liveVadModelPath'
  | 'liveSpeakerSegmentationModelPath'
  | 'liveSpeakerEmbeddingModelPath'
  | 'liveAlignmentModelPath'
  | 'batchPunctuationModelPath'
  | 'batchVadModelPath'
  | 'batchSpeakerSegmentationModelPath'
  | 'batchSpeakerEmbeddingModelPath'
  | 'batchAlignmentModelPath';
type ConfiguredModelField = { key: ModelConfigKey; path: string };

export interface HealthCheckServicePorts {
  useConfigStore: typeof useConfigStore;
  settingsStore: typeof settingsStore;
  projectService: typeof projectService;
  getPathStatusMap: typeof getPathStatusMap;
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
      await this.checkModels();
      await this.checkProjects();

      logger.info('[HealthCheck] Health check completed.');
    } catch (error) {
      logger.error('[HealthCheck] Error during health check:', error);
    }
  };

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
      { key: 'batchModelPath', path: config.batchModelPath },
      { key: 'streamingModelPath', path: config.streamingModelPath },
      { key: 'livePunctuationModelPath', path: config.livePunctuationModelPath },
      { key: 'liveVadModelPath', path: config.liveVadModelPath },
      { key: 'liveSpeakerSegmentationModelPath', path: config.liveSpeakerSegmentationModelPath },
      { key: 'liveSpeakerEmbeddingModelPath', path: config.liveSpeakerEmbeddingModelPath },
      { key: 'liveAlignmentModelPath', path: config.liveAlignmentModelPath },
      { key: 'batchPunctuationModelPath', path: config.batchPunctuationModelPath },
      { key: 'batchVadModelPath', path: config.batchVadModelPath },
      { key: 'batchSpeakerSegmentationModelPath', path: config.batchSpeakerSegmentationModelPath },
      { key: 'batchSpeakerEmbeddingModelPath', path: config.batchSpeakerEmbeddingModelPath },
      { key: 'batchAlignmentModelPath', path: config.batchAlignmentModelPath },
    ] as const;

    const configuredModelFields: ConfiguredModelField[] = modelFields
      .map((model) => ({
        key: model.key,
        path: model.path?.trim() || '',
      }))
      .filter((model) => model.path.length > 0);
    const pathStatusMap = await this.ports.getPathStatusMap(
      configuredModelFields.map((model) => model.path)
    );

    for (const model of configuredModelFields) {
      if (pathStatusMap[model.path]?.kind === 'missing') {
        logger.warn(
          `[HealthCheck] Model path not found: ${model.path}. Clearing field ${model.key}.`
        );
        patch[model.key] = '';
        changed = true;
      }
    }

    if (changed) {
      setConfig(patch);
      const updatedConfig = { ...this.ports.useConfigStore.getState().config, ...patch };
      await this.ports.settingsStore.set(STORE_KEY_CONFIG, updatedConfig);
    }
  };

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
  };
}

export function createHealthCheckService(ports: HealthCheckServicePorts): HealthCheckService {
  return new HealthCheckService(ports);
}

export const healthCheckService = createHealthCheckService({
  useConfigStore,
  settingsStore,
  projectService,
  getPathStatusMap,
});
