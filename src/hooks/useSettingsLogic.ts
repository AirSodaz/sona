import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, ITN_MODELS, modelService, ModelInfo } from '../services/modelService';
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
export function useSettingsLogic(isOpen: boolean, onClose: () => void) {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const { confirm, alert } = useDialogStore();
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'local' | 'models'>('general');
    const [streamingModelPath, setStreamingModelPath] = useState(config.streamingModelPath);
    const [offlineModelPath, setOfflineModelPath] = useState(config.offlineModelPath);

    const [punctuationModelPath, setPunctuationModelPath] = useState(config.punctuationModelPath || '');
    const [vadModelPath, setVadModelPath] = useState(config.vadModelPath || '');
    const [vadBufferSize, setVadBufferSize] = useState(config.vadBufferSize || 5);
    const [enabledITNModels, setEnabledITNModels] = useState<Set<string>>(new Set(config.enabledITNModels || (config.enableITN ? ['itn-zh-number'] : [])));
    const [itnRulesOrder, setItnRulesOrder] = useState<string[]>(config.itnRulesOrder || ['itn-zh-number']);
    const [appLanguage, setAppLanguage] = useState<'auto' | 'en' | 'zh'>((config.appLanguage as any) || 'auto');

    const [theme, setTheme] = useState<'auto' | 'light' | 'dark'>((config.theme as any) || 'auto');
    const [font, setFont] = useState<string>(config.font || 'system');

    // Download state
    const [downloadingId, setDownloadingId] = useState<string | null>(null);
    const [deletingId, setDeletingId] = useState<string | null>(null);
    const [progress, setProgress] = useState(0);
    const [statusMessage, setStatusMessage] = useState('');
    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
    const [installedITNModels, setInstalledITNModels] = useState<Set<string>>(new Set());
    const [abortController, setAbortController] = useState<AbortController | null>(null);

    // Sync from config when isOpen or config changes
    useEffect(() => {
        setStreamingModelPath(config.streamingModelPath);
        setOfflineModelPath(config.offlineModelPath);
        setPunctuationModelPath(config.punctuationModelPath || '');
        setVadModelPath(config.vadModelPath || '');
        setVadBufferSize(config.vadBufferSize || 5);
        setEnabledITNModels(new Set(config.enabledITNModels || (config.enableITN ? ['itn-zh-number'] : [])));
        setItnRulesOrder(config.itnRulesOrder || ['itn-zh-number']);
        setAppLanguage(config.appLanguage || 'auto');

        setTheme(config.theme || 'auto');
        setFont(config.font || 'system');
    }, [config, isOpen]); // Added isOpen to ensure refresh on open

    const checkInstalledModels = useCallback(async () => {
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
    }, []);

    useEffect(() => {
        checkInstalledModels();
    }, [checkInstalledModels]);

    const handleSave = useCallback(() => {
        const enabledList = Array.from(enabledITNModels);
        setConfig({
            streamingModelPath,
            offlineModelPath,
            punctuationModelPath,
            vadModelPath,
            vadBufferSize,
            enabledITNModels: enabledList,
            itnRulesOrder,
            enableITN: enabledList.length > 0, // Legacy support
            appLanguage,
            theme,
            font: font as any
        });
        localStorage.setItem('sona-config', JSON.stringify({
            streamingModelPath,
            offlineModelPath,
            punctuationModelPath,
            vadModelPath,
            vadBufferSize,
            enabledITNModels: enabledList,
            itnRulesOrder,
            enableITN: enabledList.length > 0,
            appLanguage,
            theme,
            font
        }));

        // Apply language immediately
        if (appLanguage === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(appLanguage);
        }

        onClose();
    }, [
        enabledITNModels,
        streamingModelPath,
        offlineModelPath,
        punctuationModelPath,
        vadModelPath,
        vadBufferSize,
        itnRulesOrder,
        appLanguage,
        theme,
        font,
        setConfig,
        i18n,
        onClose
    ]);

    const getBrowseTitle = useCallback((type: 'streaming' | 'offline' | 'punctuation' | 'vad'): string => {
        switch (type) {
            case 'streaming':
                return t('settings.streaming_path_label');
            case 'offline':
                return t('settings.offline_path_label');
            case 'vad':
                return t('settings.vad_path_label');
            default:
                return 'Select Punctuation Model Path';
        }
    }, [t]);

    const handleBrowse = useCallback(async (type: 'streaming' | 'offline' | 'punctuation' | 'vad') => {
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
                    } else {
                        setPunctuationModelPath(path);
                    }
                }
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
        }
    }, [getBrowseTitle]);

    const handleCancelDownload = useCallback(() => {
        if (abortController) {
            abortController.abort();
            setAbortController(null);
            setStatusMessage('Cancelling...');
        }
    }, [abortController]);

    const setModelPathByType = useCallback((type: 'streaming' | 'offline' | 'punctuation' | 'vad', path: string) => {
        if (type === 'streaming') {
            setStreamingModelPath(path);
        } else if (type === 'offline') {
            setOfflineModelPath(path);
        } else if (type === 'vad') {
            setVadModelPath(path);
        } else {
            setPunctuationModelPath(path);
        }
    }, []);

    const handleDownload = useCallback(async (model: ModelInfo) => {
        if (downloadingId) return;

        // Check hardware compatibility
        const { compatible, reason } = await modelService.checkHardware(model.id);
        if (!compatible) {
            const confirmed = await confirm(
                `${reason}\n\nDo you want to download it anyway?`,
                { title: 'Hardware Warning', variant: 'warning' }
            );
            if (!confirmed) return;
        }

        const controller = new AbortController();
        setAbortController(controller);
        setDownloadingId(model.id);
        setProgress(0);

        try {
            const downloadedPath = await modelService.downloadModel(model.id, (pct, status) => {
                setProgress(pct);
                setStatusMessage(status);
            }, controller.signal);

            setModelPathByType(model.type, downloadedPath);

            await checkInstalledModels();

            // Auto-switch to local tab to show result
            setTimeout(() => {
                setActiveTab('local');
                setDownloadingId(null);
            }, 1000);

        } catch (error: any) {
            if (error.message === 'Download cancelled') {
                console.log('Download cancelled by user');
            } else {
                console.error('Download failed:', error);
            }
            // Reset state
            setDownloadingId(null);
            setAbortController(null);
        }
    }, [downloadingId, confirm, checkInstalledModels, setModelPathByType]);

    const handleDownloadITN = useCallback(async (modelId: string) => {
        if (downloadingId) return;

        const controller = new AbortController();
        setAbortController(controller);
        setDownloadingId(modelId);
        setProgress(0);

        try {
            await modelService.downloadITNModel(modelId, (pct, status) => {
                setProgress(pct);
                setStatusMessage(status);
            }, controller.signal);

            await checkInstalledModels();
            // Automatically enable ITN after download
            setEnabledITNModels(prev => new Set(prev).add(modelId));
            setDownloadingId(null);

        } catch (error: any) {
            if (error.message === 'Download cancelled') {
                console.log('Download cancelled by user');
            } else {
                console.error('Download failed:', error);
            }
            // Reset state
            setDownloadingId(null);
            setAbortController(null);
        }
    }, [downloadingId, checkInstalledModels]);

    const handleLoad = useCallback(async (model: ModelInfo) => {
        try {
            const path = await modelService.getModelPath(model.id);
            setModelPathByType(model.type, path);
        } catch (error: any) {
            console.error('Load failed:', error);
        }
    }, [setModelPathByType]);

    const handleDelete = useCallback(async (model: ModelInfo) => {
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
            if (streamingModelPath === deletedPath) {
                setStreamingModelPath('');
            }
            if (offlineModelPath === deletedPath) {
                setOfflineModelPath('');
            }
            if (punctuationModelPath === deletedPath) {
                setPunctuationModelPath('');
            }
            if (vadModelPath === deletedPath) {
                setVadModelPath('');
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
    }, [
        deletingId,
        confirm,
        t,
        alert,
        checkInstalledModels,
        streamingModelPath,
        offlineModelPath,
        punctuationModelPath,
        vadModelPath
    ]);

    const isModelSelected = useCallback((model: ModelInfo): boolean => {
        if (model.type === 'streaming') {
            return streamingModelPath.includes(model.filename || model.id);
        }
        if (model.type === 'offline') {
            return offlineModelPath.includes(model.filename || model.id);
        }
        if (model.type === 'punctuation') {
            return punctuationModelPath.includes(model.filename || model.id);
        }
        if (model.type === 'vad') {
            return vadModelPath.includes(model.filename || model.id);
        }
        return false;
    }, [streamingModelPath, offlineModelPath, punctuationModelPath, vadModelPath]);

    return {
        activeTab,
        setActiveTab,
        appLanguage,
        setAppLanguage,
        theme,
        setTheme,
        font,
        setFont,

        streamingModelPath,
        setStreamingModelPath,
        offlineModelPath,
        setOfflineModelPath,
        punctuationModelPath,
        setPunctuationModelPath,
        vadModelPath,
        setVadModelPath,

        vadBufferSize,
        setVadBufferSize,

        itnRulesOrder,
        setItnRulesOrder,
        enabledITNModels,
        setEnabledITNModels,
        installedITNModels,

        downloadingId,
        deletingId,
        progress,
        statusMessage,
        installedModels,

        handleSave,
        handleBrowse,
        handleDownload,
        handleDownloadITN,
        handleCancelDownload,
        handleLoad,
        handleDelete,
        isModelSelected
    };
}
