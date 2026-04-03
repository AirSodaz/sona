import React, { useEffect, useState, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { PRESET_MODELS, modelService, ModelInfo } from '../../services/modelService';
import { ModelCard } from './ModelCard';
import { Dropdown } from '../Dropdown';
import { AppConfig } from '../../types/transcript';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { Layers, Mic, Type, Activity } from 'lucide-react';

interface ModelSectionProps {
    title: string;
    description?: string;
    icon?: React.ReactNode;
    type: 'asr' | 'punctuation' | 'vad';
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

    // Memoize the mapping between paths and model IDs in state to trigger re-renders
    const [pathMap, setPathMap] = useState<Map<string, string>>(new Map());

    // Initialize mapping of paths to model IDs efficiently
    useEffect(() => {
        const initPathMap = async () => {
            const map = new Map<string, string>();

            // Resolve all model paths concurrently to avoid sequential IPC/FS delays
            await Promise.all(
                PRESET_MODELS.map(async (model) => {
                    if (model.modes?.includes('streaming') || model.modes?.includes('offline')) {
                        try {
                            const path = await modelService.getModelPath(model.id);
                            map.set(path, model.id);
                        } catch (e) {
                            console.error(`Failed to resolve path for model ${model.id}`, e);
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

    const handleModelChange = async (type: 'streaming' | 'offline', modelId: string) => {
        if (type === 'streaming') {
            setSelectedStreamingModelId(modelId);
        } else {
            setSelectedOfflineModelId(modelId);
        }

        const configKey = type === 'streaming' ? 'streamingModelPath' : 'offlineModelPath';

        if (!modelId) {
            updateConfig({ [configKey]: '' });
            return;
        }

        try {
            const path = await modelService.getModelPath(modelId);
            updateConfig({ [configKey]: path });
            await applyModelRules(modelId);
        } catch (e) {
            console.error(`Failed to get ${type} model path`, e);
        }
    };

    const sectionProps = {
        installedModels,
        downloads,
        onDelete,
        onDownload,
        onCancelDownload
    };

    const getModelLabel = useCallback((model: ModelInfo) => {
        let label = model.name;
        if (model.versionLabel) {
            label += ` (${model.versionLabel})`;
        }
        if (!installedModels.has(model.id)) {
            label += t('settings.not_installed');
        }
        return label;
    }, [installedModels, t]);

    const streamingOptions = useMemo(() => {
        return PRESET_MODELS.filter(m => m.modes?.includes('streaming')).map(model => ({
            value: model.id,
            label: getModelLabel(model),
            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } as React.CSSProperties : undefined
        }));
    }, [getModelLabel, installedModels]);

    const offlineOptions = useMemo(() => {
        return PRESET_MODELS.filter(m => m.modes?.includes('offline')).map(model => ({
            value: model.id,
            label: getModelLabel(model),
            style: !installedModels.has(model.id) ? { color: 'var(--color-text-muted)', cursor: 'not-allowed', pointerEvents: 'none' } as React.CSSProperties : undefined
        }));
    }, [getModelLabel, installedModels]);

    return (
        <SettingsTabContainer id="settings-panel-models" ariaLabelledby="settings-tab-models">
            <SettingsPageHeader 
                icon={<Layers size={28} />} 
                title={t('settings.model_hub')} 
                description={t('settings.model_selection_desc')} 
            />
            <SettingsSection
                title={t('settings.model_selection')}
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
        </SettingsTabContainer>
    );
}
