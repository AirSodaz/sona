import React, { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { AppConfig } from '../../types/transcript';

interface ModelSectionProps {
    title: string;
    type: 'asr' | 'punctuation' | 'vad';
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
    const groupedModels = useMemo(() => {
        const models = PRESET_MODELS.filter(m => {
            if (type === 'asr') {
                const blacklist = ['vad', 'punctuation', 'itn'];
                return !blacklist.includes(m.type);
            }
            return m.type === type;
        });

        const grouped: ModelInfo[][] = [];
        const groupMap = new Map<string, ModelInfo[]>();

        models.forEach(model => {
            if (model.groupId) {
                if (!groupMap.has(model.groupId)) {
                    const group: ModelInfo[] = [];
                    groupMap.set(model.groupId, group);
                    grouped.push(group);
                }
                groupMap.get(model.groupId)!.push(model);
            } else {
                grouped.push([model]);
            }
        });

        return grouped;
    }, [type]);

    return (
        <>
            <div className="settings-label">
                {title}
            </div>
            {groupedModels.map(group => {
                const key = group[0].groupId || group[0].id;
                return (
                    <ModelCard
                        key={key}
                        models={group}
                        installedModels={installedModels}
                        downloads={downloads}
                        onDelete={onDelete}
                        onDownload={onDownload}
                        onCancelDownload={onCancelDownload}
                    />
                );
            })}
        </>
    );
}

interface SettingsModelsTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

export function SettingsModelsTab({
    config,
    updateConfig,
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
                if (model.modes?.includes('streaming')) {
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
                if (model.modes?.includes('offline')) {
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

    const streamingOptions = useMemo(() => {
        return PRESET_MODELS.filter(m => m.modes?.includes('streaming')).map(model => ({
            value: model.id,
            label: `${model.name}${model.versionLabel ? ` (${model.versionLabel})` : ''}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
        }));
    }, [installedModels, t]);

    const offlineOptions = useMemo(() => {
        return PRESET_MODELS.filter(m => m.modes?.includes('offline')).map(model => ({
            value: model.id,
            label: `${model.name}${model.versionLabel ? ` (${model.versionLabel})` : ''}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
        }));
    }, [installedModels, t]);

    return (
        <div
            className="model-list"
            role="tabpanel"
            id="settings-panel-models"
            aria-labelledby="settings-tab-models"
            tabIndex={0}
        >
            <div className="settings-item" style={{ paddingBottom: '16px', marginBottom: '16px', borderBottom: '1px solid var(--color-border)' }}>
                <label htmlFor="settings-streaming-path" className="settings-label" style={{ display: 'block' }}>{t('settings.streaming_model_label', { defaultValue: 'Live Record Model' })}</label>
                <div style={{ display: 'flex', gap: 8, marginBottom: '16px' }}>
                    <Dropdown
                        id="settings-streaming-path"
                        value={selectedStreamingModelId}
                        onChange={(value) => handleStreamingModelChange(value)}
                        placeholder={t('settings.select_streaming_model', { defaultValue: 'Select streaming model...' })}
                        options={streamingOptions}
                        style={{ flex: 1 }}
                    />
                </div>

                <label htmlFor="settings-offline-path" className="settings-label" style={{ display: 'block' }}>{t('settings.offline_model_label', { defaultValue: 'Batch Import Model' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <Dropdown
                        id="settings-offline-path"
                        value={selectedOfflineModelId}
                        onChange={(value) => handleOfflineModelChange(value)}
                        placeholder={t('settings.select_offline_model', { defaultValue: 'Select offline model...' })}
                        options={offlineOptions}
                        style={{ flex: 1 }}
                    />
                </div>
            </div>

            {/* Streaming models removed */}
            <ModelSection title={t('settings.recognition_models')} type="asr" {...sectionProps} />

            {/* Punctuation and VAD models restored */}
            <ModelSection title={t('settings.punctuation_models', { defaultValue: 'Punctuation Models' })} type="punctuation" {...sectionProps} />
            <ModelSection title={t('settings.vad_models', { defaultValue: 'Voice Activity Detection (VAD) Models' })} type="vad" {...sectionProps} />
        </div>
    );
}
