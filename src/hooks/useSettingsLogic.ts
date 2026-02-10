import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, ITN_MODELS, modelService, ModelInfo, ProgressCallback } from '../services/modelService';
import { open } from '@tauri-apps/plugin-dialog';

/**
 * Custom hook managing the business logic for the Settings dialog.
 *
 * Handles state for tabs, model paths, downloading, and saving configuration.
 *
 * @param isOpen Whether the settings dialog is open.
 * @param onClose Callback to close the dialog.
 * @return An object containing form state and action handlers.
 */
export function useSettingsLogic(_isOpen: boolean, _onClose: () => void) {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const { confirm, alert } = useDialogStore();
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'local' | 'models' | 'shortcuts'>('general');

    // We read directly from the config store
    // Local state for ITN set is derived from config for easier UI handling, but we will sync it back immediately on change.
    const [enabledITNModels, setEnabledITNModels] = useState<Set<string>>(new Set(config.enabledITNModels || (config.enableITN ? ['itn-zh-number'] : [])));

    // Download state
    type DownloadState = {
        progress: number;
        status: string;
        controller: AbortController;
    };
    const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});

    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
    const [installedITNModels, setInstalledITNModels] = useState<Set<string>>(new Set());

    // Sync ITN state when config changes externally (though we update config immediately now)
    useEffect(() => {
        setEnabledITNModels(new Set(config.enabledITNModels || (config.enableITN ? ['itn-zh-number'] : [])));
    }, [config.enabledITNModels, config.enableITN]);

    async function checkInstalledModels() {
        const installed = new Set<string>();
        for (const model of PRESET_MODELS) {
            if (await modelService.isModelInstalled(model.id)) {
                installed.add(model.id);
            }
        }
        setInstalledModels(installed);

        const installedITN = new Set<string>();
        for (const model of ITN_MODELS) {
            if (await modelService.isITNModelInstalled(model.id)) {
                installedITN.add(model.id);
            }
        }
        setInstalledITNModels(installedITN);
    }

    useEffect(() => {
        checkInstalledModels();
    }, []);

    // Helper to update config and persist immediately
    const updateConfig = (updates: Partial<typeof config>) => {
        const newConfig = { ...config, ...updates };
        setConfig(newConfig);

        // Persist to localStorage
        localStorage.setItem('sona-config', JSON.stringify({
            streamingModelPath: newConfig.streamingModelPath,
            offlineModelPath: newConfig.offlineModelPath,
            punctuationModelPath: newConfig.punctuationModelPath,
            vadModelPath: newConfig.vadModelPath,
            ctcModelPath: newConfig.ctcModelPath,
            vadBufferSize: newConfig.vadBufferSize,
            enabledITNModels: newConfig.enabledITNModels,
            itnRulesOrder: newConfig.itnRulesOrder,
            enableITN: (newConfig.enabledITNModels?.length ?? 0) > 0,
            appLanguage: newConfig.appLanguage,
            theme: newConfig.theme,
            font: newConfig.font as any
        }));
    };

    // Setters that update store immediately
    const setStreamingModelPath = (path: string) => updateConfig({ streamingModelPath: path });
    const setOfflineModelPath = (path: string) => updateConfig({ offlineModelPath: path });
    const setPunctuationModelPath = (path: string) => updateConfig({ punctuationModelPath: path });
    const setVadModelPath = (path: string) => updateConfig({ vadModelPath: path });
    const setCtcModelPath = (path: string) => updateConfig({ ctcModelPath: path });
    const setVadBufferSize = (size: number) => updateConfig({ vadBufferSize: size });

    const setItnRulesOrder = (action: React.SetStateAction<string[]>) => {
        const newOrder = typeof action === 'function'
            ? (action as (prev: string[]) => string[])(config.itnRulesOrder || ['itn-zh-number'])
            : action;
        updateConfig({ itnRulesOrder: newOrder });
    };

    const handleSetEnabledITNModels = (action: React.SetStateAction<Set<string>>) => {
        const currentSet = new Set(config.enabledITNModels || (config.enableITN ? ['itn-zh-number'] : []));
        const newSet = typeof action === 'function'
            ? (action as (prev: Set<string>) => Set<string>)(currentSet)
            : action;

        setEnabledITNModels(newSet);
        updateConfig({
            enabledITNModels: Array.from(newSet),
            enableITN: newSet.size > 0
        });
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


    function getBrowseTitle(type: 'streaming' | 'offline' | 'punctuation' | 'vad' | 'ctc'): string {
        switch (type) {
            case 'streaming':
                return t('settings.streaming_path_label');
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

    async function handleBrowse(type: 'streaming' | 'offline' | 'punctuation' | 'vad' | 'ctc') {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: getBrowseTitle(type)
            });

            if (selected) {
                const path = Array.isArray(selected) ? selected[0] : selected;
                if (path) {
                    if (type === 'streaming') {
                        setStreamingModelPath(path);
                    } else if (type === 'offline') {
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

    function setModelPathByType(type: 'streaming' | 'offline' | 'punctuation' | 'vad' | 'ctc', path: string) {
        if (type === 'streaming') {
            setStreamingModelPath(path);
        } else if (type === 'offline') {
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
            (path) => setModelPathByType(model.type, path)
        );
    }

    async function handleDownloadITN(modelId: string) {
        await executeDownload(
            modelId,
            (id, cb, sig) => modelService.downloadITNModel(id, cb, sig),
            () => setEnabledITNModels(prev => new Set(prev).add(modelId))
        );
    }

    async function handleLoad(model: ModelInfo) {
        try {
            const path = await modelService.getModelPath(model.id);
            setModelPathByType(model.type, path);
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
            if (config.streamingModelPath === deletedPath) {
                setStreamingModelPath('');
            }
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
        if (model.type === 'streaming') {
            return (config.streamingModelPath || '').includes(model.filename || model.id);
        }
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

        streamingModelPath: config.streamingModelPath,
        setStreamingModelPath,
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

        itnRulesOrder: config.itnRulesOrder || ['itn-zh-number'],
        setItnRulesOrder,

        enabledITNModels,
        setEnabledITNModels: handleSetEnabledITNModels,
        installedITNModels,

        deletingId,
        downloads,
        installedModels,

        handleBrowse,
        handleDownload,
        handleDownloadITN,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected
    };
}
