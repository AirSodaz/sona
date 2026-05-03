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
import { doesModelPathMatch } from '../utils/modelSelection';

const DEFAULT_SENSEVOICE_INT8_MODEL_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const DEFAULT_SENSEVOICE_FP32_MODEL_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17';
const DEFAULT_SILERO_VAD_MODEL_ID = 'silero-vad';

type DownloadState = {
    progress: number;
    status: string;
    controller: AbortController;
};

const EMPTY_MODEL_CATALOG_SNAPSHOT: ModelCatalogSnapshot = {
    modelsDir: '',
    models: [],
    sections: [],
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
            const rules = modelService.getModelRules(model.id);
            if (rules.requiresVad) {
                document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: 'silero-vad' } }));
            }
            if (rules.requiresPunctuation) {
                document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8' } }));
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
                isSelected = isSelected || doesModelPathMatch(config.streamingModelPath || '', model);
            }
            if (model.modes.includes('offline')) {
                isSelected = isSelected || doesModelPathMatch(config.offlineModelPath || '', model);
            }
            return isSelected;
        }
        if (model.type === 'punctuation') {
            return doesModelPathMatch(config.punctuationModelPath || '', model);
        }
        if (model.type === 'vad') {
            return doesModelPathMatch(config.vadModelPath || '', model);
        }
        if (model.type === 'speaker-segmentation') {
            return doesModelPathMatch(config.speakerSegmentationModelPath || '', model);
        }
        if (model.type === 'speaker-embedding') {
            return doesModelPathMatch(config.speakerEmbeddingModelPath || '', model);
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
            const snapshotById = new Map(snapshot.models.map((model) => [model.id, model]));
            const senseVoiceInt8 = snapshotById.get(DEFAULT_SENSEVOICE_INT8_MODEL_ID);
            const senseVoiceFp32 = snapshotById.get(DEFAULT_SENSEVOICE_FP32_MODEL_ID);
            const sileroVad = snapshotById.get(DEFAULT_SILERO_VAD_MODEL_ID);

            const updates: Partial<typeof config> = {
                punctuationModelPath: '',
                vadBufferSize: 5,
                maxConcurrent: 2,
                enableITN: true,
                speakerSegmentationModelPath: '',
                speakerEmbeddingModelPath: '',
            };

            const fallbackModel = senseVoiceInt8?.isInstalled
                ? senseVoiceInt8
                : senseVoiceFp32?.isInstalled
                    ? senseVoiceFp32
                    : null;

            if (fallbackModel) {
                updates.streamingModelPath = fallbackModel.installPath;
                updates.offlineModelPath = fallbackModel.installPath;
            }

            if (sileroVad?.isInstalled) {
                updates.vadModelPath = sileroVad.installPath;
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
