import React, { useState } from 'react';
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
    const [testStatus, setTestStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');

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
            setTestMessage('Connection successful! Response: ' + response);
        } catch (error: any) {
            setTestStatus('error');
            setTestMessage('Connection failed: ' + error);
        }
    };

    return (
        <div className="settings-group" role="tabpanel">
            <div className="settings-item">
                <label className="settings-label">AI Service Type</label>
                <div style={{ maxWidth: 300 }}>
                    <Dropdown
                        id="ai-service-type"
                        value={aiServiceType}
                        onChange={setAiServiceType}
                        options={[
                            { value: 'openai', label: 'OpenAI Compatible' },
                            { value: 'anthropic', label: 'Anthropic' },
                            { value: 'ollama', label: 'Ollama' },
                            { value: 'gemini', label: 'Google Gemini' }
                        ]}
                    />
                </div>
            </div>

            <div className="settings-item">
                <label className="settings-label">Base URL</label>
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
                <label className="settings-label">API Key</label>
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
                <label className="settings-label">Model Name</label>
                <input
                    type="text"
                    className="settings-input"
                    value={aiModel}
                    onChange={(e) => setAiModel(e.target.value)}
                    placeholder="gpt-4o"
                    style={{ width: '100%', padding: '8px', marginTop: '8px', borderRadius: '4px', border: '1px solid var(--color-border)', backgroundColor: 'var(--color-bg-input)', color: 'var(--color-text)' }}
                />
            </div>

            <div className="settings-item" style={{ marginTop: 20 }}>
                <button
                    className="btn btn-primary"
                    onClick={handleTestConnection}
                    disabled={testStatus === 'loading'}
                    style={{ padding: '8px 16px', borderRadius: '4px', cursor: 'pointer' }}
                >
                    {testStatus === 'loading' ? 'Testing...' : 'Test Connection'}
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
