import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, modelService, ModelInfo, ProgressCallback } from '../services/modelService';
import { open } from '@tauri-apps/plugin-dialog';

const DEFAULT_AI_URLS: Record<string, string> = {
    openai: 'https://api.openai.com/v1',
    anthropic: 'https://api.anthropic.com',
    ollama: 'http://localhost:11434/v1',
    gemini: 'https://generativelanguage.googleapis.com',
    deepseek: 'https://api.deepseek.com',
    kimi: 'https://api.moonshot.cn/v1',
    siliconflow: 'https://api.siliconflow.cn/v1',
};

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
    const { confirm, alert } = useDialogStore();
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'microphone' | 'subtitle' | 'local' | 'models' | 'shortcuts' | 'about' | 'ai_service'>('general');

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
        for (const model of PRESET_MODELS) {
            if (await modelService.isModelInstalled(model.id)) {
                installed.add(model.id);
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
                        setModelPathByType(model.type as any, path);
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

    const changeAiServiceType = (type: string) => {
        const currentType = config.aiServiceType || 'openai';
        const aiServices = config.aiServices || {};

        // Save current settings
        const currentSettings = {
            baseUrl: config.aiBaseUrl || '',
            apiKey: config.aiApiKey || '',
            model: config.aiModel || ''
        };
        const updatedServices = { ...aiServices, [currentType]: currentSettings };

        // Load new settings
        const newSettings = updatedServices[type] || {
            baseUrl: DEFAULT_AI_URLS[type] || '',
            apiKey: '',
            model: ''
        };

        updateConfig({
            aiServiceType: type,
            aiBaseUrl: newSettings.baseUrl,
            aiApiKey: newSettings.apiKey,
            aiModel: newSettings.model,
            aiServices: updatedServices
        });
    };

    function getBrowseTitle(type: 'offline' | 'punctuation' | 'vad' | 'ctc'): string {
        switch (type) {
            case 'offline':
                return t('settings.offline_path_label');
            case 'vad':
                return t('settings.vad_path_label');
            case 'ctc':
                return t('settings.ctc_path_label');
            default:
                return 'Select Punctuation Model Path';
        }
    }

    async function handleBrowse(type: 'offline' | 'punctuation' | 'vad' | 'ctc') {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: getBrowseTitle(type)
            });

            if (selected) {
                const path = Array.isArray(selected) ? selected[0] : selected;
                if (path) {
                    if (type === 'offline') {
                        updateConfig({ offlineModelPath: path });
                    } else if (type === 'vad') {
                        updateConfig({ vadModelPath: path });
                    } else if (type === 'ctc') {
                        updateConfig({ ctcModelPath: path });
                    } else {
                        updateConfig({ punctuationModelPath: path });
                    }
                }
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
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

    function setModelPathByType(type: 'offline' | 'punctuation' | 'vad' | 'ctc', path: string) {
        if (type === 'offline') {
            updateConfig({ offlineModelPath: path });
        } else if (type === 'vad') {
            updateConfig({ vadModelPath: path });
        } else if (type === 'ctc') {
            updateConfig({ ctcModelPath: path });
        } else {
            updateConfig({ punctuationModelPath: path });
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
                console.error('Download failed:', error);
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
                `${reason}\n\nDo you want to download it anyway?`,
                { title: 'Hardware Warning', variant: 'warning' }
            );
            if (!confirmed) return;
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
                    setModelPathByType(model.type as any, path);

                    // If it's an offline model, automatically apply rules and trigger background downloads for dependencies
                    if (model.type === 'offline') {
                        const rules = modelService.getModelRules(model.id);
                        if (rules.requiresVad) {
                            document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: 'silero-vad' } }));
                        }
                        if (rules.requiresPunctuation) {
                            document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8' } }));
                        }
                    }
                }
            }
        );
    }

    async function handleLoad(model: ModelInfo) {
        try {
            const path = await modelService.getModelPath(model.id);
            setModelPathByType(model.type as any, path);
        } catch (error: any) {
            console.error('Load failed:', error);
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
            // Streaming path removed
            if (config.offlineModelPath === deletedPath) {
                updateConfig({ offlineModelPath: '' });
            }
            if (config.punctuationModelPath === deletedPath) {
                updateConfig({ punctuationModelPath: '' });
            }
            if (config.vadModelPath === deletedPath) {
                updateConfig({ vadModelPath: '' });
            }
            if (config.ctcModelPath === deletedPath) {
                updateConfig({ ctcModelPath: '' });
            }
        } catch (error: any) {
            console.error('Delete failed:', error);
            await alert(`Failed to delete model: ${error.message}`, {
                title: 'Error',
                variant: 'error'
            });
        } finally {
            setDeletingId(null);
        }
    }

    function isModelSelected(model: ModelInfo): boolean {
        // Streaming path removed
        if (model.type === 'offline') {
            return (config.offlineModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'punctuation') {
            return (config.punctuationModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'vad') {
            return (config.vadModelPath || '').includes(model.filename || model.id);
        }
        if (model.type === 'ctc') {
            return (config.ctcModelPath || '').includes(model.filename || model.id);
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

        // Default model IDs
        const defaultOfflineModelId = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17';
        const defaultVadModelId = 'silero-vad';

        let offlinePath = '';
        let vadPath = '';

        try {
            offlinePath = await modelService.getModelPath(defaultOfflineModelId);
        } catch (e) {
            console.warn('Failed to resolve default offline model path', e);
        }

        try {
            vadPath = await modelService.getModelPath(defaultVadModelId);
        } catch (e) {
            console.warn('Failed to resolve default VAD model path', e);
        }

        // Apply all defaults at once
        updateConfig({
            offlineModelPath: offlinePath,
            punctuationModelPath: '',
            vadModelPath: vadPath,
            ctcModelPath: '',
            vadBufferSize: 5,
            maxConcurrent: 2,
            enableITN: true,
            enabledITNModels: [],
            itnRulesOrder: [],
        });
    }

    return {
        activeTab,
        setActiveTab,
        config,
        updateConfig,
        changeAiServiceType,

        deletingId,
        downloads,
        installedModels,

        handleBrowse,
        handleDownload,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected,
        restoreDefaultModelSettings
    };
}
