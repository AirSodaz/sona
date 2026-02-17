import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, modelService, ModelInfo, ProgressCallback } from '../services/modelService';
import { open } from '@tauri-apps/plugin-dialog';

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

    const [activeTab, setActiveTab] = useState<'general' | 'local' | 'models' | 'shortcuts' | 'about'>('general');

    useEffect(() => {
        if (_isOpen && initialTab) {
            setActiveTab(initialTab as any);
        }
    }, [initialTab, _isOpen]);

    // We read directly from the config store
    const [enabledITNModels, setEnabledITNModels] = useState<Set<string>>(new Set(config.enabledITNModels || []));
    const [enableITN, setEnableITNState] = useState<boolean>(config.enableITN ?? true);

    // Download state
    type DownloadState = {
        progress: number;
        status: string;
        controller: AbortController;
    };
    const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});

    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());

    // Sync ITN state when config changes externally
    useEffect(() => {
        setEnabledITNModels(new Set(config.enabledITNModels || []));
        setEnableITNState(config.enableITN ?? true);
    }, [config.enabledITNModels, config.enableITN]);

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

    // Helper to update config and persist immediately
    const updateConfig = (updates: Partial<typeof config>) => {
        const newConfig = { ...config, ...updates };
        setConfig(newConfig);
        // Persistence is now handled by useAppInitialization
    };

    // Setters that update store immediately
    const setOfflineModelPath = (path: string) => updateConfig({ offlineModelPath: path });
    const setPunctuationModelPath = (path: string) => updateConfig({ punctuationModelPath: path });
    const setCtcModelPath = (path: string) => updateConfig({ ctcModelPath: path });
    const setVadModelPath = (path: string) => updateConfig({ vadModelPath: path });
    const setVadBufferSize = (size: number) => updateConfig({ vadBufferSize: size });
    const setMaxConcurrent = (size: number) => updateConfig({ maxConcurrent: size });

    const setItnRulesOrder = (action: React.SetStateAction<string[]>) => {
        const newOrder = typeof action === 'function'
            ? (action as (prev: string[]) => string[])(config.itnRulesOrder || ['itn-zh-number'])
            : action;
        updateConfig({ itnRulesOrder: newOrder });
    };

    const handleSetEnabledITNModels = (action: React.SetStateAction<Set<string>>) => {
        const currentSet = new Set(config.enabledITNModels || []);
        const newSet = typeof action === 'function'
            ? (action as (prev: Set<string>) => Set<string>)(currentSet)
            : action;

        setEnabledITNModels(newSet);
        updateConfig({
            enabledITNModels: Array.from(newSet)
        });
    };

    const setEnableITN = (enabled: boolean) => {
        setEnableITNState(enabled);
        updateConfig({ enableITN: enabled });
    };

    const setAppLanguage = (lang: 'auto' | 'en' | 'zh') => {
        updateConfig({ appLanguage: lang });
        if (lang === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(lang);
        }
    };

    const setTheme = (theme: 'auto' | 'light' | 'dark') => updateConfig({ theme: theme });
    const setFont = (font: string) => updateConfig({ font: font as any });


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
                        setOfflineModelPath(path);
                    } else if (type === 'vad') {
                        setVadModelPath(path);
                    } else if (type === 'ctc') {
                        setCtcModelPath(path);
                    } else {
                        setPunctuationModelPath(path);
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
            setOfflineModelPath(path);
        } else if (type === 'vad') {
            setVadModelPath(path);
        } else if (type === 'ctc') {
            setCtcModelPath(path);
        } else {
            setPunctuationModelPath(path);
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
                    setEnabledITNModels(prev => new Set(prev).add(model.id));
                    // Also enable ITN if not enabled? No, keep user choice.
                } else {
                    setModelPathByType(model.type as any, path);
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
                setOfflineModelPath('');
            }
            if (config.punctuationModelPath === deletedPath) {
                setPunctuationModelPath('');
            }
            if (config.vadModelPath === deletedPath) {
                setVadModelPath('');
            }
            if (config.ctcModelPath === deletedPath) {
                setCtcModelPath('');
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

        // Sync local state
        setEnabledITNModels(new Set());
        setEnableITNState(true);
    }

    return {
        activeTab,
        setActiveTab,

        // Return values directly from config store or local state
        appLanguage: config.appLanguage || 'auto',
        setAppLanguage,
        theme: config.theme || 'auto',
        setTheme,
        font: config.font || 'system',
        setFont,

        // streamingModelPath removed
        offlineModelPath: config.offlineModelPath,
        setOfflineModelPath,
        punctuationModelPath: config.punctuationModelPath || '',
        setPunctuationModelPath,
        vadModelPath: config.vadModelPath || '',
        setVadModelPath,
        ctcModelPath: config.ctcModelPath || '',
        setCtcModelPath,

        vadBufferSize: config.vadBufferSize || 5,
        setVadBufferSize,

        maxConcurrent: config.maxConcurrent || 2,
        setMaxConcurrent,

        itnRulesOrder: config.itnRulesOrder || ['itn-zh-number'],
        setItnRulesOrder,

        enabledITNModels,
        setEnabledITNModels: handleSetEnabledITNModels,
        enableITN,
        setEnableITN,

        installedITNModels: installedModels, // Use installedModels instead

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
