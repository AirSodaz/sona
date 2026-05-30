import React, { useCallback, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Server, RefreshCw, Copy, Check } from 'lucide-react';
import { useApiServerConfig, useSetConfig } from '../../stores/configStore';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer } from './SettingsLayout';
import { Switch } from '../Switch';
import { invokeTauri } from '../../services/tauri/invoke';
import { TauriCommand } from '../../services/tauri/commands';

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
            // eslint-disable-next-line no-console
            console.error('Failed to copy API key: ', err);
        });
    }, [config.httpServerApiKey]);

    useEffect(() => {
        const timer = setTimeout(() => {
            if (config.httpServerEnabled) {
                invokeTauri(TauriCommand.apiServer.start, {
                    host: config.httpServerHost ?? '127.0.0.1',
                    port: config.httpServerPort ?? 14200,
                    apiKey: config.httpServerApiKey ?? ''
                }).catch((e) => {
                    // eslint-disable-next-line no-console
                    console.error(e);
                });
            } else {
                invokeTauri(TauriCommand.apiServer.stop).catch((e) => {
                    // eslint-disable-next-line no-console
                    console.error(e);
                });
            }
        }, 1000);
        return () => clearTimeout(timer);
    }, [config.httpServerEnabled, config.httpServerHost, config.httpServerPort, config.httpServerApiKey]);

    return (
        <SettingsTabContainer id="settings-panel-api-server" ariaLabelledby="settings-tab-api_server">
            <SettingsPageHeader
                icon={<Server width={28} height={28} />}
                title={t('settings.api_server.title', { defaultValue: 'API Server' })}
                description={t('settings.api_server.description', {
                    defaultValue: 'Expose an HTTP API for external headless integration with Sona. Features like live recording and batch transcription can be invoked via HTTP.',
                })}
            />

            <SettingsSection>
                {/* Enable Toggle */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('settings.api_server.enable_label', { defaultValue: 'Enable API Server' })}
                        </div>
                    </div>
                    <div className="settings-item-action">
                        <Switch
                            checked={config.httpServerEnabled ?? false}
                            onChange={(checked) => setConfig({ httpServerEnabled: checked })}
                            aria-label={t('settings.api_server.enable_label', { defaultValue: 'Enable API Server' })}
                        />
                    </div>
                </div>

                {/* Host */}
                <div className="settings-item-container layout-horizontal">
                    <div className="settings-item-info">
                        <div className="settings-item-title">
                            {t('settings.api_server.host_label', { defaultValue: 'Host' })}
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
                            {t('settings.api_server.port_label', { defaultValue: 'Port' })}
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
                            {t('settings.api_server.api_key_label', { defaultValue: 'API Key' })}
                        </div>
                    </div>
                    <div className="settings-item-action" style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                        <input
                            type="password"
                            className="input-text"
                            value={config.httpServerApiKey ?? ''}
                            onChange={handleKeyChange}
                            style={{ width: '280px' }}
                            placeholder={t('settings.api_server.api_key_placeholder', { defaultValue: 'Optional bearer token' })}
                        />
                        <button 
                            className="btn btn-icon" 
                            onClick={handleGenerateKey}
                            title={t('settings.api_server.generate_key', { defaultValue: 'Generate' })}
                            aria-label={t('settings.api_server.generate_key', { defaultValue: 'Generate' })}
                        >
                            <RefreshCw size={16} />
                        </button>
                        <button 
                            className="btn btn-icon" 
                            onClick={handleCopyKey}
                            title={t('settings.api_server.copy_key', { defaultValue: 'Copy' })}
                            aria-label={t('settings.api_server.copy_key', { defaultValue: 'Copy' })}
                        >
                            {copied ? <Check size={16} color="green" /> : <Copy size={16} />}
                        </button>
                    </div>
                </div>

            </SettingsSection>
        </SettingsTabContainer>
    );
}
