import React, { useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { modelService } from '../../services/modelService';
import type { ModelCatalogGroup, ModelCatalogSectionType, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { useModelConfig, useSetConfig, useTranscriptionConfig } from '../../stores/configStore';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { Mic, Type, Activity, Settings2, PlaySquare } from 'lucide-react';
import { ModelIcon, RestoreIcon } from '../Icons';
import { logger } from '../../utils/logger';
import { useModelManagerContext } from '../../hooks/useModelManager';
import { Switch } from '../Switch';

interface ModelSectionProps {
    title: string;
    description?: string;
    icon?: React.ReactNode;
    groups: ModelCatalogGroup[];
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

const ModelSection = React.memo(function ModelSection({
    title,
    description,
    icon,
    groups,
    installedModels,
    downloads,
    onDelete,
    onDownload,
    onCancelDownload
}: ModelSectionProps): React.JSX.Element {
    return (
        <SettingsSection title={title} description={description} icon={icon}>
            {groups.map(group => {
                return (
                    <ModelCard
                        key={group.key}
                        models={group.models}
                        installedModels={installedModels}
                        downloads={downloads}
                        onDelete={onDelete}
                        onDownload={onDownload}
                        onCancelDownload={onCancelDownload}
                    />
                );
            })}
        </SettingsSection>
    );
});

interface SettingsModelsTabProps {
    isActive?: boolean;
}

export const SettingsModelsTab = React.memo(function SettingsModelsTab({ isActive: _isActive = true }: SettingsModelsTabProps): React.JSX.Element {
    void _isActive;

    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const transcriptionConfig = useTranscriptionConfig();
    const updateConfig = useSetConfig();
    const {
        installedModels,
        modelCatalog,
        downloads,
        handleDelete,
        handleDownload,
        handleCancelDownload,
        restoreDefaultModelSettings
    } = useModelManagerContext();

    const streamingModelPath = modelConfig.streamingModelPath;
    const offlineModelPath = modelConfig.offlineModelPath;
    const speakerSegmentationModelPath = modelConfig.speakerSegmentationModelPath || '';
    const speakerEmbeddingModelPath = modelConfig.speakerEmbeddingModelPath || '';
    const vadBufferSize = transcriptionConfig.vadBufferSize || 5;
    const maxConcurrent = transcriptionConfig.maxConcurrent || 2;
    const enableITN = transcriptionConfig.enableITN ?? true;

    const modelById = useMemo(
        () => new Map(modelCatalog.models.map((model) => [model.id, model])),
        [modelCatalog.models],
    );
    const modelIdByInstallPath = useMemo(
        () => new Map(modelCatalog.models
            .filter((model) => model.installPath)
            .map((model) => [model.installPath, model.id])),
        [modelCatalog.models],
    );
    const sectionGroupsByType = useMemo(
        () => new Map(modelCatalog.sections.map((section) => [section.type, section.groups])),
        [modelCatalog.sections],
    );

    const getSectionGroups = useCallback(
        (type: ModelCatalogSectionType) => sectionGroupsByType.get(type) ?? [],
        [sectionGroupsByType],
    );

    const selectedStreamingModelId = useMemo(
        () => (streamingModelPath ? modelIdByInstallPath.get(streamingModelPath) || '' : ''),
        [modelIdByInstallPath, streamingModelPath],
    );
    const selectedOfflineModelId = useMemo(
        () => (offlineModelPath ? modelIdByInstallPath.get(offlineModelPath) || '' : ''),
        [modelIdByInstallPath, offlineModelPath],
    );
    const selectedSpeakerSegmentationModelId = useMemo(
        () => (speakerSegmentationModelPath ? modelIdByInstallPath.get(speakerSegmentationModelPath) || '' : ''),
        [modelIdByInstallPath, speakerSegmentationModelPath],
    );
    const selectedSpeakerEmbeddingModelId = useMemo(
        () => (speakerEmbeddingModelPath ? modelIdByInstallPath.get(speakerEmbeddingModelPath) || '' : ''),
        [modelIdByInstallPath, speakerEmbeddingModelPath],
    );

    const applyModelRules = async (modelId: string) => {
        try {
            const rules = modelService.getModelRules(modelId);

            if (rules.requiresVad) {
                const vadModelId = 'silero-vad';
                // Only sync and save if path is missing in config
                if (!modelConfig.vadModelPath) {
                    const vadModel = modelById.get(vadModelId);
                    if (vadModel?.isInstalled) {
                        updateConfig({ vadModelPath: vadModel.installPath });
                    } else {
                        document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: vadModelId } }));
                    }
                }
            }

            if (rules.requiresPunctuation) {
                const punctModelId = 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8';
                // Only sync and save if path is missing in config
                if (!modelConfig.punctuationModelPath) {
                    const punctuationModel = modelById.get(punctModelId);
                    if (punctuationModel?.isInstalled) {
                        updateConfig({ punctuationModelPath: punctuationModel.installPath });
                    } else {
                        document.dispatchEvent(new CustomEvent('download-background-model', { detail: { modelId: punctModelId } }));
                    }
                }
            }
        } catch (e) {
            logger.error('Failed to apply model rules', e);
        }
    };

    const handleModelChange = async (
        type: 'streaming' | 'offline' | 'speakerSegmentation' | 'speakerEmbedding',
        modelId: string,
    ) => {
        const configKey = type === 'streaming'
            ? 'streamingModelPath'
            : type === 'offline'
                ? 'offlineModelPath'
                : type === 'speakerSegmentation'
                    ? 'speakerSegmentationModelPath'
                    : 'speakerEmbeddingModelPath';

        if (!modelId) {
            updateConfig({ [configKey]: '' });
            return;
        }

        try {
            const path = modelById.get(modelId)?.installPath || await modelService.getModelPath(modelId);
            updateConfig({ [configKey]: path });
            await applyModelRules(modelId);
        } catch (e) {
            logger.error(`Failed to get ${type} model path`, e);
        }
    };

    const sectionProps = {
        installedModels,
        downloads,
        onDelete: handleDelete,
        onDownload: handleDownload,
        onCancelDownload: handleCancelDownload
    };

    const getModelLabel = useCallback((model: ModelInfo) => {
        let label = model.name;
        if (model.versionLabel) {
            label += ` (${model.versionLabel})`;
        }
        return label;
    }, []);

    const speakerDisabledOption = useMemo(() => ({
        value: '',
        label: t('settings.value_off', { defaultValue: 'Off' }),
    }), [t]);

    const streamingOptions = useMemo(() => {
        return modelCatalog.models
            .filter(m => m.modes?.includes('streaming'))
            .filter(m => installedModels.has(m.id) || m.id === selectedStreamingModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
    }, [getModelLabel, installedModels, modelCatalog.models, selectedStreamingModelId]);

    const offlineOptions = useMemo(() => {
        return modelCatalog.models
            .filter(m => m.modes?.includes('offline'))
            .filter(m => installedModels.has(m.id) || m.id === selectedOfflineModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
    }, [getModelLabel, installedModels, modelCatalog.models, selectedOfflineModelId]);

    const speakerSegmentationOptions = useMemo(() => {
        const installedOptions = modelCatalog.models
            .filter(m => m.type === 'speaker-segmentation')
            .filter(m => installedModels.has(m.id) || m.id === selectedSpeakerSegmentationModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
        return [speakerDisabledOption, ...installedOptions];
    }, [getModelLabel, installedModels, modelCatalog.models, selectedSpeakerSegmentationModelId, speakerDisabledOption]);

    const speakerEmbeddingOptions = useMemo(() => {
        const installedOptions = modelCatalog.models
            .filter(m => m.type === 'speaker-embedding')
            .filter(m => installedModels.has(m.id) || m.id === selectedSpeakerEmbeddingModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
        return [speakerDisabledOption, ...installedOptions];
    }, [getModelLabel, installedModels, modelCatalog.models, selectedSpeakerEmbeddingModelId, speakerDisabledOption]);

    return (
        <SettingsTabContainer id="settings-panel-models" ariaLabelledby="settings-tab-models">
            <SettingsPageHeader 
                icon={<ModelIcon width={28} height={28} />}
                title={t('settings.model_hub')} 
                description={t('settings.model_settings_description')} 
            />
            <SettingsSection
                title={t('settings.model_selection')}
                description={t('settings.model_selection_desc')}
                icon={<Settings2 size={20} />}
            >
                <SettingsItem
                    title={t('settings.streaming_model_label')}
                    hint={t('settings.streaming_model_hint')}
                >
                    <div style={{ width: '220px' }}>
                        <Dropdown
                            id="settings-streaming-path"
                            value={selectedStreamingModelId}
                            onChange={(value) => handleModelChange('streaming', value)}
                            placeholder={t('settings.select_streaming_model')}
                            options={streamingOptions}
                            style={{ flex: 1 }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.offline_model_label')}
                    hint={t('settings.offline_model_hint')}
                >
                    <div style={{ width: '220px' }}>
                        <Dropdown
                            id="settings-offline-path"
                            value={selectedOfflineModelId}
                            onChange={(value) => handleModelChange('offline', value)}
                            placeholder={t('settings.select_offline_model')}
                            options={offlineOptions}
                            style={{ flex: 1 }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.speaker_segmentation_model_label', { defaultValue: 'Speaker Segmentation Model' })}
                    hint={t('settings.speaker_segmentation_model_hint', { defaultValue: 'Used to split offline recordings into anonymous speaker turns.' })}
                >
                    <div style={{ width: '220px' }}>
                        <Dropdown
                            id="settings-speaker-segmentation-path"
                            value={selectedSpeakerSegmentationModelId}
                            onChange={(value) => handleModelChange('speakerSegmentation', value)}
                            placeholder={t('settings.select_speaker_segmentation_model', { defaultValue: 'Select speaker segmentation model' })}
                            options={speakerSegmentationOptions}
                            style={{ flex: 1 }}
                            aria-label={t('settings.speaker_segmentation_model_label', { defaultValue: 'Speaker Segmentation Model' })}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.speaker_embedding_model_label', { defaultValue: 'Speaker Embedding Model' })}
                    hint={t('settings.speaker_embedding_model_hint', { defaultValue: 'Used to match diarized speakers against your known speaker profiles.' })}
                >
                    <div style={{ width: '220px' }}>
                        <Dropdown
                            id="settings-speaker-embedding-path"
                            value={selectedSpeakerEmbeddingModelId}
                            onChange={(value) => handleModelChange('speakerEmbedding', value)}
                            placeholder={t('settings.select_speaker_embedding_model', { defaultValue: 'Select speaker embedding model' })}
                            options={speakerEmbeddingOptions}
                            style={{ flex: 1 }}
                            aria-label={t('settings.speaker_embedding_model_label', { defaultValue: 'Speaker Embedding Model' })}
                        />
                    </div>
                </SettingsItem>
            </SettingsSection>

            <ModelSection 
                title={t('settings.recognition_models')} 
                groups={getSectionGroups('asr')}
                icon={<Mic size={20} />}
                {...sectionProps} 
            />

            <ModelSection
                title={t('settings.punctuation_models')}
                groups={getSectionGroups('punctuation')}
                icon={<Type size={20} />}
                {...sectionProps} 
            />
            
            <ModelSection
                title={t('settings.vad_models')}
                groups={getSectionGroups('vad')}
                icon={<Activity size={20} />}
                {...sectionProps} 
            />

            <ModelSection
                title={t('settings.speaker_segmentation_models', { defaultValue: 'Speaker Segmentation Models' })}
                groups={getSectionGroups('speaker-segmentation')}
                icon={<Mic size={20} />}
                {...sectionProps}
            />

            <ModelSection
                title={t('settings.speaker_embedding_models', { defaultValue: 'Speaker Embedding Models' })}
                groups={getSectionGroups('speaker-embedding')}
                icon={<Mic size={20} />}
                {...sectionProps}
            />

            <SettingsSection
                title={t('settings.transcription_settings')}
                icon={<PlaySquare size={20} />}
                description={t('settings.transcription_settings_hint')}
            >
                <SettingsItem
                    title={t('settings.enable_itn')}
                    hint={t('settings.enable_itn_hint')}
                >
                    <Switch
                        checked={enableITN}
                        onChange={(checked) => updateConfig({ enableITN: checked })}
                    />
                </SettingsItem>

                <SettingsItem
                    title={t('settings.vad_buffer_size')}
                    hint={t('settings.vad_buffer_hint')}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-vad-buffer"
                            type="number"
                            className="settings-input"
                            value={vadBufferSize}
                            onChange={(e) => updateConfig({ vadBufferSize: Number(e.target.value) })}
                            min={0}
                            max={30}
                            step={0.5}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

                <SettingsItem
                    title={t('settings.max_concurrent_label')}
                    hint={t('settings.max_concurrent_hint')}
                >
                    <div style={{ width: '120px' }}>
                        <input
                            id="settings-max-concurrent"
                            type="number"
                            className="settings-input"
                            value={maxConcurrent}
                            onChange={(e) => {
                                const val = Number(e.target.value);
                                if (val > 0) {
                                    updateConfig({ maxConcurrent: val });
                                }
                            }}
                            min={1}
                            max={4}
                            step={1}
                            style={{ textAlign: 'center' }}
                        />
                    </div>
                </SettingsItem>

            </SettingsSection>

            <div style={{ display: 'flex', justifyContent: 'flex-start', marginTop: '8px' }}>
                <button
                    className="btn btn-restore-defaults"
                    onClick={restoreDefaultModelSettings}
                    aria-label={t('settings.restore_defaults')}
                >
                    <RestoreIcon />
                    {t('settings.restore_defaults')}
                </button>
            </div>
        </SettingsTabContainer>
    );
});
