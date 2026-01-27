import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { PRESET_MODELS, modelService, ModelInfo } from '../services/modelService';
import { open, ask, message } from '@tauri-apps/plugin-dialog';

// Icons
const FolderIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
    </svg>
);

const CheckIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="20,6 9,17 4,12" />
    </svg>
);

const DownloadIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
        <polyline points="7 10 12 15 17 10" />
        <line x1="12" x2="12" y1="15" y2="3" />
    </svg>
);

const XIcon = () => (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <line x1="18" x2="6" y1="6" y2="18" />
        <line x1="6" x2="18" y1="6" y2="18" />
    </svg>
);

const TrashIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polyline points="3 6 5 6 21 6" />
        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
    </svg>
);

const PlayIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="5 3 19 12 5 21 5 3" />
    </svg>
);

// Sidebar Icons
const GeneralIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="3"></circle>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
    </svg>
);

const ModelIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
        <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
        <line x1="12" y1="22.08" x2="12" y2="12"></line>
    </svg>
);

const LocalIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M20 17.58A5 5 0 0 0 18 8h-1.26A8 8 0 1 0 4 16.25"></path>
        <polyline points="8 16 12 20 16 16"></polyline>
        <line x1="12" y1="12" x2="12" y2="20"></line>
    </svg>
);

interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
}

export const Settings: React.FC<SettingsProps> = ({ isOpen, onClose }) => {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'local' | 'models'>('general');
    const [streamingModelPath, setStreamingModelPath] = useState(config.streamingModelPath);
    const [offlineModelPath, setOfflineModelPath] = useState(config.offlineModelPath);
    const [enableITN, setEnableITN] = useState(config.enableITN ?? true);
    const [appLanguage, setAppLanguage] = useState(config.appLanguage || 'auto');

    const [theme, setTheme] = useState(config.theme || 'auto');
    const [font, setFont] = useState(config.font || 'system');



    // Download state
    const [downloadingId, setDownloadingId] = useState<string | null>(null);
    const [deletingId, setDeletingId] = useState<string | null>(null);
    const [progress, setProgress] = useState(0);
    const [statusMessage, setStatusMessage] = useState('');
    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());


    const checkInstalledModels = async () => {
        const installed = new Set<string>();
        for (const model of PRESET_MODELS) {
            if (await modelService.isModelInstalled(model.id)) {
                installed.add(model.id);
            }
        }
        setInstalledModels(installed);
    };

    useEffect(() => {
        setStreamingModelPath(config.streamingModelPath);
        setOfflineModelPath(config.offlineModelPath);
        setEnableITN(config.enableITN ?? true);
        setAppLanguage(config.appLanguage || 'auto');

        setTheme(config.theme || 'auto');
        setFont(config.font || 'system');
        // Validate both (optional visual feedback, maybe just validate active input)
    }, [config.streamingModelPath, config.offlineModelPath, config.enableITN, config.appLanguage, config.theme, config.font]);

    useEffect(() => {
        checkInstalledModels();
    }, []);



    const handleSave = () => {

        setConfig({ streamingModelPath, offlineModelPath, enableITN, appLanguage, theme, font });
        localStorage.setItem('sona-config', JSON.stringify({ streamingModelPath, offlineModelPath, enableITN, appLanguage, theme, font }));


        // Apply language immediately
        if (appLanguage === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(appLanguage);
        }

        onClose();
    };

    const handleBrowse = async (type: 'streaming' | 'offline') => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: type === 'streaming' ? t('settings.streaming_path_label') : t('settings.offline_path_label')
            });

            if (selected) {
                const path = Array.isArray(selected) ? selected[0] : selected;
                if (path) {
                    if (type === 'streaming') {
                        setStreamingModelPath(path);
                    } else {
                        setOfflineModelPath(path);
                    }
                }
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
        }
    };

    const handleDownload = async (model: ModelInfo) => {
        if (downloadingId) return;

        // Check hardware compatibility
        const { compatible, reason } = await modelService.checkHardware(model.id);
        if (!compatible) {
            const confirmed = await ask(
                `${reason}\n\nDo you want to download it anyway?`,
                { title: 'Hardware Warning', kind: 'warning' }
            );
            if (!confirmed) return;
        }

        setDownloadingId(model.id);
        setProgress(0);

        try {
            const downloadedPath = await modelService.downloadModel(model.id, (pct, status) => {
                setProgress(pct);
                setStatusMessage(status);
            });

            setModelPathByType(model.type, downloadedPath);

            await checkInstalledModels();

            // Auto-switch to local tab to show result
            setTimeout(() => {
                setActiveTab('local');
                setDownloadingId(null);
            }, 1000);

        } catch (error: any) {
            console.error('Download failed:', error);
            setTimeout(() => setDownloadingId(null), 3000);
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

    const setModelPathByType = (type: 'streaming' | 'offline', path: string) => {
        if (type === 'streaming') {
            setStreamingModelPath(path);
        } else {
            setOfflineModelPath(path);
        }
    };

    const handleDelete = async (model: ModelInfo) => {
        if (deletingId) return;

        // Confirm deletion
        const confirmed = await ask(t('settings.delete_confirm_message', { name: model.name }), {
            title: t('settings.delete_confirm_title'),
            kind: 'warning'
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
        } catch (error: any) {
            console.error('Delete failed:', error);
            await message(`Failed to delete model: ${error.message}`, {
                title: 'Error',
                kind: 'error'
            });
        } finally {
            setDeletingId(null);
        }
    };

    // Load config from localStorage on mount
    useEffect(() => {
        const saved = localStorage.getItem('sona-config');
        if (saved) {
            try {
                const parsed = JSON.parse(saved);
                if (parsed.streamingModelPath || parsed.offlineModelPath || parsed.modelPath) {
                    // Migration: if old modelPath exists and new ones don't, mapping it to streaming (or just ignoring)
                    // Let's assume clear separation needed, but if old config exists, we can try to reuse it for streaming
                    const legacyPath = parsed.modelPath || '';

                    setConfig({
                        streamingModelPath: parsed.streamingModelPath || legacyPath,
                        offlineModelPath: parsed.offlineModelPath || '',
                        enableITN: parsed.enableITN ?? true,
                        appLanguage: parsed.appLanguage || 'auto',
                        theme: parsed.theme || 'auto',
                        font: parsed.font || 'system'

                    });
                }
            } catch (e) {
                console.error('Failed to parse saved config:', e);
            }
        }
    }, [setConfig]);

    if (!isOpen) return null;

    const renderTabButton = (id: 'general' | 'models' | 'local', label: string, Icon: React.FC) => (
        <button
            className={`settings-tab-btn ${activeTab === id ? 'active' : ''}`}
            onClick={() => setActiveTab(id)}
        >
            <Icon />
            {label}
        </button>
    );

    return (
        <div className="settings-overlay" onClick={onClose}>
            <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
                {/* Sidebar */}
                <div className="settings-sidebar">
                    <div className="settings-sidebar-header">
                        <h2>{t('settings.title')}</h2>
                    </div>

                    <div className="settings-tabs-container">
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
                            <div className="settings-group">
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
                            <div className="model-list">
                                <div className="settings-section-subtitle" style={{ marginBottom: 10, fontWeight: 'bold' }}>{t('settings.streaming_models')}</div>
                                {PRESET_MODELS.filter(m => m.type === 'streaming').map(model => (
                                    <div key={model.id} className="model-card">
                                        <div className="model-card-header">
                                            <div>
                                                <div className="model-name">{model.name}</div>
                                                <div className="model-description">{model.description}</div>
                                                <div className="model-tags">
                                                    <span className="model-tag">{model.language.toUpperCase()}</span>
                                                    <span className="model-tag">{model.engine.toUpperCase()}</span>
                                                    <span className="model-tag">{model.size}</span>
                                                </div>
                                            </div>
                                            {installedModels.has(model.id) ? (
                                                <div className="model-actions">
                                                    <button
                                                        className={`btn ${streamingModelPath.includes(model.filename || model.id) ? 'btn-success' : 'btn-primary'}`}
                                                        onClick={() => handleLoad(model)}
                                                        disabled={streamingModelPath.includes(model.filename || model.id)}
                                                        aria-label={`${t('settings.load')} ${model.name}`}
                                                    >
                                                        {streamingModelPath.includes(model.filename || model.id) ? <CheckIcon /> : <PlayIcon />}
                                                    </button>
                                                    <button
                                                        className="btn btn-secondary"
                                                        onClick={() => handleDelete(model)}
                                                        disabled={!!deletingId || !!downloadingId}
                                                        aria-label={`${t('common.delete')} ${model.name}`}
                                                    >
                                                        {deletingId === model.id ? <div className="spinner" /> : <TrashIcon />}
                                                    </button>
                                                </div>
                                            ) : (
                                                <button
                                                    className="btn btn-secondary"
                                                    onClick={() => handleDownload(model)}
                                                    disabled={!!downloadingId}
                                                    style={{ width: 120 }}
                                                    aria-label={`${t('common.download')} ${model.name}`}
                                                >
                                                    {downloadingId === model.id ? t('common.loading') : <><DownloadIcon /> {t('common.download')}</>}
                                                </button>
                                            )}
                                        </div>
                                        {downloadingId === model.id && (
                                            <div className="progress-container-mini">
                                                <div className="progress-info-mini">
                                                    <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{statusMessage || t('common.loading')}</span>
                                                    <span>{progress}%</span>
                                                </div>
                                                <div className="progress-bar-mini">
                                                    <div className="progress-fill" style={{ width: `${progress}%` }} />
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                ))}

                                <div className="settings-section-subtitle" style={{ marginTop: 30, marginBottom: 10, fontWeight: 'bold' }}>{t('settings.offline_models')}</div>
                                {PRESET_MODELS.filter(m => m.type === 'offline').map(model => (
                                    <div key={model.id} className="model-card">
                                        <div className="model-card-header">
                                            <div>
                                                <div className="model-name">{model.name}</div>
                                                <div className="model-description">{model.description}</div>
                                                <div className="model-tags">
                                                    <span className="model-tag">{model.language.toUpperCase()}</span>
                                                    <span className="model-tag">{model.engine.toUpperCase()}</span>
                                                    <span className="model-tag">{model.size}</span>
                                                </div>
                                            </div>
                                            {installedModels.has(model.id) ? (
                                                <div className="model-actions">
                                                    <button
                                                        className={`btn ${offlineModelPath.includes(model.filename || model.id) ? 'btn-success' : 'btn-primary'}`}
                                                        onClick={() => handleLoad(model)}
                                                        disabled={offlineModelPath.includes(model.filename || model.id)}
                                                        aria-label={`${t('settings.load')} ${model.name}`}
                                                    >
                                                        {offlineModelPath.includes(model.filename || model.id) ? <CheckIcon /> : <PlayIcon />}
                                                    </button>
                                                    <button
                                                        className="btn btn-secondary"
                                                        onClick={() => handleDelete(model)}
                                                        disabled={!!deletingId || !!downloadingId}
                                                        aria-label={`${t('common.delete')} ${model.name}`}
                                                    >
                                                        {deletingId === model.id ? <div className="spinner" /> : <TrashIcon />}
                                                    </button>
                                                </div>
                                            ) : (
                                                <button
                                                    className="btn btn-secondary"
                                                    onClick={() => handleDownload(model)}
                                                    disabled={!!downloadingId}
                                                    style={{ width: 120 }}
                                                    aria-label={`${t('common.download')} ${model.name}`}
                                                >
                                                    {downloadingId === model.id ? t('common.loading') : <><DownloadIcon /> {t('common.download')}</>}
                                                </button>
                                            )}
                                        </div>
                                        {downloadingId === model.id && (
                                            <div className="progress-container-mini">
                                                <div className="progress-info-mini">
                                                    <span style={{ color: 'var(--color-text-muted)', fontSize: '0.75rem' }}>{statusMessage || t('common.loading')}</span>
                                                    <span>{progress}%</span>
                                                </div>
                                                <div className="progress-bar-mini">
                                                    <div className="progress-fill" style={{ width: `${progress}%` }} />
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                ))}
                            </div>
                        )}

                        {activeTab === 'local' && (
                            <div className="settings-group">
                                <div className="settings-item">
                                    <label className="settings-label">{t('settings.streaming_path_label', { defaultValue: 'Streaming Model Path' })}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
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
                                    <label className="settings-label">{t('settings.offline_path_label', { defaultValue: 'Offline Model Path' })}</label>
                                    <div style={{ display: 'flex', gap: 8 }}>
                                        <input
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

                                <div className="settings-item" style={{ marginTop: 24, borderTop: '1px solid var(--color-border)', paddingTop: 24 }}>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                        <div>
                                            <div style={{ fontWeight: 500, marginBottom: 4 }}>{t('settings.itn_title')}</div>
                                            <div style={{ fontSize: '0.875rem', color: 'var(--color-text-muted)' }}>
                                                {t('settings.itn_desc')}
                                            </div>
                                        </div>

                                        <button
                                            className="toggle-switch"
                                            onClick={() => setEnableITN(!enableITN)}
                                            role="switch"
                                            aria-checked={enableITN}
                                            aria-label={t('settings.itn_title')}
                                            data-tooltip={t('settings.itn_title')}
                                            data-tooltip-pos="left"
                                        >
                                            <div className="toggle-switch-handle" />
                                        </button>
                                    </div>
                                    <div className="settings-hint">
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
        </div>
    );
};

export default Settings;
