import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Check, List, Loader2, X } from 'lucide-react';
import { Dropdown } from '../Dropdown';
import { AppConfig, LlmProvider, LlmProviderSetting } from '../../types/transcript';
import { normalizeError } from '../../utils/errorUtils';
import {
    buildLlmConfigPatch,
    ensureLlmState,
    getActiveLlmConfig,
    getActiveProvider,
    getActiveProviderSetting,
    getProviderDefinition,
    isLlmConfigComplete,
    LLM_PROVIDER_DEFINITIONS,
    updateProviderSetting,
} from '../../services/llmConfig';

interface SettingsLLMServiceTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    changeLlmServiceType: (type: LlmProvider) => void;
}

const MANUAL_MODEL_VALUE = '__manual__';

function getModelPlaceholder(provider: LlmProvider): string {
    switch (provider) {
        case 'azure_openai':
            return 'gpt-4o-deployment';
        case 'anthropic':
            return 'claude-sonnet-4-20250514';
        case 'gemini':
            return 'gemini-2.5-flash';
        case 'ollama':
            return 'qwen3:8b';
        case 'deep_seek':
            return 'deepseek-chat';
        case 'kimi':
            return 'moonshot-v1-8k';
        case 'qwen':
        case 'qwen_portal':
            return 'qwen-max';
        case 'groq':
            return 'llama-3.3-70b-versatile';
        case 'x_ai':
            return 'grok-3-mini';
        case 'mistral_ai':
            return 'mistral-large-latest';
        case 'perplexity':
            return 'sonar';
        default:
            return 'gpt-4.1-mini';
    }
}

export function SettingsLLMServiceTab({
    config,
    updateConfig,
    changeLlmServiceType,
}: SettingsLLMServiceTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');
    const [availableModels, setAvailableModels] = useState<string[]>([]);
    const [isManualEntry, setIsManualEntry] = useState(false);
    const [isLoadingModels, setIsLoadingModels] = useState(false);
    const debounceTimeout = useRef<NodeJS.Timeout | null>(null);

    const activeProvider = getActiveProvider(config);
    const providerDefinition = getProviderDefinition(activeProvider);
    const providerSetting = getActiveProviderSetting(config);
    const llm = getActiveLlmConfig(config);

    const llmBaseUrl = llm.baseUrl || providerDefinition.defaultApiHost;
    const llmApiKey = llm.apiKey || '';
    const llmModel = llm.model || '';
    const llmTemperature = llm.temperature ?? 0.7;
    const llmApiPath = llm.apiPath || providerDefinition.defaultApiPath || '';
    const llmApiVersion = llm.apiVersion || providerDefinition.defaultApiVersion || '';

    const providerOptions = LLM_PROVIDER_DEFINITIONS.map((provider) => ({
        value: provider.id,
        label: provider.label,
    }));

    const applyProviderSettingUpdates = (updates: Partial<LlmProviderSetting>) => {
        const currentLlmState = config.llmSettings ? { llmSettings: config.llmSettings, llm: config.llm } : ensureLlmState(config as AppConfig & Record<string, any>);
        const nextLlmSettings = updateProviderSetting(currentLlmState.llmSettings, activeProvider, updates);
        updateConfig(buildLlmConfigPatch(nextLlmSettings));
    };

    const handleServiceTypeChange = (type: string) => {
        changeLlmServiceType(type as LlmProvider);
        setAvailableModels([]);
        setIsManualEntry(false);
        setTestStatus('idle');
        setTestMessage('');
    };

    const fetchModels = async () => {
        if (!providerDefinition.supportsModelListing || !llmBaseUrl) {
            setAvailableModels([]);
            setIsManualEntry(true);
            setIsLoadingModels(false);
            return;
        }

        setIsLoadingModels(true);
        try {
            const models = await invoke<string[]>('list_llm_models', {
                request: {
                    provider: activeProvider,
                    baseUrl: llmBaseUrl,
                    apiKey: llmApiKey,
                },
            });
            setAvailableModels(models);

            if (models.length > 0) {
                if (llmModel && !models.includes(llmModel)) {
                    setIsManualEntry(true);
                } else {
                    setIsManualEntry(false);
                    if (!llmModel) {
                        applyProviderSettingUpdates({ model: models[0] });
                    }
                }
            } else {
                setIsManualEntry(true);
            }
        } catch (error) {
            console.warn('Failed to fetch models:', error);
            setAvailableModels([]);
            setIsManualEntry(true);
        } finally {
            setIsLoadingModels(false);
        }
    };

    useEffect(() => {
        if (!providerDefinition.supportsModelListing) {
            setAvailableModels([]);
            setIsManualEntry(true);
            setIsLoadingModels(false);
            return;
        }

        if (debounceTimeout.current) {
            clearTimeout(debounceTimeout.current);
        }

        debounceTimeout.current = setTimeout(() => {
            fetchModels();
        }, 700);

        return () => {
            if (debounceTimeout.current) {
                clearTimeout(debounceTimeout.current);
            }
        };
    }, [activeProvider, llmApiKey, llmBaseUrl, providerDefinition.supportsModelListing]);

    const handleTestConnection = async () => {
        setTestStatus('loading');
        setTestMessage('');
        try {
            const response = await invoke<string>('generate_llm_text', {
                request: {
                    config: llm,
                    input: 'Hello, this is a connection test.',
                },
            });
            setTestStatus('success');
            setTestMessage(response);
        } catch (error) {
            setTestStatus('error');
            setTestMessage(normalizeError(error).message);
        }
    };

    const isConnectionReady = isLlmConfigComplete(llm);
    const apiHostLabel = providerDefinition.apiHostLabel || t('settings.llm.base_url');
    const modelLabel = providerDefinition.modelLabel || t('settings.llm.model_name');

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
                <label className="settings-label">{t('settings.llm.service_type')}</label>
                <Dropdown
                    id="llm-service-type"
                    value={activeProvider}
                    onChange={handleServiceTypeChange}
                    options={providerOptions}
                    style={{ width: '100%' }}
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">{apiHostLabel}</label>
                {providerDefinition.editableApiHost === false ? (
                    <div className="settings-input" style={{ display: 'flex', alignItems: 'center', minHeight: 40, opacity: 0.75 }}>
                        {llmBaseUrl}
                    </div>
                ) : (
                    <input
                        type="text"
                        className="settings-input"
                        value={providerSetting.apiHost}
                        onChange={(e) => applyProviderSettingUpdates({ apiHost: e.target.value })}
                        placeholder={providerDefinition.defaultApiHost}
                    />
                )}
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.llm.api_key')}</label>
                <input
                    type="password"
                    className="settings-input"
                    value={llmApiKey}
                    onChange={(e) => applyProviderSettingUpdates({ apiKey: e.target.value })}
                    placeholder={providerDefinition.requiresApiKey ? 'sk-...' : t('settings.llm.optional_api_key')}
                />
            </div>

            {llmApiVersion && (
                <div className="settings-item">
                    <label className="settings-label">{t('settings.llm.api_version')}</label>
                    <input
                        type="text"
                        className="settings-input"
                        value={llmApiVersion}
                        onChange={(e) => applyProviderSettingUpdates({ apiVersion: e.target.value })}
                        placeholder={providerDefinition.defaultApiVersion || ''}
                    />
                </div>
            )}

            {llmApiPath && (
                <div className="settings-item">
                    <label className="settings-label">{t('settings.llm.api_path')}</label>
                    <input
                        type="text"
                        className="settings-input"
                        value={llmApiPath}
                        onChange={(e) => applyProviderSettingUpdates({ apiPath: e.target.value })}
                        readOnly={activeProvider === 'open_ai_responses' || activeProvider === 'volcengine' || activeProvider === 'perplexity'}
                    />
                </div>
            )}

            <div className="settings-item with-divider">
                <label className="settings-label">
                    {modelLabel}
                    {isLoadingModels && (
                        <span style={{ marginLeft: 10, fontSize: '0.8em', color: 'var(--color-text-muted)' }}>
                            {t('settings.llm.loading_models')}
                        </span>
                    )}
                </label>

                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                    <div style={{ flex: 1 }}>
                        {!isManualEntry && availableModels.length > 0 ? (
                            <Dropdown
                                id="llm-model"
                                value={llmModel}
                                onChange={(value) => {
                                    if (value === MANUAL_MODEL_VALUE) {
                                        setIsManualEntry(true);
                                    } else {
                                        applyProviderSettingUpdates({ model: value });
                                    }
                                }}
                                options={[
                                    ...availableModels.map((model) => ({ value: model, label: model })),
                                    { value: MANUAL_MODEL_VALUE, label: t('settings.llm.type_manually') },
                                ]}
                                style={{ width: '100%' }}
                            />
                        ) : (
                            <input
                                type="text"
                                className="settings-input"
                                value={llmModel}
                                onChange={(e) => applyProviderSettingUpdates({ model: e.target.value })}
                                placeholder={getModelPlaceholder(activeProvider)}
                            />
                        )}
                    </div>

                    {isManualEntry && availableModels.length > 0 && providerDefinition.supportsModelListing && (
                        <button
                            className="btn btn-secondary btn-icon"
                            onClick={() => setIsManualEntry(false)}
                            title={t('settings.llm.back_to_list')}
                            aria-label={t('settings.llm.back_to_list')}
                        >
                            <List size={16} />
                        </button>
                    )}

                    <button
                        className="btn btn-primary btn-loading-wrapper"
                        onClick={handleTestConnection}
                        disabled={testStatus === 'loading' || !isConnectionReady}
                        style={{ whiteSpace: 'nowrap' }}
                    >
                        <span className={testStatus === 'loading' ? 'btn-text-hidden' : ''}>
                            {t('settings.llm.test_connection')}
                        </span>
                        {testStatus === 'loading' && (
                            <div className="btn-spinner-overlay">
                                <Loader2 className="animate-spin" size={16} />
                            </div>
                        )}
                    </button>
                </div>

                <div
                    style={{
                        alignItems: 'center',
                        display: 'grid',
                        gap: '8px',
                        gridTemplateColumns: '1fr 180px 60px',
                        marginTop: '16px',
                    }}
                >
                    <label className="settings-label" style={{ marginBottom: 0, justifySelf: 'start' }}>
                        {t('settings.llm.temperature')}
                    </label>
                    <input
                        type="range"
                        style={{ justifySelf: 'end', margin: 0, width: '180px' }}
                        min={0}
                        max={2}
                        step={0.05}
                        value={llmTemperature}
                        onChange={(e) => applyProviderSettingUpdates({ temperature: parseFloat(e.target.value) })}
                    />
                    <input
                        type="number"
                        className="settings-input"
                        style={{ padding: '2px 4px', textAlign: 'center', width: '60px' }}
                        min={0}
                        max={2}
                        step={0.05}
                        value={llmTemperature}
                        onChange={(e) => {
                            const value = parseFloat(e.target.value);
                            if (!Number.isNaN(value) && value >= 0 && value <= 2) {
                                applyProviderSettingUpdates({ temperature: value });
                            }
                        }}
                    />
                </div>

                {testMessage && (
                    <div className={`connection-status ${testStatus === 'error' ? 'error' : 'success'}`}>
                        {testStatus === 'error' ? (
                            <X size={16} style={{ flexShrink: 0, marginTop: 2 }} />
                        ) : (
                            <Check size={16} style={{ flexShrink: 0, marginTop: 2 }} />
                        )}
                        <div>
                            <strong>
                                {testStatus === 'error'
                                    ? t('settings.llm.connection_failed')
                                    : t('settings.llm.connection_success')}
                            </strong>
                            <div style={{ marginTop: 4, opacity: 0.9 }}>{testMessage}</div>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
