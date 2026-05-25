import React, { useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type {
    ModelCatalogGroup,
    ModelCatalogSectionType,
    ModelInfo,
    ModelSelectionOption,
} from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { useModelConfig, useSetConfig, useTranscriptionConfig } from '../../stores/configStore';
import {
    DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
    syncStreamingVolcengineDoubaoSelectionFields,
    syncLegacyAsrSelectionFields,
    syncStreamingAsrSelectionFields,
    syncVolcengineDoubaoProviderConfig,
    syncVolcengineDoubaoSelectionFields,
} from '../../services/asrConfigService';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { Mic, Type, Activity, Settings2, PlaySquare } from 'lucide-react';
import { ModelIcon, RestoreIcon } from '../Icons';
import { useModelManagerContext } from '../../hooks/useModelManager';
import { Switch } from '../Switch';

const VOLCENGINE_DOUBAO_OPTION_ID = 'volcengine-doubao';

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

function toDropdownOptions(
    options: ModelSelectionOption[],
    selectedId: string,
): Array<{ value: string; label: string }> {
    return options
        .filter((option) => option.isInstalled || option.id === selectedId)
        .map((option) => ({
            value: option.id,
            label: option.label,
        }));
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
        selectedModelIds,
        downloads,
        handleDelete,
        handleDownload,
        handleCancelDownload,
        restoreDefaultModelSettings
    } = useModelManagerContext();

    const vadBufferSize = transcriptionConfig.vadBufferSize || 5;
    const maxConcurrent = transcriptionConfig.maxConcurrent || 2;
    const enableITN = transcriptionConfig.enableITN ?? true;

    const sectionGroupsByType = useMemo(
        () => new Map(modelCatalog.sections.map((section) => [section.type, section.groups])),
        [modelCatalog.sections],
    );
    const selectionOptions = modelCatalog.selectionOptions;

    const getSectionGroups = useCallback(
        (type: ModelCatalogSectionType) => sectionGroupsByType.get(type) ?? [],
        [sectionGroupsByType],
    );

    const selectedStreamingModelId = useMemo(
        () => modelConfig.asr?.selections.live.engine === 'volcengine-doubao'
            ? VOLCENGINE_DOUBAO_OPTION_ID
            : selectedModelIds.streaming ?? '',
        [modelConfig.asr?.selections.live.engine, selectedModelIds.streaming],
    );
    const selectedOfflineModelId = useMemo(
        () => modelConfig.asr?.selections.batch.engine === 'volcengine-doubao'
            ? VOLCENGINE_DOUBAO_OPTION_ID
            : selectedModelIds.offline ?? '',
        [modelConfig.asr?.selections.batch.engine, selectedModelIds.offline],
    );
    const selectedSpeakerSegmentationModelId = useMemo(
        () => selectedModelIds.speakerSegmentation ?? '',
        [selectedModelIds.speakerSegmentation],
    );
    const selectedSpeakerEmbeddingModelId = useMemo(
        () => selectedModelIds.speakerEmbedding ?? '',
        [selectedModelIds.speakerEmbedding],
    );

    const applyDependencyRequests = (modelId: string) => {
        const dependencyUpdates: Partial<typeof modelConfig> = {};
        const dependencies = modelCatalog.dependencyRequestsByModelId[modelId] ?? [];
        for (const dependency of dependencies) {
            const currentPath = dependency.configKey === 'vadModelPath'
                ? modelConfig.vadModelPath
                : modelConfig.punctuationModelPath;
            if (currentPath) {
                continue;
            }
            if (dependency.isInstalled) {
                dependencyUpdates[dependency.configKey] = dependency.installPath;
            } else {
                document.dispatchEvent(new CustomEvent('download-background-model', {
                    detail: { modelId: dependency.modelId },
                }));
            }
        }

        if (Object.keys(dependencyUpdates).length > 0) {
            updateConfig(dependencyUpdates);
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
            if (type === 'streaming') {
                const patch = syncStreamingAsrSelectionFields(modelConfig, {
                    modelId: null,
                    modelPath: '',
                });
                updateConfig(patch);
                return;
            }
            if (type === 'offline') {
                updateConfig(syncLegacyAsrSelectionFields(modelConfig, 'batch', {
                    modelId: null,
                    modelPath: '',
                }));
                return;
            }
            updateConfig({ [configKey]: '' });
            return;
        }

        if (modelId === VOLCENGINE_DOUBAO_OPTION_ID) {
            if (type === 'streaming') {
                updateConfig(syncStreamingVolcengineDoubaoSelectionFields(modelConfig));
            } else if (type === 'offline') {
                updateConfig(syncVolcengineDoubaoSelectionFields(modelConfig, 'batch'));
            }
            return;
        }

        const path = modelCatalog.modelPathById[modelId]
            || modelCatalog.models.find((model) => model.id === modelId)?.installPath
            || '';
        if (!path) {
            return;
        }
        if (type === 'streaming') {
            const patch = syncStreamingAsrSelectionFields(modelConfig, {
                modelId,
                modelPath: path,
            });
            updateConfig(patch);
        } else if (type === 'offline') {
            updateConfig(syncLegacyAsrSelectionFields(modelConfig, 'batch', {
                modelId,
                modelPath: path,
            }));
        } else {
            updateConfig({ [configKey]: path });
        }
        applyDependencyRequests(modelId);
    };

    const sectionProps = {
        installedModels,
        downloads,
        onDelete: handleDelete,
        onDownload: handleDownload,
        onCancelDownload: handleCancelDownload
    };

    const speakerDisabledOption = useMemo(() => ({
        value: '',
        label: t('settings.value_off', { defaultValue: 'Off' }),
    }), [t]);

    const streamingOptions = useMemo(() => {
        return [
            ...toDropdownOptions(selectionOptions.streaming, selectedStreamingModelId),
            {
                value: VOLCENGINE_DOUBAO_OPTION_ID,
                label: t('settings.asr.volcengine_doubao_option', { defaultValue: '豆包语音 (云端)' }),
            },
        ];
    }, [selectedStreamingModelId, selectionOptions.streaming, t]);

    const offlineOptions = useMemo(() => {
        return [
            ...toDropdownOptions(selectionOptions.offline, selectedOfflineModelId),
            {
                value: VOLCENGINE_DOUBAO_OPTION_ID,
                label: t('settings.asr.volcengine_doubao_option', { defaultValue: '豆包语音 (云端)' }),
            },
        ];
    }, [selectedOfflineModelId, selectionOptions.offline, t]);

    const volcengineConfig = modelConfig.asr?.providers?.volcengineDoubao
        ?? DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG;
    const isVolcengineSelected = modelConfig.asr?.selections.live.engine === 'volcengine-doubao'
        || modelConfig.asr?.selections.caption.engine === 'volcengine-doubao'
        || modelConfig.asr?.selections.voiceTyping.engine === 'volcengine-doubao'
        || modelConfig.asr?.selections.batch.engine === 'volcengine-doubao';
    const updateVolcengineConfig = (updates: Partial<typeof volcengineConfig>) => {
        updateConfig(syncVolcengineDoubaoProviderConfig(modelConfig, updates));
    };

    const speakerSegmentationOptions = useMemo(() => {
        const installedOptions = toDropdownOptions(
            selectionOptions.speakerSegmentation,
            selectedSpeakerSegmentationModelId,
        );
        return [speakerDisabledOption, ...installedOptions];
    }, [selectedSpeakerSegmentationModelId, selectionOptions.speakerSegmentation, speakerDisabledOption]);

    const speakerEmbeddingOptions = useMemo(() => {
        const installedOptions = toDropdownOptions(
            selectionOptions.speakerEmbedding,
            selectedSpeakerEmbeddingModelId,
        );
        return [speakerDisabledOption, ...installedOptions];
    }, [selectedSpeakerEmbeddingModelId, selectionOptions.speakerEmbedding, speakerDisabledOption]);

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

                {isVolcengineSelected && (
                    <div className="settings-hint">
                        {t('settings.asr.cloud_upload_hint', { defaultValue: '音频会发送到火山引擎进行识别。' })}
                    </div>
                )}
            </SettingsSection>

            <SettingsSection
                title={t('settings.asr.volcengine_credentials_title', { defaultValue: '火山 ASR 凭据' })}
                description={t('settings.asr.volcengine_credentials_hint', {
                    defaultValue: '仅在对应 ASR 槽位选择豆包语音时使用，API Key 保存在本机配置中。',
                })}
                icon={<Settings2 size={20} />}
            >
                <SettingsItem
                    title={t('settings.asr.api_key', { defaultValue: 'API Key' })}
                    hint={t('settings.asr.api_key_hint', { defaultValue: '新版控制台的 X-Api-Key；不会写入日志。' })}
                >
                    <div style={{ width: '320px' }}>
                        <input
                            id="settings-volcengine-api-key"
                            type="password"
                            className="settings-input"
                            value={volcengineConfig.apiKey}
                            onChange={(event) => updateVolcengineConfig({ apiKey: event.target.value })}
                            placeholder="X-Api-Key"
                        />
                    </div>
                </SettingsItem>
                <SettingsItem
                    title={t('settings.asr.streaming_resource_id', { defaultValue: '实时 Resource ID' })}
                    hint={t('settings.asr.streaming_resource_id_hint', { defaultValue: '默认使用豆包流式语音识别模型 2.0 小时版。' })}
                >
                    <div style={{ width: '320px' }}>
                        <input
                            id="settings-volcengine-streaming-resource"
                            type="text"
                            className="settings-input"
                            value={volcengineConfig.streamingResourceId}
                            onChange={(event) => updateVolcengineConfig({ streamingResourceId: event.target.value })}
                            placeholder={DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingResourceId}
                        />
                    </div>
                </SettingsItem>
                <SettingsItem
                    title={t('settings.asr.batch_resource_id', { defaultValue: '批量 Resource ID' })}
                    hint={t('settings.asr.batch_resource_id_hint', { defaultValue: '极速版固定默认值为 volc.bigasr.auc_turbo。' })}
                >
                    <div style={{ width: '320px' }}>
                        <input
                            id="settings-volcengine-batch-resource"
                            type="text"
                            className="settings-input"
                            value={volcengineConfig.batchResourceId}
                            onChange={(event) => updateVolcengineConfig({ batchResourceId: event.target.value })}
                            placeholder={DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.batchResourceId}
                        />
                    </div>
                </SettingsItem>
                <SettingsItem
                    title={t('settings.asr.streaming_endpoint', { defaultValue: '实时 Endpoint' })}
                    hint={t('settings.asr.streaming_endpoint_hint', { defaultValue: '高级设置：默认使用双向流式优化版。' })}
                >
                    <div style={{ width: '420px' }}>
                        <input
                            id="settings-volcengine-streaming-endpoint"
                            type="text"
                            className="settings-input"
                            value={volcengineConfig.streamingEndpoint}
                            onChange={(event) => updateVolcengineConfig({ streamingEndpoint: event.target.value })}
                            placeholder={DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.streamingEndpoint}
                        />
                    </div>
                </SettingsItem>
                <SettingsItem
                    title={t('settings.asr.batch_endpoint', { defaultValue: '批量 Endpoint' })}
                    hint={t('settings.asr.batch_endpoint_hint', { defaultValue: '高级设置：极速版同步识别接口。' })}
                >
                    <div style={{ width: '420px' }}>
                        <input
                            id="settings-volcengine-batch-endpoint"
                            type="text"
                            className="settings-input"
                            value={volcengineConfig.batchEndpoint}
                            onChange={(event) => updateVolcengineConfig({ batchEndpoint: event.target.value })}
                            placeholder={DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG.batchEndpoint}
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
