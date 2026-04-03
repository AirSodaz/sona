import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, modelService, ModelInfo, ProgressCallback } from '../services/modelService';
import { getRecommendedOnboardingConfig, resolveRecommendedOnboardingPaths } from '../services/onboardingService';
import { LlmProvider } from '../types/transcript';
import { buildLlmConfigPatch, ensureLlmState, setActiveProvider } from '../services/llmConfig';

/**
 * Custom hook managing the business logic for the Settings dialog.
 *
 * Handles state for tabs, model paths, downloading, and saving configuration.
 *
 * @return An object containing form state and action handlers.
 * @param _isOpen
 * @param _onClose
 * @param initialTab
 */
export function useSettingsLogic(_isOpen: boolean, _onClose: () => void, initialTab?: string) {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const confirm = useDialogStore((state) => state.confirm);
    const showError = useDialogStore((state) => state.showError);
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'microphone' | 'subtitle' | 'local' | 'models' | 'shortcuts' | 'about' | 'llm_service'>('general');

    useEffect(() => {
        if (_isOpen) {
            if (initialTab) {
                setActiveTab(initialTab as any);
            }
        } else {
            setActiveTab('general');
        }
    }, [initialTab, _isOpen]);

    // Download state
    type DownloadState = {
        progress: number;
        status: string;
        controller: AbortController;
    };
    const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});

    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());

    // Sync language change
    useEffect(() => {
        if (config.appLanguage === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(config.appLanguage);
        }
    }, [config.appLanguage, i18n]);

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
        if (_isOpen) {
            checkInstalledModels();
        }
    }, [_isOpen]);

    // Listen for background download requests from components
    useEffect(() => {
        const handleBackgroundDownload = async (event: Event) => {
            const customEvent = event as CustomEvent;
            if (customEvent.detail && customEvent.detail.modelId) {
                const modelId = customEvent.detail.modelId;
                const model = PRESET_MODELS.find(m => m.id === modelId);
                if (model && !downloads[modelId]) {
                    try {
                        // Silent download without updating progress UI
                        const path = await modelService.downloadModel(modelId, undefined);
                        await checkInstalledModels();
                        setModelPath(model, path);
                    } catch (e) {
                        console.error(`Background download failed for ${modelId}:`, e);
                    }
                }
            }
        };

        document.addEventListener('download-background-model', handleBackgroundDownload);
        return () => {
            document.removeEventListener('download-background-model', handleBackgroundDownload);
        };
    }, [downloads]);

    // Helper to update config and persist immediately
    const updateConfig = (updates: Partial<typeof config>) => {
        setConfig(updates);
        // Persistence is now handled by useAppInitialization
    };

    const changeLlmServiceType = (provider: LlmProvider) => {
        const currentLlmState = config.llmSettings ? { llmSettings: config.llmSettings } : ensureLlmState(config as typeof config & Record<string, any>);
        const nextLlmSettings = setActiveProvider(currentLlmState.llmSettings, provider);
        updateConfig(buildLlmConfigPatch(nextLlmSettings));
    };

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
                case 'itn':
                    // Handled elsewhere
                    return;
            }
        }

        if (Object.keys(updates).length > 0) {
            updateConfig(updates);
        }
    }

    /**
     * Executes a download operation with common lifecycle management.
     */
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

        } catch (error: any) {
            if (error.message === 'Download cancelled') {
                console.log('Download cancelled by user');
            } else {
                await showError({
                    code: 'model.download_failed',
                    messageKey: 'errors.model.download_failed',
                    cause: error,
                });
            }
        } finally {
            // Reset state
            setDownloads(prev => {
                const next = { ...prev };
                delete next[modelId];
                return next;
            });
        }
    }

    async function handleDownload(model: ModelInfo) {
        // Check hardware compatibility
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

        // Trigger background downloads for dependencies immediately
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
                if (model.type === 'itn') {
                    // Update enabledITNModels in config
                    const current = new Set(config.enabledITNModels || []);
                    current.add(model.id);
                    updateConfig({ enabledITNModels: Array.from(current) });
                } else {
                    setModelPath(model, path);
                }
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

    // Delete state
    const [deletingId, setDeletingId] = useState<string | null>(null);

    async function handleDelete(model: ModelInfo) {
        if (deletingId) return;

        // Confirm deletion
        const confirmed = await confirm(t('settings.delete_confirm_message', { name: model.name }), {
            title: t('settings.delete_confirm_title'),
            variant: 'warning'
        });

        if (!confirmed) return;

        setDeletingId(model.id);
        try {
            await modelService.deleteModel(model.id);
            await checkInstalledModels();
            // If the deleted model was selected, clear the path
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
        return false;
    }

    /**
     * Restores all model settings to their default values after user confirmation.
     */
    async function restoreDefaultModelSettings() {
        const confirmed = await confirm(t('settings.restore_defaults_confirm'), {
            title: t('settings.restore_defaults'),
            variant: 'warning'
        });
        if (!confirmed) return;

        // Apply all defaults at once
        try {
            const recommendedPaths = await resolveRecommendedOnboardingPaths();
            updateConfig({
                ...getRecommendedOnboardingConfig(recommendedPaths),
                punctuationModelPath: '',
                vadBufferSize: 5,
                maxConcurrent: 2,
                enableITN: true,
                enabledITNModels: [],
                itnRulesOrder: [],
            });
        } catch (e) {
            console.warn('Failed to resolve default onboarding model paths', e);
        }
    }

    return {
        activeTab,
        setActiveTab,
        config,
        updateConfig,
        changeLlmServiceType,

        deletingId,
        downloads,
        installedModels,

        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected,
        restoreDefaultModelSettings
    };
}
