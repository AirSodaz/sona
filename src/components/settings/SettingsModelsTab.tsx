import React from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';

interface ModelSectionProps {
    title: string;
    type: 'streaming' | 'offline' | 'punctuation' | 'vad' | 'ctc';
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
            <div className="settings-section-subtitle" style={{ marginTop: 30, marginBottom: 10, fontWeight: 'bold' }}>
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
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onLoad: (model: ModelInfo) => void;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
    isModelSelected: (model: ModelInfo) => boolean;
}

export function SettingsModelsTab({
    installedModels,
    downloads,
    onLoad,
    onDelete,
    onDownload,
    onCancelDownload,
    isModelSelected
}: SettingsModelsTabProps): React.JSX.Element {
    const { t } = useTranslation();

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
            <ModelSection title={t('settings.streaming_models')} type="streaming" {...sectionProps} />
            <ModelSection title={t('settings.offline_models')} type="offline" {...sectionProps} />
            <ModelSection title={t('settings.punctuation_models')} type="punctuation" {...sectionProps} />
            <ModelSection title={t('settings.vad_models')} type="vad" {...sectionProps} />
            <ModelSection title={t('settings.ctc_models')} type="ctc" {...sectionProps} />

            {/* Removed VAD buffer size setting */}
        </div>
    );
}
