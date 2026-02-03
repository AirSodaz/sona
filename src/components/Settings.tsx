import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { PRESET_MODELS, ITN_MODELS, modelService, ModelInfo } from '../services/modelService';
import { open } from '@tauri-apps/plugin-dialog';
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent
} from '@dnd-kit/core';
import {
    arrayMove,
    SortableContext,
    sortableKeyboardCoordinates,
    verticalListSortingStrategy,
    useSortable
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { ModelCard } from './ModelCard';
import {
    FolderIcon,
    GeneralIcon,
    ModelIcon,
    LocalIcon,
    DragHandleIcon,
    XIcon,
    DownloadIcon
} from './Icons';

function SortableItem(props: any) {
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
    } = useSortable({ id: props.id });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
        touchAction: 'none', // Prevent scrolling on touch while dragging
        ...props.style
    };

    return (
        <div ref={setNodeRef} style={style}>
            <div style={{ display: 'flex', alignItems: 'center', width: '100%' }}>
                <div {...attributes} {...listeners} style={{ display: 'flex', alignItems: 'center', paddingRight: 8, cursor: 'grab' }}>
                    <DragHandleIcon />
                </div>
                <div style={{ flex: 1 }}>
                    {props.children}
                </div>
            </div>
        </div>
    );
}

interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
}

export const Settings: React.FC<SettingsProps> = ({ isOpen, onClose }) => {
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
    const [appLanguage, setAppLanguage] = useState(config.appLanguage || 'auto');

    const [theme, setTheme] = useState(config.theme || 'auto');
    const [font, setFont] = useState(config.font || 'system');

    // Download state
    const [downloadingId, setDownloadingId] = useState<string | null>(null);
    const [deletingId, setDeletingId] = useState<string | null>(null);
    const [progress, setProgress] = useState(0);
    const [statusMessage, setStatusMessage] = useState('');
    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
    const [installedITNModels, setInstalledITNModels] = useState<Set<string>>(new Set());
    const [abortController, setAbortController] = useState<AbortController | null>(null);
    const modalRef = useRef<HTMLDivElement>(null);

    const sensors = useSensors(
        useSensor(PointerSensor),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    // Focus management
    useEffect(() => {
        if (isOpen) {
            const previousFocus = document.activeElement as HTMLElement;
            // Wait for render
            requestAnimationFrame(() => {
                modalRef.current?.focus();
            });

            const handleKeyDown = (e: KeyboardEvent) => {
                if (e.key === 'Escape') {
                    // Only close if no other dialog is open (GlobalDialog)
                    if (useDialogStore.getState().isOpen) return;
                    onClose();
                }
            };
            window.addEventListener('keydown', handleKeyDown);

            return () => {
                window.removeEventListener('keydown', handleKeyDown);
                previousFocus?.focus();
            };
        }
    }, [isOpen, onClose]);

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;

        if (over && active.id !== over.id) {
            setItnRulesOrder((items) => {
                const oldIndex = items.indexOf(String(active.id));
                const newIndex = items.indexOf(String(over.id));
                return arrayMove(items, oldIndex, newIndex);
            });
        }
    };

    const checkInstalledModels = async () => {
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
    };

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
    }, [config.streamingModelPath, config.offlineModelPath, config.punctuationModelPath, config.vadModelPath, config.vadBufferSize, config.enabledITNModels, config.itnRulesOrder, config.appLanguage, config.theme, config.font]);

    useEffect(() => {
        checkInstalledModels();
    }, []);

    const handleSave = () => {
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
            font
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
    };

    const handleBrowse = async (type: 'streaming' | 'offline' | 'punctuation' | 'vad') => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: type === 'streaming' ? t('settings.streaming_path_label') :
                    type === 'offline' ? t('settings.offline_path_label') :
                        type === 'vad' ? t('settings.vad_path_label') : 'Select Punctuation Model Path'
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
    };

    const handleCancelDownload = () => {
        if (abortController) {
            abortController.abort();
            setAbortController(null);
            setStatusMessage('Cancelling...');
        }
    };

    const handleDownload = async (model: ModelInfo) => {
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
    };

    const handleDownloadITN = async (modelId: string) => {
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
    };

    const handleLoad = async (model: ModelInfo) => {
        try {
            const path = await modelService.getModelPath(model.id);
            setModelPathByType(model.type, path);
        } catch (error: any) {
            console.error('Load failed:', error);
        }
    };

    const setModelPathByType = (type: 'streaming' | 'offline' | 'punctuation' | 'vad', path: string) => {
        if (type === 'streaming') {
            setStreamingModelPath(path);
        } else if (type === 'offline') {
            setOfflineModelPath(path);
        } else if (type === 'vad') {
            setVadModelPath(path);
        } else {
            setPunctuationModelPath(path);
        }
    };

    const handleDelete = async (model: ModelInfo) => {
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
    };

    const isModelSelected = (model: ModelInfo): boolean => {
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
    };

    if (!isOpen) return null;

    const renderTabButton = (id: 'general' | 'models' | 'local', label: string, Icon: React.FC) => (
        <button
            className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
            onClick={() => setActiveTab(id)}
            role="tab"
            aria-selected={activeTab === id}
            aria-controls={`settings-panel-${id}`}
            id={`settings-tab-${id}`}
        >
            <Icon />
            {label}
        </button>
    );

    const renderModelSection = (title: string, type: 'streaming' | 'offline' | 'punctuation' | 'vad') => (
        <>
            <div className="settings-section-subtitle" style={{ marginTop: 30, marginBottom: 10, fontWeight: 'bold' }}>{title}</div>
            {PRESET_MODELS.filter(m => m.type === type).map(model => (
                <ModelCard
                    key={model.id}
                    model={model}
                    isInstalled={installedModels.has(model.id)}
                    isSelected={isModelSelected(model)}
                    downloadingId={downloadingId}
                    deletingId={deletingId}
                    progress={progress}
                    statusMessage={statusMessage}
                    onLoad={handleLoad}
                    onDelete={handleDelete}
                    onDownload={handleDownload}
                    onCancelDownload={handleCancelDownload}
                />
            ))}
        </>
    );

    return (
        <div className="settings-overlay" onClick={onClose}>
            <div
                ref={modalRef}
                className="settings-modal"
                onClick={(e) => e.stopPropagation()}
                role="dialog"
                aria-modal="true"
                aria-labelledby="settings-title"
                tabIndex={-1}
                style={{ outline: 'none' }}
            >
                {/* Sidebar */}
                <div className="settings-sidebar">
                    <div className="settings-sidebar-header">
                        <h2 id="settings-title">{t('settings.title')}</h2>
                    </div>

                    <div className="settings-tabs-container" role="tablist" aria-orientation="vertical">
                        {renderTabButton('general', t('settings.general'), GeneralIcon)}
                        {renderTabButton('models', t('settings.model_hub'), ModelIcon)}
                        {renderTabButton('local', t('settings.local_path'), LocalIcon)}
                    </div>
                </div>

                {/* Main Content */}
                <div className="settings-content">
                    {/* Header with close button */}
                    <div className="settings-close-btn-container">
                        <button
                            className="btn btn-icon"
                            onClick={onClose}
                            aria-label={t('common.close')}
                            data-tooltip={t('common.close')}
                            data-tooltip-pos="bottom-left"
                        >
                            <XIcon />
                        </button>
                    </div>

                    {/* Scrollable Content Area */}
                    <div className="settings-content-scroll">
                        <div className="settings-section-header">
                            <h3 className="settings-section-title">
                                {activeTab === 'general' && t('settings.general')}
                                {activeTab === 'models' && t('settings.model_hub')}
                                {activeTab === 'local' && t('settings.local_path')}
                            </h3>
                            <div className="settings-divider" />
                        </div>

                        {activeTab === 'general' && (
                            <div
                                className="settings-group"
                                role="tabpanel"
                                id="settings-panel-general"
                                aria-labelledby="settings-tab-general"
                                tabIndex={0}
                            >
                                <div className="settings-item">
                                    <label htmlFor="settings-language" className="settings-label">{t('settings.language')}</label>
                                    <div style={{ maxWidth: 300 }}>
                                        <select
                                            id="settings-language"
                                            className="settings-input"
                                            value={appLanguage}
                                            onChange={(e) => setAppLanguage(e.target.value as 'auto' | 'en' | 'zh')}
                                            style={{ width: '100%' }}
                                        >
                                            <option value="auto">{t('common.auto')}</option>
                                            <option value="en">English</option>
                                            <option value="zh">中文</option>
                                        </select>
                                    </div>
                                    <div className="settings-hint">
                                        {t('settings.language_hint', { defaultValue: '' })}
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                                    <label htmlFor="settings-theme" className="settings-label">{t('settings.theme', { defaultValue: 'Theme' })}</label>
                                    <div style={{ maxWidth: 300 }}>
                                        <select
                                            id="settings-theme"
                                            className="settings-input"
                                            value={theme}
                                            onChange={(e) => setTheme(e.target.value as 'auto' | 'light' | 'dark')}
                                            style={{ width: '100%' }}
                                        >
                                            <option value="auto">{t('common.auto')}</option>
                                            <option value="light">{t('settings.theme_light', { defaultValue: 'Light' })}</option>
                                            <option value="dark">{t('settings.theme_dark', { defaultValue: 'Dark' })}</option>
                                        </select>
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                                    <label htmlFor="settings-font" className="settings-label">{t('settings.font', { defaultValue: 'Font' })}</label>
                                    <div style={{ maxWidth: 300 }}>
                                        <select
                                            id="settings-font"
                                            className="settings-input"
                                            value={font as string}
                                            onChange={(e) => setFont(e.target.value as any)}
                                            style={{ width: '100%', fontFamily: font === 'mono' ? 'monospace' : font === 'serif' ? 'serif' : 'inherit' }}
                                        >
                                            <option value="system">{t('settings.font_system', { defaultValue: 'System Default' })}</option>
                                            <option value="serif">Serif (Merriweather)</option>
                                            <option value="sans">Sans Serif (Inter)</option>
                                            <option value="mono">Monospace (JetBrains Mono)</option>
                                            <option value="arial">Arial</option>
                                            <option value="georgia">Georgia</option>
                                        </select>
                                    </div>
                                </div>
                            </div>
                        )}

                        {activeTab === 'models' && (
                            <div
                                className="model-list"
                                role="tabpanel"
                                id="settings-panel-models"
                                aria-labelledby="settings-tab-models"
                                tabIndex={0}
                            >
                                {renderModelSection(t('settings.streaming_models'), 'streaming')}
                                {renderModelSection(t('settings.offline_models'), 'offline')}
                                {renderModelSection(t('settings.punctuation_models'), 'punctuation')}
                                {renderModelSection(t('settings.vad_models'), 'vad')}

                                <div className="settings-item" style={{ marginTop: 24, paddingTop: 24, borderTop: '1px solid var(--color-border)' }}>
                                    <label htmlFor="settings-vad-buffer" className="settings-label">{t('settings.vad_buffer_size')}</label>
                                    <div style={{ maxWidth: 300 }}>
                                        <input
                                            id="settings-vad-buffer"
                                            type="number"
                                            className="settings-input"
                                            value={vadBufferSize}
                                            onChange={(e) => setVadBufferSize(Number(e.target.value))}
                                            min={0}
                                            max={30}
                                            step={0.5}
                                            style={{ width: '100%' }}
                                        />
                                    </div>
                                    <div className="settings-hint">
                                        {t('settings.vad_buffer_hint')}
                                    </div>
                                </div>
                            </div>
                        )}

                        {activeTab === 'local' && (
                            <div
                                className="settings-group"
                                role="tabpanel"
                                id="settings-panel-local"
                                aria-labelledby="settings-tab-local"
                                tabIndex={0}
                            >
                                <div className="settings-item">
                                    <label htmlFor="settings-streaming-path" className="settings-label">{t('settings.streaming_path_label', { defaultValue: 'Streaming Model Path' })}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
                                            id="settings-streaming-path"
                                            type="text"
                                            title={streamingModelPath}
                                            className="settings-input"
                                            value={streamingModelPath}
                                            onChange={(e) => setStreamingModelPath(e.target.value)}
                                            placeholder={t('settings.path_placeholder')}
                                            style={{ flex: 1 }}
                                        />
                                        <button
                                            className="btn btn-secondary"
                                            onClick={() => handleBrowse('streaming')}
                                            aria-label={t('settings.browse')}
                                        >
                                            <FolderIcon />
                                        </button>
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 16 }}>
                                    <label htmlFor="settings-offline-path" className="settings-label">{t('settings.offline_path_label', { defaultValue: 'Offline Model Path' })}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
                                            id="settings-offline-path"
                                            type="text"
                                            title={offlineModelPath}
                                            className="settings-input"
                                            value={offlineModelPath}
                                            onChange={(e) => setOfflineModelPath(e.target.value)}
                                            placeholder={t('settings.path_placeholder')}
                                            style={{ flex: 1 }}
                                        />
                                        <button
                                            className="btn btn-secondary"
                                            onClick={() => handleBrowse('offline')}
                                            aria-label={t('settings.browse')}
                                        >
                                            <FolderIcon />
                                        </button>
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 16 }}>
                                    <label htmlFor="settings-punctuation-path" className="settings-label">{t('settings.punctuation_path_label')}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
                                            id="settings-punctuation-path"
                                            type="text"
                                            title={punctuationModelPath}
                                            className="settings-input"
                                            value={punctuationModelPath}
                                            onChange={(e) => setPunctuationModelPath(e.target.value)}
                                            placeholder={t('settings.path_placeholder')}
                                            style={{ flex: 1 }}
                                        />
                                        <button
                                            className="btn btn-secondary"
                                            onClick={() => handleBrowse('punctuation')}
                                            aria-label={t('settings.browse')}
                                        >
                                            <FolderIcon />
                                        </button>
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 16 }}>
                                    <label htmlFor="settings-vad-path" className="settings-label">{t('settings.vad_path_label', { defaultValue: 'VAD Model Path' })}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
                                            id="settings-vad-path"
                                            type="text"
                                            title={vadModelPath}
                                            className="settings-input"
                                            value={vadModelPath}
                                            onChange={(e) => setVadModelPath(e.target.value)}
                                            placeholder={t('settings.path_placeholder')}
                                            style={{ flex: 1 }}
                                        />
                                        <button
                                            className="btn btn-secondary"
                                            onClick={() => handleBrowse('vad')}
                                            aria-label={t('settings.browse')}
                                        >
                                            <FolderIcon />
                                        </button>
                                    </div>
                                </div>

                                <div className="settings-item" style={{ marginTop: 24, borderTop: '1px solid var(--color-border)', paddingTop: 24 }}>
                                    <div style={{ marginBottom: 12 }}>
                                        <div style={{ fontWeight: 500, marginBottom: 4 }}>{t('settings.itn_title')}</div>
                                        <div style={{ fontSize: '0.875rem', color: 'var(--color-text-muted)' }}>
                                            {t('settings.itn_desc')}
                                        </div>
                                    </div>

                                    <div className="settings-list">
                                        <DndContext
                                            sensors={sensors}
                                            collisionDetection={closestCenter}
                                            onDragEnd={handleDragEnd}
                                        >
                                            <SortableContext
                                                items={itnRulesOrder}
                                                strategy={verticalListSortingStrategy}
                                            >
                                                {itnRulesOrder.map(modelId => {
                                                    const model = ITN_MODELS.find(m => m.id === modelId) || { id: modelId, name: modelId, description: '', filename: '' };
                                                    const isInstalled = installedITNModels.has(model.id);
                                                    const isEnabled = enabledITNModels.has(model.id);

                                                    return (
                                                        <SortableItem key={model.id} id={model.id} style={{ borderBottom: '1px solid var(--color-border-subtle)' }}>
                                                            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 0', width: '100%' }}>
                                                                <div>
                                                                    <div style={{ fontWeight: 500 }}>{model.name}</div>
                                                                    <div style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>{model.description}</div>
                                                                </div>

                                                                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                                                    {!isInstalled ? (
                                                                        <>
                                                                            {downloadingId === model.id ? (
                                                                                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                                                                    <span style={{ fontSize: '0.8rem' }}>{Math.round(progress)}%</span>
                                                                                    <button className="btn btn-sm btn-icon" onClick={handleCancelDownload} title="Cancel">
                                                                                        <XIcon />
                                                                                    </button>
                                                                                </div>
                                                                            ) : (
                                                                                <button
                                                                                    className="btn btn-sm btn-secondary"
                                                                                    onClick={() => handleDownloadITN(model.id)}
                                                                                    disabled={!!downloadingId}
                                                                                >
                                                                                    <DownloadIcon />
                                                                                    Download
                                                                                </button>
                                                                            )}
                                                                        </>
                                                                    ) : (
                                                                        <button
                                                                            className="toggle-switch"
                                                                            onClick={() => {
                                                                                const next = new Set(enabledITNModels);
                                                                                if (next.has(model.id)) next.delete(model.id);
                                                                                else next.add(model.id);
                                                                                setEnabledITNModels(next);
                                                                            }}
                                                                            role="switch"
                                                                            aria-checked={isEnabled}
                                                                            aria-label={t('settings.toggle_model', { name: model.name })}
                                                                            style={{ opacity: 1, cursor: 'pointer' }}
                                                                            onPointerDown={(e) => e.stopPropagation()}
                                                                        >
                                                                            <div className="toggle-switch-handle" />
                                                                        </button>
                                                                    )}
                                                                </div>
                                                            </div>
                                                        </SortableItem>
                                                    );
                                                })}
                                            </SortableContext>
                                        </DndContext>
                                    </div>
                                    <div className="settings-hint" style={{ marginTop: 8 }}>
                                        {t('settings.itn_note')}
                                    </div>
                                </div>
                            </div>
                        )}

                    </div>

                    {/* Footer */}
                    <div className="settings-footer">
                        <button className="btn btn-secondary" onClick={onClose}>
                            {t('common.cancel')}
                        </button>
                        <button className="btn btn-primary" onClick={handleSave}>
                            {t('settings.save_button')}
                        </button>
                    </div>
                </div>
            </div>
        </div >
    );
};

export default Settings;
