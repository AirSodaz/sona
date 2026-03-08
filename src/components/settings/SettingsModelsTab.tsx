import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { AppConfig } from '../../types/transcript';

interface ModelSectionProps {
    title: string;
    type: 'sensevoice' | 'paraformer' | 'punctuation' | 'vad' | 'ctc' | ('sensevoice' | 'paraformer' | 'punctuation' | 'vad' | 'ctc')[];
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

function ModelSection({
    title,
    type,
    installedModels,
    downloads,
    onDelete,
    onDownload,
    onCancelDownload
}: ModelSectionProps): React.JSX.Element {
    const typesArray = Array.isArray(type) ? type : [type];
    const models = PRESET_MODELS.filter(m => typesArray.includes(m.type as any));

    return (
        <>
            <div className="settings-section-subtitle">
                {title}
            </div>
            {models.map(model => (
                <ModelCard
                    key={model.id}
                    model={model}
                    isInstalled={installedModels.has(model.id)}
                    isDownloading={!!downloads[model.id]}
                    progress={downloads[model.id]?.progress || 0}
                    statusMessage={downloads[model.id]?.status || ''}
                    onDelete={onDelete}
                    onDownload={onDownload}
                    onCancelDownload={() => onCancelDownload(model.id)}
                />
            ))}
        </>
    );
}

interface SettingsModelsTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    handleBrowse: (type: 'sensevoice' | 'paraformer' | 'punctuation' | 'vad' | 'ctc') => Promise<void>;
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

export function SettingsModelsTab({
    config,
    updateConfig,
    handleBrowse,
    installedModels,
    downloads,
    onDelete,
    onDownload,
    onCancelDownload
}: SettingsModelsTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [selectedStreamingModelId, setSelectedStreamingModelId] = useState<string>('');
    const [selectedOfflineModelId, setSelectedOfflineModelId] = useState<string>('');

    const streamingModelPath = config.streamingModelPath;
    const offlineModelPath = config.offlineModelPath;

    // Sync streamingModelPath with selected model ID
    useEffect(() => {
        const findModel = async () => {
            if (!streamingModelPath) {
                setSelectedStreamingModelId('');
                return;
            }

            for (const model of PRESET_MODELS) {
                if ((model.type === 'sensevoice' || model.type === 'paraformer') && model.modes?.includes('streaming')) {
                    const path = await modelService.getModelPath(model.id);
                    if (path === streamingModelPath) {
                        setSelectedStreamingModelId(model.id);
                        return;
                    }
                }
            }
            setSelectedStreamingModelId('');
        };
        findModel();
    }, [streamingModelPath]);

    // Sync offlineModelPath with selected model ID
    useEffect(() => {
        const findModel = async () => {
            if (!offlineModelPath) {
                setSelectedOfflineModelId('');
                return;
            }

            for (const model of PRESET_MODELS) {
                if (model.type === 'sensevoice' && model.modes?.includes('offline')) {
                    const path = await modelService.getModelPath(model.id);
                    if (path === offlineModelPath) {
                        setSelectedOfflineModelId(model.id);
                        return;
                    }
                }
            }
            setSelectedOfflineModelId('');
        };
        findModel();
    }, [offlineModelPath]);

    const applyModelRules = async (modelId: string) => {
        try {
            const rules = modelService.getModelRules(modelId);

            if (rules.requiresVad) {
                const vadModelId = 'silero-vad';
                // Only sync and save if path is missing in config
                if (!config.vadModelPath) {
                    if (installedModels.has(vadModelId)) {
                        const vadPath = await modelService.getModelPath(vadModelId);
                        updateConfig({ vadModelPath: vadPath });
                    } else {
                        document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: vadModelId } }));
                    }
                }
            }

            if (rules.requiresPunctuation) {
                const punctModelId = 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8';
                // Only sync and save if path is missing in config
                if (!config.punctuationModelPath) {
                    if (installedModels.has(punctModelId)) {
                        const punctPath = await modelService.getModelPath(punctModelId);
                        updateConfig({ punctuationModelPath: punctPath });
                    } else {
                        document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: punctModelId } }));
                    }
                }
            }
        } catch (e) {
            console.error('Failed to apply model rules', e);
        }
    };

    const handleStreamingModelChange = async (modelId: string) => {
        setSelectedStreamingModelId(modelId);
        if (!modelId) {
             updateConfig({ streamingModelPath: '' });
             return;
        }

        try {
            const path = await modelService.getModelPath(modelId);
            updateConfig({ streamingModelPath: path });
            await applyModelRules(modelId);
        } catch (e) {
            console.error('Failed to get streaming model path', e);
        }
    };

    const handleOfflineModelChange = async (modelId: string) => {
        setSelectedOfflineModelId(modelId);
        if (!modelId) {
             updateConfig({ offlineModelPath: '' });
             return;
        }

        try {
            const path = await modelService.getModelPath(modelId);
            updateConfig({ offlineModelPath: path });
            await applyModelRules(modelId);

        } catch (e) {
            console.error('Failed to get offline model path', e);
        }
    };

    const sectionProps = {
        installedModels,
        downloads,
        onDelete,
        onDownload,
        onCancelDownload
    };

    return (
        <div
            className="model-list"
            role="tabpanel"
            id="settings-panel-models"
            aria-labelledby="settings-tab-models"
            tabIndex={0}
        >
            <div className="settings-item" style={{ paddingBottom: '16px', marginBottom: '16px', borderBottom: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-streaming-path" className="settings-label" style={{ fontSize: '1.1em', fontWeight: 600 }}>{t('settings.streaming_model_label', { defaultValue: 'Streaming Model' })}</label>
                <div style={{ display: 'flex', gap: 8, marginBottom: '16px' }}>
                    <Dropdown
                        id="settings-streaming-path"
                        value={selectedStreamingModelId}
                        onChange={(value) => handleStreamingModelChange(value)}
                        placeholder={t('settings.select_streaming_model', { defaultValue: 'Select streaming model...' })}
                        options={PRESET_MODELS.filter(m => (m.type === 'sensevoice' || m.type === 'paraformer') && m.modes?.includes('streaming')).map(model => ({
                            value: model.id,
                            label: `${model.name}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
                            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
                        }))}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('sensevoice')}
                        title={t('common.browse', { defaultValue: 'Browse' })}
                    >
                        ...
                    </button>
                </div>

                <label htmlFor="settings-offline-path" className="settings-label" style={{ fontSize: '1.1em', fontWeight: 600 }}>{t('settings.offline_model_label', { defaultValue: 'Offline Model' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <Dropdown
                        id="settings-offline-path"
                        value={selectedOfflineModelId}
                        onChange={(value) => handleOfflineModelChange(value)}
                        placeholder={t('settings.select_offline_model', { defaultValue: 'Select offline model...' })}
                        options={PRESET_MODELS.filter(m => m.type === 'sensevoice' && m.modes?.includes('offline')).map(model => ({
                            value: model.id,
                            label: `${model.name}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
                            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
                        }))}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('sensevoice')}
                        title={t('common.browse', { defaultValue: 'Browse' })}
                    >
                        ...
                    </button>
                </div>
            </div>

            {/* Streaming models removed */}
            <ModelSection title={t('settings.recognition_models')} type={['sensevoice', 'paraformer']} {...sectionProps} />
            <ModelSection title={t('settings.ctc_models')} type="ctc" {...sectionProps} />

            {/* Removed VAD and Punctuation models from UI */}
            {/* Removed VAD buffer size setting */}
        </div>
    );
}
