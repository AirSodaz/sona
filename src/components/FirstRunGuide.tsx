import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { modelService, PRESET_MODELS } from '../services/modelService';
import { DownloadIcon, CheckIcon } from './Icons';

export function FirstRunGuide() {
    const { t } = useTranslation();
    const [isVisible, setIsVisible] = useState(false);
    const [status, setStatus] = useState<'idle' | 'downloading' | 'error'>('idle');
    const [progress, setProgress] = useState<Record<string, { pct: number, status: string }>>({});
    const setConfig = useTranscriptStore((state) => state.setConfig);

    useEffect(() => {
        // Check if first run
        const completed = localStorage.getItem('sona-first-run-completed');
        if (!completed) {
            setIsVisible(true);
        }
    }, []);

    const handleOneClickDownload = async () => {
        setStatus('downloading');

        const modelsToDownload = [
            // Recognition
            { id: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17', type: 'offline' },
            // Punctuation
            { id: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8', type: 'punctuation' },
            // VAD
            { id: 'silero-vad', type: 'vad' }
        ];

        // Initialize progress
        const initialProgress: Record<string, { pct: number, status: string }> = {};
        for (const model of modelsToDownload) {
            initialProgress[model.id] = { pct: 0, status: 'Starting...' };
        }
        setProgress(initialProgress);

        try {
            // Concurrent downloads
            const downloadPromises = modelsToDownload.map(async (model) => {
                const path = await modelService.downloadModel(model.id, (pct, statusText) => {
                    setProgress(prev => ({
                        ...prev,
                        [model.id]: { pct, status: statusText }
                    }));
                });
                return { type: model.type, path };
            });

            const results = await Promise.all(downloadPromises);

            const paths: Record<string, string> = {};
            results.forEach(r => {
                paths[r.type] = r.path;
            });

            // Update Config
            const newConfig = {
                offlineModelPath: paths['offline'],
                vadModelPath: paths['vad'],
                punctuationModelPath: '', // Downloaded but not enabled
                ctcModelPath: '', // Not downloaded
                enableITN: true,
                enabledITNModels: [] // Chinese Number ITN not downloaded/enabled
            };

            setConfig(newConfig);

            // Persist to localStorage (replicating logic from useSettingsLogic/useAppInitialization)
            const saved = localStorage.getItem('sona-config');
            let currentSavedConfig = {};
            if (saved) {
                try {
                    currentSavedConfig = JSON.parse(saved);
                } catch (e) { /* ignore */ }
            }

            const configToSave = {
                ...currentSavedConfig,
                ...newConfig
            };

            localStorage.setItem('sona-config', JSON.stringify(configToSave));

            completeFirstRun();

        } catch (error) {
            console.error('First run download failed:', error);
            setStatus('error');
        }
    };

    const completeFirstRun = () => {
        localStorage.setItem('sona-first-run-completed', 'true');
        setIsVisible(false);
    };

    if (!isVisible) return null;

    return (
        <div className="settings-overlay" style={{ zIndex: 2000 }}>
            <div className="settings-modal" style={{ height: 'auto', maxHeight: '90vh', width: '600px', flexDirection: 'column' }}>
                <div style={{ padding: '32px 32px 24px', textAlign: 'center' }}>
                    <h2 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>{t('first_run.title')}</h2>
                    <p style={{ color: 'var(--color-text-secondary)', marginBottom: '32px' }}>
                        {t('first_run.description')}
                    </p>

                    {status === 'idle' && (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                            <button
                                className="btn btn-primary"
                                onClick={handleOneClickDownload}
                                style={{ padding: '12px', fontSize: '1rem', justifyContent: 'center' }}
                            >
                                <DownloadIcon />
                                {t('first_run.one_click_download')}
                            </button>
                            <button
                                className="btn btn-secondary"
                                onClick={completeFirstRun}
                                style={{ padding: '12px', fontSize: '1rem', justifyContent: 'center' }}
                            >
                                {t('first_run.skip')}
                            </button>
                        </div>
                    )}

                    {status === 'downloading' && (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '24px', textAlign: 'left' }}>
                            <div style={{ textAlign: 'center', fontWeight: 500 }}>
                                {t('first_run.downloading')}
                            </div>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                                {Object.entries(progress).map(([id, info]) => {
                                    const modelDef = PRESET_MODELS.find(m => m.id === id);
                                    const isComplete = info.pct === 100;
                                    return (
                                        <div key={id}>
                                            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', marginBottom: '6px', alignItems: 'center' }}>
                                                <span style={{ fontWeight: 500, display: 'flex', alignItems: 'center', gap: '6px' }}>
                                                    {modelDef?.name || id}
                                                    {isComplete && <CheckIcon style={{ width: 14, height: 14, color: 'var(--color-success)' }} />}
                                                </span>
                                                <span style={{ color: isComplete ? 'var(--color-success)' : 'var(--color-text-muted)' }}>
                                                    {isComplete ? 'Done' : `${info.pct}%`}
                                                </span>
                                            </div>
                                            <div className="progress-bar-mini">
                                                <div
                                                    className="progress-fill"
                                                    style={{
                                                        width: `${info.pct}%`,
                                                        backgroundColor: isComplete ? 'var(--color-success)' : undefined
                                                    }}
                                                />
                                            </div>
                                            <div style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', marginTop: '4px', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                                                {info.status}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {status === 'error' && (
                        <div style={{ color: 'var(--color-error)', marginTop: '16px' }}>
                            {t('first_run.error')}
                            <div style={{ marginTop: '16px' }}>
                                <button className="btn btn-secondary" onClick={() => setStatus('idle')}>
                                    {t('common.cancel')}
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
