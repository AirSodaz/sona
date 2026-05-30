import React, { useCallback, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Server, RefreshCw, Copy, Check } from 'lucide-react';
import { useApiServerConfig, useSetConfig } from '../../stores/configStore';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';

export function SettingsApiServerTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useApiServerConfig();
    const setConfig = useSetConfig();

    const [copied, setCopied] = useState(false);

    useEffect(() => {
        if (copied) {
            const timer = setTimeout(() => setCopied(false), 2000);
            return () => clearTimeout(timer);
        }
    }, [copied]);

    const handleEnableChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setConfig({ httpServerEnabled: e.target.checked });
    };

    const handleHostChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setConfig({ httpServerHost: e.target.value });
    };

    const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const port = parseInt(e.target.value, 10);
        if (!isNaN(port)) {
            setConfig({ httpServerPort: port });
        }
    };

    const handleKeyChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setConfig({ httpServerApiKey: e.target.value });
    };

    const handleGenerateKey = useCallback(() => {
        const newKey = crypto.randomUUID();
        setConfig({ httpServerApiKey: newKey });
    }, [setConfig]);

    const handleCopyKey = useCallback(() => {
        const key = config.httpServerApiKey || '';
        navigator.clipboard.writeText(key).then(() => {
            setCopied(true);
        }).catch((err) => {
            console.error('Failed to copy API key: ', err);
        });
    }, [config.httpServerApiKey]);

    return (
        <SettingsTabContainer id="settings-panel-api-server" ariaLabelledby="settings-tab-api_server">
            <SettingsPageHeader
                icon={<Server width={28} height={28} />}
                title={t('api_server.title', { defaultValue: 'API Server' })}
                description={t('api_server.description', {
                    defaultValue: 'Expose an HTTP API for external headless integration with Sona. Features like live recording and batch transcription can be invoked via HTTP.',
                })}
            />

            <div className="settings-banner warning" style={{ marginBottom: '24px', padding: '12px 16px', backgroundColor: 'var(--color-bg-warning, #fff3cd)', color: 'var(--color-text-warning, #856404)', borderRadius: '6px', fontSize: '14px' }}>
                {t('api_server.restart_warning', { defaultValue: 'Changes to the API Server take effect on the next application launch.' })}
            </div>

            <SettingsSection>
                {/* Enable Toggle */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('api_server.enable_label', { defaultValue: 'Enable API Server' })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <label className="toggle-switch">
                            <input
                                type="checkbox"
                                checked={config.httpServerEnabled ?? false}
                                onChange={handleEnableChange}
                            />
                            <span className="toggle-slider"></span>
                        </label>
                    </div>
                </div>

                {/* Host */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('api_server.host_label', { defaultValue: 'Host' })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <input
                            type="text"
                            className="input-text"
                            value={config.httpServerHost ?? '127.0.0.1'}
                            onChange={handleHostChange}
                            style={{ width: '200px' }}
                        />
                    </div>
                </div>

                {/* Port */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('api_server.port_label', { defaultValue: 'Port' })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <input
                            type="number"
                            className="input-text"
                            value={config.httpServerPort ?? 14200}
                            onChange={handlePortChange}
                            min={1}
                            max={65535}
                            style={{ width: '200px' }}
                        />
                    </div>
                </div>

                {/* API Key */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('api_server.api_key_label', { defaultValue: 'API Key' })}
                        </div>
                    </div>
                    <div className="settings-item-action" style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                        <input
                            type="password"
                            className="input-text"
                            value={config.httpServerApiKey ?? ''}
                            onChange={handleKeyChange}
                            style={{ width: '280px' }}
                            placeholder="Optional bearer token"
                        />
                        <button 
                            className="btn btn-icon" 
                            onClick={handleGenerateKey}
                            title={t('api_server.generate_key', { defaultValue: 'Generate' })}
                            aria-label={t('api_server.generate_key', { defaultValue: 'Generate' })}
                        >
                            <RefreshCw size={16} />
                        </button>
                        <button 
                            className="btn btn-icon" 
                            onClick={handleCopyKey}
                            title={t('api_server.copy_key', { defaultValue: 'Copy' })}
                            aria-label={t('api_server.copy_key', { defaultValue: 'Copy' })}
                        >
                            {copied ? <Check size={16} color="green" /> : <Copy size={16} />}
                        </button>
                    </div>
                </div>

            </SettingsSection>
        </SettingsTabContainer>
    );
}
