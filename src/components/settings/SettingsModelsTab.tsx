import React from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { AppConfig } from '../../types/transcript';

interface ModelSectionProps {
    title: string;
    type: 'offline' | 'punctuation' | 'vad' | 'ctc';
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
    isModelSelected: (model: ModelInfo) => boolean;
}

function ModelSection({
    title,
    type,
    installedModels,
    downloads,
    onLoad,
    onDelete,
    onDownload,
    onCancelDownload,
    isModelSelected
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
                    isSelected={isModelSelected(model)}
                    isDownloading={!!downloads[model.id]}
                    progress={downloads[model.id]?.progress || 0}
                    statusMessage={downloads[model.id]?.status || ''}
                    onLoad={onLoad}
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
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
    isModelSelected: (model: ModelInfo) => boolean; // Keep for backward compatibility if needed, but we'll use internal logic or this
}

export function SettingsModelsTab({
    config,
    installedModels,
    downloads,
    onLoad,
    onDelete,
    onDownload,
    onCancelDownload,
    isModelSelected: propsIsModelSelected // Allow overriding or ignoring
}: SettingsModelsTabProps): React.JSX.Element {
    const { t } = useTranslation();

    const isModelSelected = (model: ModelInfo): boolean => {
        if (propsIsModelSelected) return propsIsModelSelected(model);

        // Fallback to internal logic using config
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
    };

    const sectionProps = {
        installedModels,
        downloads,
        onLoad,
        onDelete,
        onDownload,
        onCancelDownload,
        isModelSelected
    };

    return (
        <div
            className="model-list"
            role="tabpanel"
            id="settings-panel-models"
            aria-labelledby="settings-tab-models"
            tabIndex={0}
        >
            {/* Streaming models removed */}
            <ModelSection title={t('settings.offline_models')} type="offline" {...sectionProps} />
            <ModelSection title={t('settings.punctuation_models')} type="punctuation" {...sectionProps} />
            <ModelSection title={t('settings.vad_models')} type="vad" {...sectionProps} />
            <ModelSection title={t('settings.ctc_models')} type="ctc" {...sectionProps} />

            {/* Removed VAD buffer size setting */}
        </div>
    );
}
