import { useState, useEffect, createContext, useContext, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import {
    modelService,
} from '../services/modelService';
import type {
    ModelCatalogModel,
    ModelCatalogSnapshot,
    ModelInfo,
    ProgressCallback,
} from '../services/modelService';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
type DownloadState = {
    progress: number;
    status: string;
    controller: AbortController;
};

const EMPTY_MODEL_CATALOG_SNAPSHOT: ModelCatalogSnapshot = {
    modelsDir: '',
    models: [],
    sections: [],
    selectionOptions: {
        streaming: [],
        offline: [],
        speakerSegmentation: [],
        speakerEmbedding: [],
    },
    modelPathById: {},
    modelIdByNormalizedPath: {},
    pathMatchTokens: [],
    dependencyRequestsByModelId: {},
    restoreDefaults: {
        punctuationModelPath: '',
        speakerSegmentationModelPath: '',
        speakerEmbeddingModelPath: '',
        enableITN: true,
        vadBufferSize: 5,
        maxConcurrent: 2,
    },
};

export type ModelManagerContextType = ReturnType<typeof useModelManager>;

export const ModelManagerContext = createContext<ModelManagerContextType | null>(null);

function scheduleAfterFrame(callback: () => void): () => void {
    if (typeof requestAnimationFrame === 'function') {
        const frameId = requestAnimationFrame(() => callback());
        return () => cancelAnimationFrame(frameId);
    }

    const timeoutId = window.setTimeout(callback, 0);
    return () => window.clearTimeout(timeoutId);
}

function normalizeModelPath(path: string): string {
    return path.replace(/\\/g, '/').toLowerCase();
}

function resolveModelIdFromSnapshot(
    snapshot: ModelCatalogSnapshot,
    modelPath: string,
    allowedModelIds: string[],
): string {
    if (!modelPath.trim()) {
        return '';
    }

    const allowedIds = new Set(allowedModelIds);
    const normalizedPath = normalizeModelPath(modelPath);
    const exactId = snapshot.modelIdByNormalizedPath[normalizedPath];
    if (exactId && allowedIds.has(exactId)) {
        return exactId;
    }

    const tokenMatch = snapshot.pathMatchTokens.find((token) => (
        allowedIds.has(token.id)
        && token.token.length > 0
        && normalizedPath.includes(token.token)
    ));
    return tokenMatch?.id ?? '';
}

export function useModelManagerContext() {
    const context = useContext(ModelManagerContext);
    if (!context) {
        throw new Error('useModelManagerContext must be used within a ModelManagerContext.Provider');
    }
    return context;
}

/**
 * Hook encapsulating all model lifecycle operations:
 * installation checks, downloads, deletion, and path management.
 */
export function useModelManager(isOpen: boolean) {
    const config = useConfigStore((state) => state.config);
    const setConfig = useConfigStore((state) => state.setConfig);
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const { t } = useTranslation();

    const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});
    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
    const [modelCatalog, setModelCatalog] = useState<ModelCatalogSnapshot>(EMPTY_MODEL_CATALOG_SNAPSHOT);
    const [deletingId, setDeletingId] = useState<string | null>(null);

    const updateConfig = useCallback((updates: Partial<typeof config>) => {
        setConfig(updates);
    }, [setConfig]);

    const applyModelCatalogSnapshot = useCallback((snapshot: ModelCatalogSnapshot) => {
        setModelCatalog(snapshot);
        setInstalledModels(new Set(
            snapshot.models
                .filter((model) => model.isInstalled)
                .map((model) => model.id)
        ));
    }, []);

    const refreshModelCatalogSnapshot = useCallback(async () => {
        const snapshot = await modelService.getModelCatalogSnapshot();
        applyModelCatalogSnapshot(snapshot);
        return snapshot;
    }, [applyModelCatalogSnapshot]);

    const getCatalogModel = useCallback((modelId: string): ModelCatalogModel | undefined => {
        return modelCatalog.models.find((model) => model.id === modelId);
    }, [modelCatalog.models]);

    const getModelInstallPath = useCallback(async (model: ModelInfo): Promise<string> => {
        const catalogModel = getCatalogModel(model.id);
        if (catalogModel?.installPath) {
            return catalogModel.installPath;
        }
        return await modelService.getModelPath(model.id);
    }, [getCatalogModel]);

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        return scheduleAfterFrame(() => {
            void refreshModelCatalogSnapshot();
        });
    }, [isOpen, refreshModelCatalogSnapshot]);

    const setModelPath = useCallback((model: ModelInfo, path: string) => {
        const updates: Partial<typeof config> = {};

        if (model.modes && model.modes.length > 0) {
            if (model.modes.includes('streaming')) {
                updates.streamingModelPath = path;
            }
            if (model.modes.includes('offline')) {
                updates.offlineModelPath = path;
            }
        } else {
            switch (model.type) {
                case 'vad':
                    updates.vadModelPath = path;
                    break;
                case 'punctuation':
                    updates.punctuationModelPath = path;
                    break;
                case 'speaker-segmentation':
                    updates.speakerSegmentationModelPath = path;
                    break;
                case 'speaker-embedding':
                    updates.speakerEmbeddingModelPath = path;
                    break;
                case 'itn':
                    return;
            }
        }

        if (Object.keys(updates).length > 0) {
            updateConfig(updates);
        }
    }, [updateConfig]);

    // Listen for background download requests from components
    useEffect(() => {
        const handleBackgroundDownload = async (event: Event) => {
            const customEvent = event as CustomEvent;
            if (customEvent.detail && customEvent.detail.modelId) {
                const modelId = customEvent.detail.modelId;
                const model = getCatalogModel(modelId);
                if (model && !downloads[modelId]) {
                    try {
                        const path = await modelService.downloadModel(modelId, undefined);
                        await refreshModelCatalogSnapshot();
                        setModelPath(model, path);
                    } catch (e) {
                        logger.error(`Background download failed for ${modelId}:`, e);
                    }
                }
            }
        };

        document.addEventListener('download-background-model', handleBackgroundDownload);
        return () => {
            document.removeEventListener('download-background-model', handleBackgroundDownload);
        };
    }, [downloads, getCatalogModel, refreshModelCatalogSnapshot, setModelPath]);

    async function executeDownload(
        modelId: string,
        downloadFn: (id: string, callback: ProgressCallback, signal: AbortSignal) => Promise<string>,
        onSuccess: (path: string) => void | Promise<void>
    ) {
        if (downloads[modelId]) return;

        const controller = new AbortController();
        setDownloads(prev => ({
            ...prev,
            [modelId]: { progress: 0, status: '', controller }
        }));

        try {
            const downloadedPath = await downloadFn(modelId, (pct, status) => {
                setDownloads(prev => {
                    if (!prev[modelId]) return prev;
                    return {
                        ...prev,
                        [modelId]: { ...prev[modelId], progress: pct, status: status }
                    }
                });
            }, controller.signal);

            await refreshModelCatalogSnapshot();
            await onSuccess(downloadedPath);

        } catch (error) {
            if (extractErrorMessage(error) === 'Download cancelled') {
                logger.info('Download cancelled by user');
            } else {
                await showError({
                    code: 'model.download_failed',
                    messageKey: 'errors.model.download_failed',
                    cause: error,
                });
            }
        } finally {
            setDownloads(prev => {
                const next = { ...prev };
                delete next[modelId];
                return next;
            });
        }
    }

    function handleCancelDownload(modelId: string) {
        const download = downloads[modelId];
        if (download) {
            download.controller.abort();
            setDownloads(prev => {
                const next = { ...prev };
                delete next[modelId];
                return next;
            });
        }
    }

    async function handleDownload(model: ModelInfo) {
        const { compatible, reason } = await modelService.checkHardware(model.id);
        if (!compatible) {
            const confirmed = await confirm(
                t('settings.hardware_warning_confirm', { reason }),
                {
                    title: t('settings.hardware_warning_title'),
                    variant: 'warning'
                }
            );
            if (!confirmed) return;
        }

        if (model.modes && model.modes.length > 0) {
            const dependencyUpdates: Partial<typeof config> = {};
            const dependencies = modelCatalog.dependencyRequestsByModelId[model.id] ?? [];
            for (const dependency of dependencies) {
                if (config[dependency.configKey]) {
                    continue;
                }
                if (dependency.isInstalled) {
                    dependencyUpdates[dependency.configKey] = dependency.installPath;
                } else {
                    document.dispatchEvent(new CustomEvent('download-background-model', {
                        detail: { modelId: dependency.modelId },
                    }));
                }
            }
            if (Object.keys(dependencyUpdates).length > 0) {
                updateConfig(dependencyUpdates);
            }
        }

        await executeDownload(
            model.id,
            (id, cb, sig) => modelService.downloadModel(id, cb, sig),
            (path) => {
                setModelPath(model, path);
            }
        );
    }

    async function handleLoad(model: ModelInfo) {
        try {
            const path = await getModelInstallPath(model);
            setModelPath(model, path);
        } catch (error) {
            await showError({
                code: 'model.load_failed',
                messageKey: 'errors.model.load_failed',
                messageParams: { name: model.name },
                cause: error,
            });
        }
    }

    async function handleDelete(model: ModelInfo) {
        if (deletingId) return;

        const confirmed = await confirm(t('settings.delete_confirm_message', { name: model.name }), {
            title: t('settings.delete_confirm_title'),
            variant: 'warning'
        });

        if (!confirmed) return;

        setDeletingId(model.id);
        try {
            const deletedPath = await getModelInstallPath(model);
            await modelService.deleteModel(model.id);
            await refreshModelCatalogSnapshot();
            if (config.streamingModelPath === deletedPath) {
                updateConfig({ streamingModelPath: '' });
            }
            if (config.offlineModelPath === deletedPath) {
                updateConfig({ offlineModelPath: '' });
            }
            if (config.punctuationModelPath === deletedPath) {
                updateConfig({ punctuationModelPath: '' });
            }
            if (config.vadModelPath === deletedPath) {
                updateConfig({ vadModelPath: '' });
            }
            if (config.speakerSegmentationModelPath === deletedPath) {
                updateConfig({ speakerSegmentationModelPath: '' });
            }
            if (config.speakerEmbeddingModelPath === deletedPath) {
                updateConfig({ speakerEmbeddingModelPath: '' });
            }
        } catch (error) {
            await showError({
                code: 'model.delete_failed',
                messageKey: 'errors.model.delete_failed',
                messageParams: { name: model.name },
                cause: error,
            });
        } finally {
            setDeletingId(null);
        }
    }

    function isModelSelected(model: ModelInfo): boolean {
        if (model.modes && model.modes.length > 0) {
            let isSelected = false;
            if (model.modes.includes('streaming')) {
                isSelected = isSelected || resolveModelIdFromSnapshot(
                    modelCatalog,
                    config.streamingModelPath || '',
                    modelCatalog.selectionOptions.streaming.map((option) => option.id),
                ) === model.id;
            }
            if (model.modes.includes('offline')) {
                isSelected = isSelected || resolveModelIdFromSnapshot(
                    modelCatalog,
                    config.offlineModelPath || '',
                    modelCatalog.selectionOptions.offline.map((option) => option.id),
                ) === model.id;
            }
            return isSelected;
        }
        if (model.type === 'punctuation') {
            return resolveModelIdFromSnapshot(
                modelCatalog,
                config.punctuationModelPath || '',
                modelCatalog.models.filter((item) => item.type === 'punctuation').map((item) => item.id),
            ) === model.id;
        }
        if (model.type === 'vad') {
            return resolveModelIdFromSnapshot(
                modelCatalog,
                config.vadModelPath || '',
                modelCatalog.models.filter((item) => item.type === 'vad').map((item) => item.id),
            ) === model.id;
        }
        if (model.type === 'speaker-segmentation') {
            return resolveModelIdFromSnapshot(
                modelCatalog,
                config.speakerSegmentationModelPath || '',
                modelCatalog.selectionOptions.speakerSegmentation.map((option) => option.id),
            ) === model.id;
        }
        if (model.type === 'speaker-embedding') {
            return resolveModelIdFromSnapshot(
                modelCatalog,
                config.speakerEmbeddingModelPath || '',
                modelCatalog.selectionOptions.speakerEmbedding.map((option) => option.id),
            ) === model.id;
        }
        return false;
    }

    async function restoreDefaultModelSettings() {
        const confirmed = await confirm(t('settings.restore_defaults_confirm'), {
            title: t('settings.restore_defaults'),
            variant: 'warning'
        });
        if (!confirmed) return;

        try {
            const snapshot = await refreshModelCatalogSnapshot();
            const defaults = snapshot.restoreDefaults;

            const updates: Partial<typeof config> = {
                punctuationModelPath: defaults.punctuationModelPath ?? '',
                vadBufferSize: Number.isFinite(defaults.vadBufferSize) ? defaults.vadBufferSize : 5,
                maxConcurrent: Number.isFinite(defaults.maxConcurrent) ? defaults.maxConcurrent : 2,
                enableITN: defaults.enableITN,
                speakerSegmentationModelPath: defaults.speakerSegmentationModelPath ?? '',
                speakerEmbeddingModelPath: defaults.speakerEmbeddingModelPath ?? '',
            };

            if (defaults.streamingModelPath !== undefined) {
                updates.streamingModelPath = defaults.streamingModelPath;
            }
            if (defaults.offlineModelPath !== undefined) {
                updates.offlineModelPath = defaults.offlineModelPath;
            }
            if (defaults.vadModelPath !== undefined) {
                updates.vadModelPath = defaults.vadModelPath;
            }

            updateConfig(updates);
        } catch (e) {
            logger.warn('Failed to restore default model settings', e);
        }
    }

    return {
        deletingId,
        downloads,
        installedModels,
        modelCatalog,

        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected,
        restoreDefaultModelSettings,
    };
}
