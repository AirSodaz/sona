import { useState, useEffect, createContext, useContext } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, modelService, ModelInfo, ProgressCallback } from '../services/modelService';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';

const DEFAULT_SENSEVOICE_INT8_MODEL_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
const DEFAULT_SENSEVOICE_FP32_MODEL_ID = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17';
const DEFAULT_SILERO_VAD_MODEL_ID = 'silero-vad';

type DownloadState = {
    progress: number;
    status: string;
    controller: AbortController;
};

export type ModelManagerContextType = ReturnType<typeof useModelManager>;

export const ModelManagerContext = createContext<ModelManagerContextType | null>(null);

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
    const [deletingId, setDeletingId] = useState<string | null>(null);

    const updateConfig = (updates: Partial<typeof config>) => {
        setConfig(updates);
    };

    async function checkInstalledModels() {
        const installed = new Set<string>();
        const results = await Promise.all(
            PRESET_MODELS.map(async (model) => {
                const isInstalled = await modelService.isModelInstalled(model.id);
                return { id: model.id, isInstalled };
            })
        );
        for (const result of results) {
            if (result.isInstalled) {
                installed.add(result.id);
            }
        }
        setInstalledModels(installed);
    }

    useEffect(() => {
        if (isOpen) {
            checkInstalledModels();
        }
    }, [isOpen]);

    // Listen for background download requests from components
    useEffect(() => {
        const handleBackgroundDownload = async (event: Event) => {
            const customEvent = event as CustomEvent;
            if (customEvent.detail && customEvent.detail.modelId) {
                const modelId = customEvent.detail.modelId;
                const model = PRESET_MODELS.find(m => m.id === modelId);
                if (model && !downloads[modelId]) {
                    try {
                        const path = await modelService.downloadModel(modelId, undefined);
                        await checkInstalledModels();
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
    }, [downloads]);

    function setModelPath(model: ModelInfo, path: string) {
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
    }

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

            await checkInstalledModels();
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
            const path = await modelService.getModelPath(model.id);
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
            await modelService.deleteModel(model.id);
            await checkInstalledModels();
            const deletedPath = await modelService.getModelPath(model.id);
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
                isSelected = isSelected || (config.streamingModelPath || '').includes(model.filename || model.id);
            }
            if (model.modes.includes('offline')) {
                isSelected = isSelected || (config.offlineModelPath || '').includes(model.filename || model.id);
            }
            return isSelected;
        }
        if (model.type === 'punctuation') {
            return (config.punctuationModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'vad') {
            return (config.vadModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'speaker-segmentation') {
            return (config.speakerSegmentationModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'speaker-embedding') {
            return (config.speakerEmbeddingModelPath || '').includes(model.filename || model.id);
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
            const [hasSenseVoiceInt8, hasSenseVoiceFp32, hasSileroVad] = await Promise.all([
                modelService.isModelInstalled(DEFAULT_SENSEVOICE_INT8_MODEL_ID),
                modelService.isModelInstalled(DEFAULT_SENSEVOICE_FP32_MODEL_ID),
                modelService.isModelInstalled(DEFAULT_SILERO_VAD_MODEL_ID),
            ]);

            const updates: Partial<typeof config> = {
                punctuationModelPath: '',
                vadBufferSize: 5,
                maxConcurrent: 2,
                enableITN: true,
                speakerSegmentationModelPath: '',
                speakerEmbeddingModelPath: '',
            };

            const fallbackModelId = hasSenseVoiceInt8
                ? DEFAULT_SENSEVOICE_INT8_MODEL_ID
                : hasSenseVoiceFp32
                    ? DEFAULT_SENSEVOICE_FP32_MODEL_ID
                    : null;

            if (fallbackModelId) {
                const modelPath = await modelService.getModelPath(fallbackModelId);
                updates.streamingModelPath = modelPath;
                updates.offlineModelPath = modelPath;
            }

            if (hasSileroVad) {
                updates.vadModelPath = await modelService.getModelPath(DEFAULT_SILERO_VAD_MODEL_ID);
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

        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected,
        restoreDefaultModelSettings,
    };
}
