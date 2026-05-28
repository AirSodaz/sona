import React, { useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type {
    ModelCatalogSectionType,
    ModelSelectionOption,
} from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { useModelConfig, useSetConfig, useTranscriptionConfig } from '../../stores/configStore';
import {
    GROQ_WHISPER_PROVIDER_ID,
    ONLINE_ASR_PROVIDER_DEFINITIONS,
    VOLCENGINE_DOUBAO_PROVIDER_ID,
    syncOnlineAsrSelectionFields,
    syncStreamingOnlineAsrSelectionFields,
    syncLegacyAsrSelectionFields,
    syncStreamingAsrSelectionFields,
} from '../../services/asrConfigService';
import { isOnlineAsrProviderId } from '../../services/onlineAsrProviders';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader, SettingsAccordion } from './SettingsLayout';
import { Settings2, PlaySquare } from 'lucide-react';
import { ModelIcon, RestoreIcon, CloudIcon } from '../Icons';
import { useModelManagerContext } from '../../hooks/useModelManager';
import { Switch } from '../Switch';
import { CUSTOM_PROVIDER_COMPONENTS, DynamicProviderSettings } from './OnlineAsrSettingsCards';

const onlineAsrProvider = ONLINE_ASR_PROVIDER_DEFINITIONS[0];
const VOLCENGINE_DOUBAO_OPTION_ID = onlineAsrProvider.id;

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
    const batchVadEnabled = transcriptionConfig.batchVadEnabled ?? true;

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
        () => modelConfig.asr?.selections.live.engine === 'online'
            ? (modelConfig.asr.selections.live.providerId ?? VOLCENGINE_DOUBAO_OPTION_ID)
            : selectedModelIds.streaming ?? '',
        [modelConfig.asr?.selections.live, selectedModelIds.streaming],
    );
    const selectedOfflineModelId = useMemo(
        () => modelConfig.asr?.selections.batch.engine === 'online'
            ? (modelConfig.asr.selections.batch.providerId ?? VOLCENGINE_DOUBAO_OPTION_ID)
            : selectedModelIds.offline ?? '',
        [modelConfig.asr?.selections.batch, selectedModelIds.offline],
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

        if (isOnlineAsrProviderId(modelId)) {
            if (type === 'streaming') {
                updateConfig(syncStreamingOnlineAsrSelectionFields(modelConfig, modelId));
            } else if (type === 'offline') {
                updateConfig(syncOnlineAsrSelectionFields(modelConfig, 'batch', modelId));
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
            ...ONLINE_ASR_PROVIDER_DEFINITIONS
                .filter(provider => provider.id !== GROQ_WHISPER_PROVIDER_ID && provider.defaultConfig) // Groq doesn't support streaming. In future, we can check provider.streaming?.supported !== false. Wait! The definition might not have streaming field directly. Let's just filter groq-whisper directly here to be safe and clean since there's no full manifest typed.
                .filter(provider => {
                    if (provider.id === selectedStreamingModelId) return true;
                    const providerConfig = modelConfig.asr?.providers?.online?.[provider.id]
                        ?? (provider.id === VOLCENGINE_DOUBAO_PROVIDER_ID ? modelConfig.asr?.providers?.volcengineDoubao : undefined)
                        ?? provider.defaultConfig;
                    return provider.isConfigured(providerConfig as typeof provider.defaultConfig, 'streaming');
                })
                .map((provider) => ({
                    value: provider.id,
                    label: (
                        <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            {t(provider.optionLabelKey, { defaultValue: provider.optionDefaultLabel })}
                            <CloudIcon style={{ color: 'var(--color-text-muted)' }} />
                        </span>
                    ),
                })),
        ];
    }, [selectedStreamingModelId, selectionOptions.streaming, t, modelConfig.asr?.providers]);

    const offlineOptions = useMemo(() => {
        return [
            ...toDropdownOptions(selectionOptions.offline, selectedOfflineModelId),
            ...ONLINE_ASR_PROVIDER_DEFINITIONS
                .filter(provider => {
                    if (provider.id === selectedOfflineModelId) return true;
                    const providerConfig = modelConfig.asr?.providers?.online?.[provider.id]
                        ?? (provider.id === VOLCENGINE_DOUBAO_PROVIDER_ID ? modelConfig.asr?.providers?.volcengineDoubao : undefined)
                        ?? provider.defaultConfig;
                    return provider.isConfigured(providerConfig as typeof provider.defaultConfig, 'offline');
                })
                .map((provider) => ({
                value: provider.id,
                label: (
                    <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        {t(provider.optionLabelKey, { defaultValue: provider.optionDefaultLabel })}
                        <CloudIcon style={{ color: 'var(--color-text-muted)' }} />
                    </span>
                ),
            })),
        ];
    }, [selectedOfflineModelId, selectionOptions.offline, t, modelConfig.asr?.providers]);

    const isVolcengineSelected = Object.values(modelConfig.asr?.selections ?? {}).some(
        (selection) => selection.engine === 'online' && selection.providerId === VOLCENGINE_DOUBAO_PROVIDER_ID,
    );

    const getSectionStatus = (type: ModelCatalogSectionType) => {
        const groups = getSectionGroups(type);
        const allModels = groups.flatMap(group => group.models);

        const downloadingModel = allModels.find(model => !!downloads[model.id]);
        if (downloadingModel) {
            const progress = downloads[downloadingModel.id].progress;
            return {
                type: 'pending',
                text: t('settings.downloading_progress', { progress: Math.round(progress), defaultValue: `正在下载 (${Math.round(progress)}%)` })
            };
        }

        const installedCount = allModels.filter(model => installedModels.has(model.id)).length;
        if (installedCount > 0) {
            return {
                type: 'ready',
                text: type === 'vad' || type === 'punctuation'
                    ? t('settings.ready', { defaultValue: '已就绪' })
                    : t('settings.installed_count', { count: installedCount, defaultValue: `已安装 ${installedCount} 个` })
            };
        }

        return {
            type: 'off',
            text: t('settings.not_installed', { defaultValue: '未安装' })
        };
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
                        {t(onlineAsrProvider.cloudUploadHintKey, { defaultValue: onlineAsrProvider.cloudUploadHintDefault })}
                    </div>
                )}
            </SettingsSection>

            <SettingsSection
                title={t('settings.offline_model_management', { defaultValue: '离线模型管理' })}
                icon={<RestoreIcon />}
            >
                <SettingsAccordion
                    title={t('settings.recognition_models')}
                    status={<span className={`status-badge ${getSectionStatus('asr').type}`}>{getSectionStatus('asr').text}</span>}
                    defaultOpen={true}
                >
                    {getSectionGroups('asr').map(group => (
                        <ModelCard
                            key={group.key}
                            models={group.models}
                            {...sectionProps}
                        />
                    ))}
                </SettingsAccordion>

                <SettingsAccordion
                    title={t('settings.punctuation_models')}
                    status={<span className={`status-badge ${getSectionStatus('punctuation').type}`}>{getSectionStatus('punctuation').text}</span>}
                >
                    {getSectionGroups('punctuation').map(group => (
                        <ModelCard
                            key={group.key}
                            models={group.models}
                            {...sectionProps}
                        />
                    ))}
                </SettingsAccordion>

                <SettingsAccordion
                    title={t('settings.vad_models')}
                    status={<span className={`status-badge ${getSectionStatus('vad').type}`}>{getSectionStatus('vad').text}</span>}
                >
                    {getSectionGroups('vad').map(group => (
                        <ModelCard
                            key={group.key}
                            models={group.models}
                            {...sectionProps}
                        />
                    ))}
                </SettingsAccordion>

                <SettingsAccordion
                    title={t('settings.speaker_segmentation_models', { defaultValue: 'Speaker Segmentation Models' })}
                    status={<span className={`status-badge ${getSectionStatus('speaker-segmentation').type}`}>{getSectionStatus('speaker-segmentation').text}</span>}
                >
                    {getSectionGroups('speaker-segmentation').map(group => (
                        <ModelCard
                            key={group.key}
                            models={group.models}
                            {...sectionProps}
                        />
                    ))}
                </SettingsAccordion>

                <SettingsAccordion
                    title={t('settings.speaker_embedding_models', { defaultValue: 'Speaker Embedding Models' })}
                    status={<span className={`status-badge ${getSectionStatus('speaker-embedding').type}`}>{getSectionStatus('speaker-embedding').text}</span>}
                >
                    {getSectionGroups('speaker-embedding').map(group => (
                        <ModelCard
                            key={group.key}
                            models={group.models}
                            {...sectionProps}
                        />
                    ))}
                </SettingsAccordion>
            </SettingsSection>

            <SettingsSection
                title={t('settings.online_model_management', { defaultValue: '在线模型管理' })}
                icon={<Settings2 size={20} />}
            >
                {ONLINE_ASR_PROVIDER_DEFINITIONS.map(provider => {
                    const Component = CUSTOM_PROVIDER_COMPONENTS[provider.id] || DynamicProviderSettings;
                    return <Component key={provider.id} provider={provider} />;
                })}
            </SettingsSection>

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
                    title={t('settings.batch_vad_enabled')}
                    hint={t('settings.batch_vad_enabled_hint')}
                >
                    <Switch
                        checked={batchVadEnabled}
                        onChange={(checked) => updateConfig({ batchVadEnabled: checked })}
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
