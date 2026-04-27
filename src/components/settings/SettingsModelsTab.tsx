import React, { useEffect, useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
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
    type: 'asr' | 'punctuation' | 'vad' | 'speaker-segmentation' | 'speaker-embedding';
    installedModels: Set<string>;
    downloads: Record<string, { progress: number; status: string }>;
    onDelete: (model: ModelInfo) => void;
    onDownload: (model: ModelInfo) => void;
    onCancelDownload: (modelId: string) => void;
}

function ModelSection({
    title,
    description,
    icon,
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
                const blacklist = ['vad', 'punctuation', 'itn', 'speaker-segmentation', 'speaker-embedding'];
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
        <SettingsSection title={title} description={description} icon={icon}>
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
        </SettingsSection>
    );
}

export function SettingsModelsTab(): React.JSX.Element {
    const { t } = useTranslation();
    const modelConfig = useModelConfig();
    const transcriptionConfig = useTranscriptionConfig();
    const updateConfig = useSetConfig();
    const {
        installedModels,
        downloads,
        handleDelete,
        handleDownload,
        handleCancelDownload,
        restoreDefaultModelSettings
    } = useModelManagerContext();

    const [selectedStreamingModelId, setSelectedStreamingModelId] = useState<string>('');
    const [selectedOfflineModelId, setSelectedOfflineModelId] = useState<string>('');
    const [selectedSpeakerSegmentationModelId, setSelectedSpeakerSegmentationModelId] = useState<string>('');
    const [selectedSpeakerEmbeddingModelId, setSelectedSpeakerEmbeddingModelId] = useState<string>('');

    const streamingModelPath = modelConfig.streamingModelPath;
    const offlineModelPath = modelConfig.offlineModelPath;
    const speakerSegmentationModelPath = modelConfig.speakerSegmentationModelPath || '';
    const speakerEmbeddingModelPath = modelConfig.speakerEmbeddingModelPath || '';
    const vadBufferSize = transcriptionConfig.vadBufferSize || 5;
    const maxConcurrent = transcriptionConfig.maxConcurrent || 2;
    const enableITN = transcriptionConfig.enableITN ?? true;

    // Memoize the mapping between paths and model IDs in state to trigger re-renders
    const [pathMap, setPathMap] = useState<Map<string, string>>(new Map());

    // Initialize mapping of paths to model IDs efficiently
    useEffect(() => {
        const initPathMap = async () => {
            const map = new Map<string, string>();

            // Resolve all model paths concurrently to avoid sequential IPC/FS delays
            await Promise.all(
                PRESET_MODELS.map(async (model) => {
                    if (
                        model.modes?.includes('streaming')
                        || model.modes?.includes('offline')
                        || model.type === 'speaker-segmentation'
                        || model.type === 'speaker-embedding'
                    ) {
                        try {
                            const path = await modelService.getModelPath(model.id);
                            map.set(path, model.id);
                        } catch (e) {
                            logger.error(`Failed to resolve path for model ${model.id}`, e);
                        }
                    }
                })
            );

            setPathMap(map);
        };

        initPathMap();
    }, []); // Only run once on mount

    // Sync selected streaming model when the path config or the map changes
    useEffect(() => {
        if (!streamingModelPath) {
            setSelectedStreamingModelId('');
            return;
        }

        if (pathMap.size > 0) {
            setSelectedStreamingModelId(pathMap.get(streamingModelPath) || '');
        }
    }, [streamingModelPath, pathMap]);

    // Sync selected offline model when the path config or the map changes
    useEffect(() => {
        if (!offlineModelPath) {
            setSelectedOfflineModelId('');
            return;
        }

        if (pathMap.size > 0) {
            setSelectedOfflineModelId(pathMap.get(offlineModelPath) || '');
        }
    }, [offlineModelPath, pathMap]);

    useEffect(() => {
        if (!speakerSegmentationModelPath) {
            setSelectedSpeakerSegmentationModelId('');
            return;
        }

        if (pathMap.size > 0) {
            setSelectedSpeakerSegmentationModelId(pathMap.get(speakerSegmentationModelPath) || '');
        }
    }, [speakerSegmentationModelPath, pathMap]);

    useEffect(() => {
        if (!speakerEmbeddingModelPath) {
            setSelectedSpeakerEmbeddingModelId('');
            return;
        }

        if (pathMap.size > 0) {
            setSelectedSpeakerEmbeddingModelId(pathMap.get(speakerEmbeddingModelPath) || '');
        }
    }, [speakerEmbeddingModelPath, pathMap]);

    const applyModelRules = async (modelId: string) => {
        try {
            const rules = modelService.getModelRules(modelId);

            if (rules.requiresVad) {
                const vadModelId = 'silero-vad';
                // Only sync and save if path is missing in config
                if (!modelConfig.vadModelPath) {
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
                if (!modelConfig.punctuationModelPath) {
                    if (installedModels.has(punctModelId)) {
                        const punctPath = await modelService.getModelPath(punctModelId);
                        updateConfig({ punctuationModelPath: punctPath });
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
        if (type === 'streaming') {
            setSelectedStreamingModelId(modelId);
        } else if (type === 'offline') {
            setSelectedOfflineModelId(modelId);
        } else if (type === 'speakerSegmentation') {
            setSelectedSpeakerSegmentationModelId(modelId);
        } else {
            setSelectedSpeakerEmbeddingModelId(modelId);
        }

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
            const path = await modelService.getModelPath(modelId);
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
        return PRESET_MODELS
            .filter(m => m.modes?.includes('streaming'))
            .filter(m => installedModels.has(m.id) || m.id === selectedStreamingModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
    }, [getModelLabel, installedModels, selectedStreamingModelId]);

    const offlineOptions = useMemo(() => {
        return PRESET_MODELS
            .filter(m => m.modes?.includes('offline'))
            .filter(m => installedModels.has(m.id) || m.id === selectedOfflineModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
    }, [getModelLabel, installedModels, selectedOfflineModelId]);

    const speakerSegmentationOptions = useMemo(() => {
        const installedOptions = PRESET_MODELS
            .filter(m => m.type === 'speaker-segmentation')
            .filter(m => installedModels.has(m.id) || m.id === selectedSpeakerSegmentationModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
        return [speakerDisabledOption, ...installedOptions];
    }, [getModelLabel, installedModels, selectedSpeakerSegmentationModelId, speakerDisabledOption]);

    const speakerEmbeddingOptions = useMemo(() => {
        const installedOptions = PRESET_MODELS
            .filter(m => m.type === 'speaker-embedding')
            .filter(m => installedModels.has(m.id) || m.id === selectedSpeakerEmbeddingModelId)
            .map(model => ({
                value: model.id,
                label: getModelLabel(model)
            }));
        return [speakerDisabledOption, ...installedOptions];
    }, [getModelLabel, installedModels, selectedSpeakerEmbeddingModelId, speakerDisabledOption]);

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
                type="asr" 
                icon={<Mic size={20} />}
                {...sectionProps} 
            />

            <ModelSection
                title={t('settings.punctuation_models')}
                type="punctuation"
                icon={<Type size={20} />}
                {...sectionProps} 
            />
            
            <ModelSection
                title={t('settings.vad_models')}
                type="vad"
                icon={<Activity size={20} />}
                {...sectionProps} 
            />

            <ModelSection
                title={t('settings.speaker_segmentation_models', { defaultValue: 'Speaker Segmentation Models' })}
                type="speaker-segmentation"
                icon={<Mic size={20} />}
                {...sectionProps}
            />

            <ModelSection
                title={t('settings.speaker_embedding_models', { defaultValue: 'Speaker Embedding Models' })}
                type="speaker-embedding"
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
}
