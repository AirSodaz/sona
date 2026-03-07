import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { AppConfig } from '../../types/transcript';

interface ModelSectionProps {
    title: string;
    type: 'offline' | 'punctuation' | 'vad' | 'ctc';
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
    const models = PRESET_MODELS.filter(m => m.type === type);

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
    handleBrowse: (type: 'offline' | 'punctuation' | 'vad' | 'ctc') => Promise<void>;
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
    const [selectedOfflineModelId, setSelectedOfflineModelId] = useState<string>('');

    const offlineModelPath = config.offlineModelPath;

    // Sync offlineModelPath with selected model ID
    useEffect(() => {
        const findModel = async () => {
            if (!offlineModelPath) {
                setSelectedOfflineModelId('');
                return;
            }

            for (const model of PRESET_MODELS) {
                if (model.type === 'offline') {
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

    const handleOfflineModelChange = async (modelId: string) => {
        setSelectedOfflineModelId(modelId);
        if (!modelId) {
             updateConfig({ offlineModelPath: '' });
             return;
        }

        try {
            const path = await modelService.getModelPath(modelId);
            updateConfig({ offlineModelPath: path });

            // Automatically apply model rules for VAD and Punctuation based on the new selection
            const rules = modelService.getModelRules(modelId);

            if (rules.requiresVad) {
                const vadModelId = 'silero-vad';
                if (installedModels.has(vadModelId)) {
                    const vadPath = await modelService.getModelPath(vadModelId);
                    updateConfig({ vadModelPath: vadPath });
                } else {
                    document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: vadModelId } }));
                }
            } else {
                updateConfig({ vadModelPath: '' });
            }

            if (rules.requiresPunctuation) {
                const punctModelId = 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8';
                if (installedModels.has(punctModelId)) {
                    const punctPath = await modelService.getModelPath(punctModelId);
                    updateConfig({ punctuationModelPath: punctPath });
                } else {
                    document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: punctModelId } }));
                }
            } else {
                updateConfig({ punctuationModelPath: '' });
            }

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
                <label htmlFor="settings-offline-path" className="settings-label" style={{ fontSize: '1.1em', fontWeight: 600 }}>{t('settings.offline_path_label', { defaultValue: 'Select Model' })}</label>
                <div style={{ display: 'flex', gap: 8 }}>
                    <Dropdown
                        id="settings-offline-path"
                        value={selectedOfflineModelId}
                        onChange={(value) => handleOfflineModelChange(value)}
                        placeholder={t('settings.select_model', { defaultValue: 'Select a model...' })}
                        options={PRESET_MODELS.filter(m => m.type === 'offline').map(model => ({
                            value: model.id,
                            label: `${model.name}${!installedModels.has(model.id) ? t('settings.not_installed', { defaultValue: ' (Not Downloaded)' }) : ''}`,
                            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } : undefined
                        }))}
                        style={{ flex: 1 }}
                    />
                    <button
                        className="btn btn-secondary"
                        onClick={() => handleBrowse('offline')}
                        title={t('common.browse', { defaultValue: 'Browse' })}
                    >
                        ...
                    </button>
                </div>
            </div>

            {/* Streaming models removed */}
            <ModelSection title={t('settings.offline_models')} type="offline" {...sectionProps} />
            <ModelSection title={t('settings.punctuation_models')} type="punctuation" {...sectionProps} />
            <ModelSection title={t('settings.vad_models')} type="vad" {...sectionProps} />
            <ModelSection title={t('settings.ctc_models')} type="ctc" {...sectionProps} />

            {/* Removed VAD buffer size setting */}
        </div>
    );
}
