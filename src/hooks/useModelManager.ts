import { useState, useEffect, createContext, useContext, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import {
    modelService,
} from '../services/modelService';
import {
    buildModelPathConfigPatch,
    buildModelRemovalConfigPatch,
    buildRestoreDefaultModelConfigPatch,
} from '../services/modelManagerService';
import type {
    ModelCatalogSelectedIds,
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

const EMPTY_MODEL_CATALOG_SELECTED_IDS: ModelCatalogSelectedIds = {
    streaming: null,
    offline: null,
    speakerSegmentation: null,
    speakerEmbedding: null,
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
    const [selectedModelIds, setSelectedModelIds] = useState<ModelCatalogSelectedIds>(EMPTY_MODEL_CATALOG_SELECTED_IDS);
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

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        let cancelled = false;
        const cancelScheduledResolve = scheduleAfterFrame(() => {
            void modelService.resolveModelCatalogSelectedIds({
                streamingModelPath: config.streamingModelPath || '',
                offlineModelPath: config.offlineModelPath || '',
                speakerSegmentationModelPath: config.speakerSegmentationModelPath || '',
                speakerEmbeddingModelPath: config.speakerEmbeddingModelPath || '',
            }).then((selectedIds) => {
                if (!cancelled) {
                    setSelectedModelIds(selectedIds);
                }
            }).catch((error) => {
                logger.warn('[useModelManager] Failed to resolve selected model ids:', error);
                if (!cancelled) {
                    setSelectedModelIds(EMPTY_MODEL_CATALOG_SELECTED_IDS);
                }
            });
        });

        return () => {
            cancelled = true;
            cancelScheduledResolve();
        };
    }, [
        config.offlineModelPath,
        config.speakerEmbeddingModelPath,
        config.speakerSegmentationModelPath,
        config.streamingModelPath,
        isOpen,
    ]);

    const setModelPath = useCallback((model: ModelInfo, path: string) => {
        const updates = buildModelPathConfigPatch(config, model, path);
        if (Object.keys(updates).length > 0) {
            updateConfig(updates);
        }
    }, [config, updateConfig]);

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
            updateConfig(buildModelRemovalConfigPatch(config, deletedPath));
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

    async function restoreDefaultModelSettings() {
        const confirmed = await confirm(t('settings.restore_defaults_confirm'), {
            title: t('settings.restore_defaults'),
            variant: 'warning'
        });
        if (!confirmed) return;

        try {
            const snapshot = await refreshModelCatalogSnapshot();
            const defaults = snapshot.restoreDefaults;
            updateConfig(buildRestoreDefaultModelConfigPatch(config, defaults));
        } catch (e) {
            logger.warn('Failed to restore default model settings', e);
        }
    }

    return {
        deletingId,
        downloads,
        installedModels,
        modelCatalog,
        selectedModelIds,

        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        restoreDefaultModelSettings,
    };
}
