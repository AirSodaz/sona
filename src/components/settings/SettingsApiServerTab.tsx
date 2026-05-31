import React, { useCallback, useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Server, RefreshCw, Copy, Check, Activity, Info, Clock, Zap } from 'lucide-react';

import { useApiServerConfig, useSetConfig } from '../../stores/configStore';
import { SettingsPageHeader, SettingsSection, SettingsTabContainer, SettingsItem } from './SettingsLayout';
import { Switch } from '../Switch';
import { invokeTauri } from '../../services/tauri/invoke';
import { TauriCommand } from '../../services/tauri/commands';

interface ServerHealth {
  status: string;
  version: string;
  uptime: number;
}

interface ServerInfo {
  platform: string;
  gpuAvailable: boolean;
  models: string[];
  vadInstalled: boolean;
  punctuationInstalled: boolean;
}

type JobStatus = 'Pending' | 'Processing' | { Completed: unknown } | { Failed: string };

export function SettingsApiServerTab(): React.JSX.Element {
    const { t } = useTranslation();
    const config = useApiServerConfig();
    const setConfig = useSetConfig();

    const [copied, setCopied] = useState(false);
    const [health, setHealth] = useState<ServerHealth | null>(null);
    const [info, setInfo] = useState<ServerInfo | null>(null);
    const [jobs, setJobs] = useState<Record<string, JobStatus>>({});
    const [lastError, setLastError] = useState<string | null>(null);

    const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);

    useEffect(() => {
        if (copied) {
            const timer = setTimeout(() => setCopied(false), 2000);
            return () => clearTimeout(timer);
        }
    }, [copied]);

    const fetchData = useCallback(async () => {
        if (!config.httpServerEnabled) return;

        const host = config.httpServerHost || '127.0.0.1';
        const port = config.httpServerPort || 14200;
        const apiKey = config.httpServerApiKey || '';
        const baseUrl = `http://${host}:${port}`;

        const headers: Record<string, string> = {};
        if (apiKey) {
            headers['Authorization'] = `Bearer ${apiKey}`;
        }

        try {
            // Fetch Health (Public)
            const healthRes = await fetch(`${baseUrl}/health`);
            if (healthRes.ok) {
                const healthData = await healthRes.json();
                setHealth(healthData);
            } else {
                setHealth(null);
            }

            // Fetch Info (Public)
            const infoRes = await fetch(`${baseUrl}/info`);
            if (infoRes.ok) {
                const infoData = await infoRes.json();
                setInfo(infoData);
            } else {
                setInfo(null);
            }

            // Fetch Jobs (Private)
            const jobsRes = await fetch(`${baseUrl}/v1/transcriptions/jobs`, { headers });
            if (jobsRes.ok) {
                const jobsData = await jobsRes.json();
                setJobs(jobsData);
            } else {
                setJobs({});
            }

            setLastError(null);
        } catch (err) {
            setHealth(null);
            setInfo(null);
            setJobs({});
            setLastError(err instanceof Error ? err.message : String(err));
        }
    }, [config.httpServerEnabled, config.httpServerHost, config.httpServerPort, config.httpServerApiKey]);

    useEffect(() => {
        if (config.httpServerEnabled) {
            // eslint-disable-next-line react-hooks/set-state-in-effect
            void fetchData();
            pollIntervalRef.current = setInterval(fetchData, 3000);
        } else {
            setHealth(null);
            setInfo(null);
            setJobs({});
            if (pollIntervalRef.current) {
                clearInterval(pollIntervalRef.current);
            }
        }
        return () => {
            if (pollIntervalRef.current) {
                clearInterval(pollIntervalRef.current);
            }
        };
    }, [config.httpServerEnabled, fetchData]);

    const formatUptime = (seconds: number) => {
        const h = Math.floor(seconds / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        const s = seconds % 60;
        return `${h}h ${m}m ${s}s`;
    };

    const getStatusLabel = (status: JobStatus) => {
        if (status === 'Pending') return <span className="badge badge-pending">Pending</span>;
        if (status === 'Processing') return <span className="badge badge-processing">Processing</span>;
        if (typeof status === 'object') {
            if ('Completed' in status) return <span className="badge badge-completed">Completed</span>;
            if ('Failed' in status) return <span className="badge badge-failed" title={status.Failed}>Failed</span>;
        }
        return <span className="badge">Unknown</span>;
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
                <SettingsItem
                    title={t('settings.api_server.enable_label', { defaultValue: 'Enable API Server' })}
                    hint={t('settings.api_server.enable_hint', { defaultValue: 'Start an HTTP server to control Sona via external applications.' })}
                >
                    <Switch
                        checked={config.httpServerEnabled ?? false}
                        onChange={(checked) => setConfig({ httpServerEnabled: checked })}
                        aria-label={t('settings.api_server.enable_label', { defaultValue: 'Enable API Server' })}
                    />
                </SettingsItem>

                {/* Host */}
                <SettingsItem
                    title={t('settings.api_server.host_label', { defaultValue: 'Host' })}
                    hint={t('settings.api_server.host_hint', { defaultValue: "Bind address for the server. '127.0.0.1' restricts access to localhost, while '0.0.0.0' allows remote access." })}
                >
                    <input
                        type="text"
                        className="input-text"
                        value={config.httpServerHost ?? '127.0.0.1'}
                        onChange={handleHostChange}
                        style={{ width: '200px' }}
                    />
                </SettingsItem>

                {/* Port */}
                <SettingsItem
                    title={t('settings.api_server.port_label', { defaultValue: 'Port' })}
                    hint={t('settings.api_server.port_hint', { defaultValue: 'TCP port for the API server. Must be between 1 and 65535 (default: 14200).' })}
                >
                    <input
                        type="number"
                        className="input-text"
                        value={config.httpServerPort ?? 14200}
                        onChange={handlePortChange}
                        min={1}
                        max={65535}
                        style={{ width: '200px' }}
                    />
                </SettingsItem>

                {/* API Key */}
                <SettingsItem
                    title={t('settings.api_server.api_key_label', { defaultValue: 'API Key' })}
                    hint={t('settings.api_server.api_key_hint', { defaultValue: 'Optional Bearer token for authenticating HTTP requests.' })}
                >
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
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
                            data-tooltip={t('settings.api_server.generate_key', { defaultValue: 'Generate' })}
                            data-tooltip-pos="top"
                            aria-label={t('settings.api_server.generate_key', { defaultValue: 'Generate' })}
                        >
                            <RefreshCw size={16} />
                        </button>
                        <button 
                            className="btn btn-icon" 
                            onClick={handleCopyKey}
                            data-tooltip={copied ? t('settings.api_server.copied', { defaultValue: 'Copied!' }) : t('settings.api_server.copy_key', { defaultValue: 'Copy' })}
                            data-tooltip-pos="top"
                            aria-label={t('settings.api_server.copy_key', { defaultValue: 'Copy' })}
                        >
                            {copied ? <Check size={16} color="green" /> : <Copy size={16} />}
                        </button>
                    </div>
                </SettingsItem>

            </SettingsSection>

            {config.httpServerEnabled && (
                <>
                    <SettingsSection title={t('settings.api_server.status_title', { defaultValue: 'Server Status' })}>
                        <div className="settings-api-server-panel">
                            <div className="api-server-status-grid">
                                <div className="status-card">
                                    <div className="status-card-icon"><Activity size={20} /></div>
                                    <div className="status-card-content">
                                        <div className="status-card-label">{t('settings.api_server.status_label_state', { defaultValue: 'State' })}</div>
                                        <div className={`status-card-value ${health ? 'text-success' : 'text-error'}`}>
                                            {health ? t('settings.api_server.state_running', { defaultValue: 'Running' }) : t('settings.api_server.state_stopped', { defaultValue: 'Stopped' })}
                                        </div>
                                    </div>
                                </div>
                                <div className="status-card">
                                    <div className="status-card-icon"><Clock size={20} /></div>
                                    <div className="status-card-content">
                                        <div className="status-card-label">{t('settings.api_server.status_label_uptime', { defaultValue: 'Uptime' })}</div>
                                        <div className="status-card-value">{health ? formatUptime(health.uptime) : '-'}</div>
                                    </div>
                                </div>
                                <div className="status-card">
                                    <div className="status-card-icon"><Zap size={20} /></div>
                                    <div className="status-card-content">
                                        <div className="status-card-label">{t('settings.api_server.status_label_gpu', { defaultValue: 'GPU Acceleration' })}</div>
                                        <div className="status-card-value">{info?.gpuAvailable ? t('common.enabled', { defaultValue: 'Enabled' }) : t('common.disabled', { defaultValue: 'Disabled' })}</div>
                                    </div>
                                </div>
                                <div className="status-card">
                                    <div className="status-card-icon"><Info size={20} /></div>
                                    <div className="status-card-content">
                                        <div className="status-card-label">{t('settings.api_server.status_label_version', { defaultValue: 'Version' })}</div>
                                        <div className="status-card-value">{health?.version || '-'}</div>
                                    </div>
                                </div>
                            </div>
                            {lastError && !health && (
                                <div className="settings-error-notice" style={{ marginTop: '12px' }}>
                                    {t('settings.api_server.connection_error', { defaultValue: 'Connection Error' })}: {lastError}
                                </div>
                            )}
                        </div>
                    </SettingsSection>

                    <SettingsSection title={t('settings.api_server.jobs_title', { defaultValue: 'Job Queue' })}>
                        <div className="settings-api-server-panel">
                            <div className="api-server-jobs-container">
                                {Object.keys(jobs).length === 0 ? (
                                    <div className="empty-state-mini">
                                        <p>{t('settings.api_server.no_jobs', { defaultValue: 'No active or recent jobs.' })}</p>
                                    </div>
                                ) : (
                                    <table className="jobs-table">
                                        <thead>
                                            <tr>
                                                <th>{t('settings.api_server.job_id', { defaultValue: 'Job ID' })}</th>
                                                <th>{t('settings.api_server.job_status', { defaultValue: 'Status' })}</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {Object.entries(jobs).map(([id, status]) => (
                                                <tr key={id}>
                                                    <td className="job-id-cell" title={id}>{id.slice(0, 8)}...</td>
                                                    <td>{getStatusLabel(status)}</td>
                                                </tr>
                                            ))}
                                        </tbody>
                                    </table>
                                )}
                            </div>
                        </div>
                    </SettingsSection>
                </>
            )}
        </SettingsTabContainer>
    );
}
