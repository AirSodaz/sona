import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { invoke } from '@tauri-apps/api/core';
import { List, Loader2, Check, X } from 'lucide-react';
import { AppConfig } from '../../types/transcript';

interface SettingsAIServiceTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    changeAiServiceType: (type: string) => void;
}

export function SettingsAIServiceTab({
    config,
    updateConfig,
    changeAiServiceType
}: SettingsAIServiceTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');

    const [availableModels, setAvailableModels] = useState<string[]>([]);
    const [isManualEntry, setIsManualEntry] = useState(false);
    const [isLoadingModels, setIsLoadingModels] = useState(false);
    const debounceTimeout = useRef<NodeJS.Timeout | null>(null);

    const aiServiceType = config.aiServiceType || 'openai';
    const aiBaseUrl = config.aiBaseUrl || '';
    const aiApiKey = config.aiApiKey || '';
    const aiModel = config.aiModel || '';

    const handleServiceTypeChange = (type: string) => {
        changeAiServiceType(type);
        setAvailableModels([]);
        setIsManualEntry(false);
    };

    const updateAiSetting = (key: 'baseUrl' | 'apiKey' | 'model', value: string) => {
        const currentType = config.aiServiceType || 'openai';
        const aiServices = config.aiServices || {};
        const currentSettings = aiServices[currentType] || { baseUrl: '', apiKey: '', model: '' };

        const updates: Partial<AppConfig> = {
            aiServices: {
                ...aiServices,
                [currentType]: { ...currentSettings, [key]: value }
            }
        };

        if (key === 'baseUrl') updates.aiBaseUrl = value;
        if (key === 'apiKey') updates.aiApiKey = value;
        if (key === 'model') updates.aiModel = value;

        updateConfig(updates);
    };

    const fetchModels = async () => {
        if (!aiBaseUrl) return;

        setIsLoadingModels(true);
        try {
            // Note: get_ai_models command might need to be adjusted if it relies on store state,
            // but here we pass params explicitly.
            const models = await invoke<string[]>('get_ai_models', {
                apiKey: aiApiKey,
                baseUrl: aiBaseUrl,
                apiFormat: aiServiceType
            });
            setAvailableModels(models);

            if (models.length > 0) {
                 if (aiModel && !models.includes(aiModel)) {
                      // If current model is not in list, keep it but show manual entry
                      setIsManualEntry(true);
                 } else {
                      // If current model is in list or empty, use dropdown
                      setIsManualEntry(false);
                      if (!aiModel) {
                          updateAiSetting('model', models[0]);
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
    }, [aiBaseUrl, aiApiKey, aiServiceType]);

    const handleTestConnection = async () => {
        setTestStatus('loading');
        setTestMessage('');
        try {
            const response = await invoke<string>('call_ai_model', {
                apiKey: aiApiKey,
                baseUrl: aiBaseUrl,
                modelName: aiModel,
                input: 'Hello, this is a connection test.',
                apiFormat: aiServiceType
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
                    value={aiServiceType}
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
                    value={aiBaseUrl}
                    onChange={(e) => updateAiSetting('baseUrl', e.target.value)}
                    placeholder="https://api.openai.com/v1"
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.api_key')}</label>
                <input
                    type="password"
                    className="settings-input"
                    value={aiApiKey}
                    onChange={(e) => updateAiSetting('apiKey', e.target.value)}
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
                                value={aiModel}
                                onChange={(val) => {
                                    if (val === '__manual__') {
                                        setIsManualEntry(true);
                                    } else {
                                        updateAiSetting('model', val);
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
                                value={aiModel}
                                onChange={(e) => updateAiSetting('model', e.target.value)}
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
