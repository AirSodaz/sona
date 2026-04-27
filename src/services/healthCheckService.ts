import { BaseDirectory, exists } from '@tauri-apps/plugin-fs';
import { historyService } from './historyService';
import { useConfigStore } from '../stores/configStore';
import { settingsStore, STORE_KEY_CONFIG } from './storageService';
import { projectService } from './projectService';
import { logger } from '../utils/logger';

const HISTORY_DIR = 'history';

/**
 * Service to perform background health checks on application data.
 * Verifies consistency between database records and files on disk.
 */
export const healthCheckService = {
    /**
     * Runs all health checks. Should be called at app startup.
     */
    async runHealthCheck() {
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
    },

    /**
     * Verifies that history items have at least one valid file (audio or transcript).
     * If both are missing, the record is considered "dead" and removed.
     */
    async checkHistory() {
        const items = await historyService.getAll();
        if (items.length === 0) return;

        const invalidIds: string[] = [];

        for (const item of items) {
            const audioPath = `${HISTORY_DIR}/${item.audioPath}`;
            const transcriptPath = `${HISTORY_DIR}/${item.transcriptPath}`;

            try {
                const [audioExists, transcriptExists] = await Promise.all([
                    exists(audioPath, { baseDir: BaseDirectory.AppLocalData }),
                    exists(transcriptPath, { baseDir: BaseDirectory.AppLocalData })
                ]);

                // If both are missing, the record is invalid
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
                await historyService.deleteRecordings(invalidIds);
            } catch (e) {
                logger.error('[HealthCheck] Failed to remove invalid history records:', e);
            }
        }
    },

    /**
     * Verifies that configured model paths still exist on disk.
     * Clears invalid paths from the configuration to prevent engine errors.
     */
    async checkModels() {
        const config = useConfigStore.getState().config;
        const setConfig = useConfigStore.getState().setConfig;
        const patch: any = {};
        let changed = false;

        const modelFields = [
            { key: 'offlineModelPath', path: config.offlineModelPath },
            { key: 'streamingModelPath', path: config.streamingModelPath },
            { key: 'punctuationModelPath', path: config.punctuationModelPath },
            { key: 'vadModelPath', path: config.vadModelPath }
        ] as const;

        for (const model of modelFields) {
            if (model.path) {
                try {
                    // Model paths are usually absolute paths selected via dialog
                    const isExist = await exists(model.path);
                    if (!isExist) {
                        logger.warn(`[HealthCheck] Model path not found: ${model.path}. Clearing field ${model.key}.`);
                        patch[model.key] = '';
                        changed = true;
                    }
                } catch (e) {
                    // If we can't check it, it might be better to leave it, 
                    // but if it's clearly an error we might want to clear it.
                    // For now, we only clear if we are sure it doesn't exist.
                    const errMsg = String(e);
                    if (errMsg.includes('No such file') || errMsg.includes('not found')) {
                        patch[model.key] = '';
                        changed = true;
                    }
                }
            }
        }

        if (changed) {
            setConfig(patch);
            const updatedConfig = { ...useConfigStore.getState().config, ...patch };
            await settingsStore.set(STORE_KEY_CONFIG, updatedConfig);
            await settingsStore.save();
        }
    },

    /**
     * Triggers project data normalization and migration.
     */
    async checkProjects() {
        try {
            // projectService.getAll() automatically runs normalization and backfills
            await projectService.getAll();
        } catch (e) {
            logger.error('[HealthCheck] Failed to check projects:', e);
        }
    }
};
