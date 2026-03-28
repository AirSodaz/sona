import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { invoke } from '@tauri-apps/api/core';
import { List, Loader2, Check, X } from 'lucide-react';
import { AppConfig } from '../../types/transcript';

interface SettingsLLMServiceTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    changeLlmServiceType: (type: string) => void;
}

export function SettingsLLMServiceTab({
    config,
    updateConfig,
    changeLlmServiceType
}: SettingsLLMServiceTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');

    const [availableModels, setAvailableModels] = useState<string[]>([]);
    const [isManualEntry, setIsManualEntry] = useState(false);
    const [isLoadingModels, setIsLoadingModels] = useState(false);
    const debounceTimeout = useRef<NodeJS.Timeout | null>(null);

    const llmServiceType = config.llmServiceType || 'openai';
    const llmBaseUrl = config.llmBaseUrl || '';
    const llmApiKey = config.llmApiKey || '';
    const llmModel = config.llmModel || '';
    const llmTemperature = config.llmTemperature ?? 0.7;

    const handleServiceTypeChange = (type: string) => {
        changeLlmServiceType(type);
        setAvailableModels([]);
        setIsManualEntry(false);
    };

    const updateLlmSetting = (key: 'baseUrl' | 'apiKey' | 'model' | 'temperature', value: string | number) => {
        const currentType = config.llmServiceType || 'openai';
        const llmServices = config.llmServices || {};
        const currentSettings = llmServices[currentType] || { baseUrl: '', apiKey: '', model: '', temperature: 0.7 };

        const updates: Partial<AppConfig> = {
            llmServices: {
                ...llmServices,
                [currentType]: { ...currentSettings, [key]: value }
            }
        };

        switch (key) {
            case 'baseUrl':
                updates.llmBaseUrl = value as string;
                break;
            case 'apiKey':
                updates.llmApiKey = value as string;
                break;
            case 'model':
                updates.llmModel = value as string;
                break;
            case 'temperature':
                updates.llmTemperature = value as number;
                break;
        }

        updateConfig(updates);
    };

    const fetchModels = async () => {
        if (!llmBaseUrl) return;

        setIsLoadingModels(true);
        try {
            // Note: get_llm_models command might need to be adjusted if it relies on store state,
            // but here we pass params explicitly.
            const models = await invoke<string[]>('get_llm_models', {
                apiKey: llmApiKey,
                baseUrl: llmBaseUrl,
                apiFormat: llmServiceType
            });
            setAvailableModels(models);

            if (models.length > 0) {
                 if (llmModel && !models.includes(llmModel)) {
                      // If current model is not in list, keep it but show manual entry
                      setIsManualEntry(true);
                 } else {
                      // If current model is in list or empty, use dropdown
                      setIsManualEntry(false);
                      if (!llmModel) {
                          updateLlmSetting('model', models[0]);
                      }
                 }
            } else {
                // No models found, fallback to manual
                setIsManualEntry(true);
            }
        } catch (error) {
            console.warn("Failed to fetch models:", error);
            setAvailableModels([]);
            setIsManualEntry(true);
        } finally {
            setIsLoadingModels(false);
        }
    };

    useEffect(() => {
        if (debounceTimeout.current) {
            clearTimeout(debounceTimeout.current);
        }

        debounceTimeout.current = setTimeout(() => {
            fetchModels();
        }, 1000);

        return () => {
            if (debounceTimeout.current) {
                clearTimeout(debounceTimeout.current);
            }
        };
    }, [llmBaseUrl, llmApiKey, llmServiceType]);

    const handleTestConnection = async () => {
        setTestStatus('loading');
        setTestMessage('');
        try {
            const response = await invoke<string>('call_llm_model', {
                apiKey: llmApiKey,
                baseUrl: llmBaseUrl,
                modelName: llmModel,
                input: 'Hello, this is a connection test.',
                apiFormat: llmServiceType,
                temperature: llmTemperature
            });
            setTestStatus('success');
            setTestMessage(t('settings.ai.connection_success') + response);
        } catch (error: any) {
            setTestStatus('error');
            setTestMessage(t('settings.ai.connection_failed') + error);
        }
    };

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.service_type')}</label>
                <Dropdown
                    id="ai-service-type"
                    value={llmServiceType}
                    onChange={handleServiceTypeChange}
                    options={[
                        { value: 'openai', label: 'OpenAI' },
                        { value: 'anthropic', label: 'Anthropic' },
                        { value: 'ollama', label: 'Ollama' },
                        { value: 'gemini', label: 'Google Gemini' },
                        { value: 'deepseek', label: 'DeepSeek' },
                        { value: 'kimi', label: 'Kimi' },
                        { value: 'siliconflow', label: 'SiliconFlow' }
                    ]}
                    style={{ width: '100%' }}
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.base_url')}</label>
                <input
                    type="text"
                    className="settings-input"
                    value={llmBaseUrl}
                    onChange={(e) => updateLlmSetting('baseUrl', e.target.value)}
                    placeholder="https://api.openai.com/v1"
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.api_key')}</label>
                <input
                    type="password"
                    className="settings-input"
                    value={llmApiKey}
                    onChange={(e) => updateLlmSetting('apiKey', e.target.value)}
                    placeholder="sk-..."
                />
            </div>

            <div className="settings-item with-divider">
                <label className="settings-label">
                    {t('settings.ai.model_name')}
                    {isLoadingModels && <span style={{ marginLeft: 10, fontSize: '0.8em', color: 'var(--color-text-muted)' }}>{t('settings.ai.loading_models') || '(Loading models...)'}</span>}
                </label>

                <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                    <div style={{ flex: 1 }}>
                        {!isManualEntry && availableModels.length > 0 ? (
                            <Dropdown
                                id="ai-model"
                                value={llmModel}
                                onChange={(val) => {
                                    if (val === '__manual__') {
                                        setIsManualEntry(true);
                                    } else {
                                        updateLlmSetting('model', val);
                                    }
                                }}
                                options={[
                                    ...availableModels.map(m => ({ value: m, label: m })),
                                    { value: '__manual__', label: t('settings.ai.type_manually') || 'Type manually...' }
                                ]}
                                style={{ width: '100%' }}
                            />
                        ) : (
                            <input
                                type="text"
                                className="settings-input"
                                value={llmModel}
                                onChange={(e) => updateLlmSetting('model', e.target.value)}
                                placeholder="gpt-4o"
                            />
                        )}
                    </div>

                    {isManualEntry && availableModels.length > 0 && (
                        <button
                            className="btn btn-secondary btn-icon"
                            onClick={() => setIsManualEntry(false)}
                            title={t('settings.ai.back_to_list') || 'Back to list'}
                            aria-label={t('settings.ai.back_to_list') || 'Back to list'}
                        >
                            <List size={16} />
                        </button>
                    )}

                    <button
                        className="btn btn-primary btn-loading-wrapper"
                        onClick={handleTestConnection}
                        disabled={testStatus === 'loading'}
                        style={{ whiteSpace: 'nowrap' }}
                    >
                        <span className={testStatus === 'loading' ? 'btn-text-hidden' : ''}>
                            {t('settings.ai.test_connection')}
                        </span>
                        {testStatus === 'loading' && (
                            <div className="btn-spinner-overlay">
                                <Loader2 className="animate-spin" size={16} />
                            </div>
                        )}
                    </button>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', marginTop: '16px' }}>
                    <label className="settings-label" style={{ marginBottom: 0, minWidth: 'fit-content' }}>
                        {t('settings.ai.temperature') || 'Temperature'}
                    </label>
                    <input
                        type="range"
                        style={{ flex: 1, margin: 0 }}
                        min={0}
                        max={2}
                        step={0.05}
                        value={llmTemperature}
                        onChange={(e) => updateLlmSetting('temperature', parseFloat(e.target.value))}
                    />
                    <input
                        type="number"
                        className="settings-input"
                        style={{ width: '60px', padding: '2px 4px', textAlign: 'center' }}
                        min={0}
                        max={2}
                        step={0.05}
                        value={llmTemperature}
                        onChange={(e) => {
                            const val = parseFloat(e.target.value);
                            if (!isNaN(val) && val >= 0 && val <= 2) {
                                updateLlmSetting('temperature', val);
                            }
                        }}
                    />
                </div>

                {testMessage && (
                    <div className={`connection-status ${testStatus === 'error' ? 'error' : 'success'}`}>
                        {testStatus === 'error' ? <X size={16} style={{ flexShrink: 0, marginTop: 2 }} /> : <Check size={16} style={{ flexShrink: 0, marginTop: 2 }} />}
                        <div>
                            <strong>{testStatus === 'error' ? t('settings.ai.connection_failed') : t('settings.ai.connection_success')}</strong>
                            <div style={{ marginTop: 4, opacity: 0.9 }}>{testMessage.replace(t('settings.ai.connection_success'), '').replace(t('settings.ai.connection_failed'), '')}</div>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
