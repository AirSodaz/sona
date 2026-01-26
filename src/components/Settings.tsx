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

interface SettingsProps {
    isOpen: boolean;
    onClose: () => void;
}

export const Settings: React.FC<SettingsProps> = ({ isOpen, onClose }) => {
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);
    const { t, i18n } = useTranslation();

    const [activeTab, setActiveTab] = useState<'general' | 'local' | 'models'>('general');
    const [modelPath, setModelPath] = useState(config.modelPath);
    const [enableITN, setEnableITN] = useState(config.enableITN ?? true);
    const [appLanguage, setAppLanguage] = useState(config.appLanguage || 'auto');
    const [pathStatus, setPathStatus] = useState<'idle' | 'valid' | 'invalid'>('idle');

    // Download state
    const [downloadingId, setDownloadingId] = useState<string | null>(null);
    const [deletingId, setDeletingId] = useState<string | null>(null);
    const [progress, setProgress] = useState(0);
    const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());
    const [statusText, setStatusText] = useState('');

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
        setModelPath(config.modelPath);
        setEnableITN(config.enableITN ?? true);
        setAppLanguage(config.appLanguage || 'auto');
        if (config.modelPath) {
            validatePath(config.modelPath);
        }
    }, [config.modelPath, config.enableITN, config.appLanguage]);

    useEffect(() => {
        checkInstalledModels();
    }, []);

    const validatePath = async (path: string) => {
        if (!path.trim()) {
            setPathStatus('idle');
            return;
        }
        // Simple check
        setPathStatus(path.trim().length > 0 ? 'valid' : 'invalid');
    };

    const handlePathChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const path = e.target.value;
        setModelPath(path);
        validatePath(path);
    };

    const handleSave = () => {
        setConfig({ modelPath, enableITN, appLanguage });
        localStorage.setItem('sona-config', JSON.stringify({ modelPath, enableITN, appLanguage }));

        // Apply language immediately
        if (appLanguage === 'auto') {
            i18n.changeLanguage(navigator.language);
        } else {
            i18n.changeLanguage(appLanguage);
        }

        onClose();
    };

    const handleBrowse = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: t('settings.path_label')
            });

            if (selected) {
                const path = Array.isArray(selected) ? selected[0] : selected;
                if (path) {
                    setModelPath(path);
                    validatePath(path);
                }
            }
        } catch (err) {
            console.error('Failed to open dialog:', err);
        }
    };

    const handleDownload = async (model: ModelInfo) => {
        if (downloadingId) return;

        setDownloadingId(model.id);
        setProgress(0);
        setStatusText(t('settings.download_starting'));

        try {
            const downloadedPath = await modelService.downloadModel(model.id, (pct, status) => {
                setProgress(pct);
                setStatusText(status);
            });

            setModelPath(downloadedPath);
            validatePath(downloadedPath);
            setStatusText(t('settings.download_complete'));
            await checkInstalledModels();

            // Auto-switch to local tab to show result
            setTimeout(() => {
                setActiveTab('local');
                setDownloadingId(null);
            }, 1000);

        } catch (error: any) {
            console.error('Download failed:', error);
            setStatusText(t('settings.download_failed', { error: error.message }));
            setTimeout(() => setDownloadingId(null), 3000);
        }
    };

    const handleLoad = async (model: ModelInfo) => {
        try {
            const path = await modelService.getModelPath(model.id);
            setModelPath(path);
            validatePath(path);
        } catch (error: any) {
            console.error('Load failed:', error);
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
            if (modelPath === deletedPath) {
                setModelPath('');
                setPathStatus('idle');
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
                if (parsed.modelPath) {
                    setConfig({
                        modelPath: parsed.modelPath,
                        enableITN: parsed.enableITN ?? true,
                        appLanguage: parsed.appLanguage || 'auto'
                    });
                }
            } catch (e) {
                console.error('Failed to parse saved config:', e);
            }
        }
    }, [setConfig]);

    if (!isOpen) return null;

    return (
        <div
            style={{
                position: 'fixed',
                inset: 0,
                background: 'rgba(0, 0, 0, 0.7)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                zIndex: 1000,
            }}
            onClick={onClose}
        >
            <div
                style={{
                    background: 'var(--color-bg-secondary)',
                    borderRadius: 'var(--radius-lg)',
                    padding: 0,
                    width: 600,
                    maxWidth: '90vw',
                    display: 'flex',
                    flexDirection: 'column',
                    maxHeight: '85vh',
                    boxShadow: 'var(--shadow-lg)',
                    overflow: 'hidden'
                }}
                onClick={(e) => e.stopPropagation()}
            >
                {/* Header */}
                <div style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '24px 24px 0 24px'
                }}>
                    <h2 style={{ fontSize: '1.25rem', fontWeight: 600 }}>{t('settings.title')}</h2>
                    <button className="btn btn-icon" onClick={onClose}>
                        <XIcon />
                    </button>
                </div>

                <div style={{ display: 'flex', gap: 24, padding: '24px 24px 0 24px', borderBottom: '1px solid var(--color-border)' }}>
                    <button
                        className={`tab-btn ${activeTab === 'general' ? 'active' : ''}`}
                        onClick={() => setActiveTab('general')}
                        style={{
                            background: 'transparent',
                            border: 'none',
                            boxShadow: 'none',
                            borderRadius: 0,
                            padding: '0 0 12px 0',
                            borderBottom: activeTab === 'general' ? '2px solid var(--color-text-primary)' : 'none',
                            color: activeTab === 'general' ? 'var(--color-text-primary)' : 'var(--color-text-muted)',
                            fontWeight: 500
                        }}
                    >
                        {t('settings.general')}
                    </button>
                    <button
                        className={`tab-btn ${activeTab === 'models' ? 'active' : ''}`}
                        onClick={() => setActiveTab('models')}
                        style={{
                            background: 'transparent',
                            border: 'none',
                            boxShadow: 'none',
                            borderRadius: 0,
                            padding: '0 0 12px 0',
                            borderBottom: activeTab === 'models' ? '2px solid var(--color-text-primary)' : 'none',
                            color: activeTab === 'models' ? 'var(--color-text-primary)' : 'var(--color-text-muted)',
                            fontWeight: 500
                        }}
                    >
                        {t('settings.model_hub')}
                    </button>
                    <button
                        className={`tab-btn ${activeTab === 'local' ? 'active' : ''}`}
                        onClick={() => setActiveTab('local')}
                        style={{
                            background: 'transparent',
                            border: 'none',
                            boxShadow: 'none',
                            borderRadius: 0,
                            padding: '0 0 12px 0',
                            borderBottom: activeTab === 'local' ? '2px solid var(--color-text-primary)' : 'none',
                            color: activeTab === 'local' ? 'var(--color-text-primary)' : 'var(--color-text-muted)',
                            fontWeight: 500
                        }}
                    >
                        {t('settings.local_path')}
                    </button>
                </div>

                {/* Content */}
                <div style={{ padding: 24, overflowY: 'auto' }}>

                    {activeTab === 'general' && (
                        <div className="settings-group">
                            <div className="settings-item">
                                <label className="settings-label" style={{ display: 'block', marginBottom: 8, fontWeight: 500 }}>{t('settings.language')}</label>
                                <select
                                    className="settings-input"
                                    value={appLanguage}
                                    onChange={(e) => setAppLanguage(e.target.value as 'auto' | 'en' | 'zh')}
                                    style={{
                                        width: '100%',
                                        padding: '8px 12px',
                                        borderRadius: 6,
                                        border: '1px solid var(--color-border)',
                                        background: 'var(--color-bg-primary)',
                                        color: 'var(--color-text-primary)'
                                    }}
                                >
                                    <option value="auto">{t('common.auto')}</option>
                                    <option value="en">English</option>
                                    <option value="zh">中文</option>
                                </select>
                            </div>
                        </div>
                    )}

                    {activeTab === 'models' && (
                        <div className="model-list" style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                            <div style={{
                                padding: 12,
                                background: 'rgba(55, 53, 47, 0.05)',
                                borderRadius: 8,
                                fontSize: '0.875rem',
                                color: 'var(--color-text-muted)'
                            }}>
                                {t('settings.download_desc')}
                            </div>

                            {PRESET_MODELS.map(model => (
                                <div key={model.id} style={{
                                    border: '1px solid var(--color-border)',
                                    borderRadius: 8,
                                    padding: 16,
                                    display: 'flex',
                                    flexDirection: 'column',
                                    gap: 12
                                }}>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
                                        <div>
                                            <div style={{ fontWeight: 600, fontSize: '1rem' }}>{model.name}</div>
                                            <div style={{ fontSize: '0.875rem', color: 'var(--color-text-muted)', marginTop: 4 }}>{model.description}</div>
                                            <div style={{ display: 'flex', gap: 12, marginTop: 8, fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                                                <span style={{ background: 'rgba(55, 53, 47, 0.08)', padding: '2px 6px', borderRadius: 4 }}>{model.language.toUpperCase()}</span>
                                                <span style={{ background: 'rgba(55, 53, 47, 0.08)', padding: '2px 6px', borderRadius: 4 }}>{model.type}</span>
                                                <span style={{ background: 'rgba(55, 53, 47, 0.08)', padding: '2px 6px', borderRadius: 4 }}>{model.size}</span>
                                            </div>
                                        </div>
                                        {installedModels.has(model.id) ? (
                                            <div style={{ display: 'flex', gap: 8, flexShrink: 0 }}>
                                                <button
                                                    className="btn btn-primary"
                                                    onClick={() => handleLoad(model)}
                                                    style={{ width: 80, display: 'flex', gap: 6, alignItems: 'center', justifyContent: 'center', whiteSpace: 'nowrap' }}
                                                >
                                                    <PlayIcon /> {t('settings.load')}
                                                </button>
                                                <button
                                                    className="btn btn-secondary"
                                                    onClick={() => handleDelete(model)}
                                                    disabled={!!deletingId || !!downloadingId}
                                                    style={{ display: 'flex', gap: 6, alignItems: 'center', justifyContent: 'center', color: 'var(--color-error, #e53e3e)', whiteSpace: 'nowrap' }}
                                                    title="Delete model"
                                                >
                                                    {deletingId === model.id ? (
                                                        <div className="spinner" style={{ width: 14, height: 14, border: '2px solid currentColor', borderRightColor: 'transparent', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
                                                    ) : (
                                                        <TrashIcon />
                                                    )}
                                                </button>
                                            </div>
                                        ) : (
                                            <button
                                                className="btn btn-secondary"
                                                onClick={() => handleDownload(model)}
                                                disabled={!!downloadingId}
                                                style={{ width: 120, display: 'flex', gap: 6, alignItems: 'center', justifyContent: 'center', whiteSpace: 'nowrap' }}
                                            >

                                                {downloadingId === model.id ? t('common.loading') : <><DownloadIcon /> {t('common.download')}</>}
                                            </button>
                                        )}
                                    </div>

                                    {downloadingId === model.id && (
                                        <div style={{ marginTop: 8 }}>
                                            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', marginBottom: 4 }}>
                                                <span>{statusText}</span>
                                                <span>{progress}%</span>
                                            </div>
                                            <div style={{ height: 4, background: 'var(--color-border)', borderRadius: 2, overflow: 'hidden' }}>
                                                <div style={{ height: '100%', width: `${progress}%`, background: 'var(--color-text-primary)', transition: 'width 0.2s' }} />
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
                                <label className="settings-label" style={{ display: 'block', marginBottom: 8, fontWeight: 500 }}>{t('settings.path_label')}</label>
                                <div style={{ display: 'flex', gap: 8 }}>
                                    <input
                                        type="text"
                                        title={modelPath}
                                        className={`settings-input ${pathStatus === 'valid' ? 'valid' : pathStatus === 'invalid' ? 'invalid' : ''}`}
                                        value={modelPath}
                                        onChange={handlePathChange}
                                        placeholder={t('settings.path_placeholder')}
                                        style={{
                                            flex: 1,
                                            padding: '8px 12px',
                                            borderRadius: 6,
                                            border: '1px solid var(--color-border)',
                                            background: 'var(--color-bg-primary)',
                                            color: 'var(--color-text-primary)'
                                        }}
                                    />
                                    <button className="btn btn-secondary" onClick={handleBrowse}>
                                        <FolderIcon />
                                    </button>
                                </div>
                                <div className="settings-hint" style={{ marginTop: 8, fontSize: '0.875rem', color: 'var(--color-text-muted)' }}>
                                    {pathStatus === 'valid' && (
                                        <span style={{ color: 'var(--color-success)', display: 'flex', alignItems: 'center', gap: 4 }}>
                                            <CheckIcon /> {t('settings.valid_path')}
                                        </span>
                                    )}
                                    {pathStatus === 'invalid' && (
                                        <span style={{ color: 'var(--color-error)' }}>
                                            {t('settings.invalid_path')}
                                        </span>
                                    )}
                                    {pathStatus === 'idle' && (
                                        t('settings.path_hint')
                                    )}
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
                                        onClick={() => setEnableITN(!enableITN)}
                                        style={{
                                            width: 44,
                                            height: 24,
                                            borderRadius: 12,
                                            background: enableITN ? 'var(--color-text-primary)' : 'var(--color-border)',
                                            position: 'relative',
                                            transition: 'background 0.2s',
                                            border: 'none',
                                            cursor: 'pointer'
                                        }}
                                        title="Enable ITN"
                                    >
                                        <div style={{
                                            width: 20,
                                            height: 20,
                                            borderRadius: '50%',
                                            background: 'var(--color-bg-primary)',
                                            position: 'absolute',
                                            top: 2,
                                            left: enableITN ? 22 : 2,
                                            transition: 'left 0.2s',
                                            boxShadow: '0 1px 2px rgba(0,0,0,0.1)'
                                        }} />
                                    </button>
                                </div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', marginTop: 8 }}>
                                    {t('settings.itn_note')}
                                </div>
                            </div>
                        </div>
                    )}
                </div>

                <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 12, padding: 24, borderTop: '1px solid var(--color-border)' }}>
                    <button className="btn btn-secondary" onClick={onClose}>
                        {t('common.cancel')}
                    </button>
                    <button className="btn btn-primary" onClick={handleSave}>
                        {t('settings.save_button')}
                    </button>
                </div>
            </div>
        </div>
    );
};

export default Settings;
