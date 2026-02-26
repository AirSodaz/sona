import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { invoke } from '@tauri-apps/api/core';

interface SettingsAIServiceTabProps {
    aiServiceType: string;
    setAiServiceType: (type: string) => void;
    aiBaseUrl: string;
    setAiBaseUrl: (url: string) => void;
    aiApiKey: string;
    setAiApiKey: (key: string) => void;
    aiModel: string;
    setAiModel: (model: string) => void;
}

export function SettingsAIServiceTab({
    aiServiceType,
    setAiServiceType,
    aiBaseUrl,
    setAiBaseUrl,
    aiApiKey,
    setAiApiKey,
    aiModel,
    setAiModel
}: SettingsAIServiceTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');

    const [availableModels, setAvailableModels] = useState<string[]>([]);
    const [isManualEntry, setIsManualEntry] = useState(false);
    const [isLoadingModels, setIsLoadingModels] = useState(false);
    const debounceTimeout = useRef<NodeJS.Timeout | null>(null);

    const handleServiceTypeChange = (type: string) => {
        setAiServiceType(type);
        setAvailableModels([]);
        setIsManualEntry(false);
    };

    const fetchModels = async () => {
        if (!aiBaseUrl) return;

        setIsLoadingModels(true);
        try {
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
                          setAiModel(models[0]);
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
                <div style={{ maxWidth: 300 }}>
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
                    />
                </div>
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.base_url')}</label>
                <input
                    type="text"
                    className="settings-input"
                    value={aiBaseUrl}
                    onChange={(e) => setAiBaseUrl(e.target.value)}
                    placeholder="https://api.openai.com/v1"
                    style={{ width: '100%', padding: '8px', marginTop: '8px', borderRadius: '4px', border: '1px solid var(--color-border)', backgroundColor: 'var(--color-bg-input)', color: 'var(--color-text)' }}
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">{t('settings.ai.api_key')}</label>
                <input
                    type="password"
                    className="settings-input"
                    value={aiApiKey}
                    onChange={(e) => setAiApiKey(e.target.value)}
                    placeholder="sk-..."
                    style={{ width: '100%', padding: '8px', marginTop: '8px', borderRadius: '4px', border: '1px solid var(--color-border)', backgroundColor: 'var(--color-bg-input)', color: 'var(--color-text)' }}
                />
            </div>

            <div className="settings-item">
                <label className="settings-label">
                    {t('settings.ai.model_name')}
                    {isLoadingModels && <span style={{ marginLeft: 10, fontSize: '0.8em', color: 'var(--color-text-muted)' }}>{t('settings.ai.loading_models') || '(Loading models...)'}</span>}
                </label>

                {!isManualEntry && availableModels.length > 0 ? (
                    <div style={{ maxWidth: 300 }}>
                         <Dropdown
                            id="ai-model"
                            value={aiModel}
                            onChange={(val) => {
                                if (val === '__manual__') {
                                    setIsManualEntry(true);
                                } else {
                                    setAiModel(val);
                                }
                            }}
                            options={[
                                ...availableModels.map(m => ({ value: m, label: m })),
                                { value: '__manual__', label: t('settings.ai.type_manually') || 'Type manually...' }
                            ]}
                        />
                    </div>
                ) : (
                    <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                        <input
                            type="text"
                            className="settings-input"
                            value={aiModel}
                            onChange={(e) => setAiModel(e.target.value)}
                            placeholder="gpt-4o"
                            style={{ flex: 1, padding: '8px', borderRadius: '4px', border: '1px solid var(--color-border)', backgroundColor: 'var(--color-bg-input)', color: 'var(--color-text)' }}
                        />
                        {availableModels.length > 0 && (
                            <button
                                className="btn btn-secondary"
                                onClick={() => setIsManualEntry(false)}
                                style={{ padding: '8px', whiteSpace: 'nowrap', cursor: 'pointer', borderRadius: '4px', border: '1px solid var(--color-border)', backgroundColor: 'var(--color-bg-input)' }}
                            >
                                {t('settings.ai.back_to_list') || 'Back to list'}
                            </button>
                        )}
                    </div>
                )}
            </div>

            <div className="settings-item" style={{ marginTop: 20 }}>
                <button
                    className="btn btn-primary"
                    onClick={handleTestConnection}
                    disabled={testStatus === 'loading'}
                    style={{ padding: '8px 16px', borderRadius: '4px', cursor: 'pointer' }}
                >
                    {testStatus === 'loading' ? t('settings.ai.testing') : t('settings.ai.test_connection')}
                </button>
                {testMessage && (
                    <div style={{ marginTop: 10, color: testStatus === 'error' ? 'red' : 'green', whiteSpace: 'pre-wrap', maxHeight: '200px', overflowY: 'auto', border: '1px solid var(--color-border)', padding: '8px', borderRadius: '4px' }}>
                        {testMessage}
                    </div>
                )}
            </div>
        </div>
    );
}
