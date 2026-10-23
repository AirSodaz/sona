import {
  Activity,
  Check,
  Clock,
  Copy,
  Globe,
  HardDrive,
  List,
  RefreshCw,
  Server,
  Zap,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { TauriCommand, useTransport } from '../../platform';
import { useApiServerConfig, useSetConfig } from '../../stores/configStore';
import { useDialogStore } from '../../stores/dialogStore';
import type {
  ApiServerDashboardSnapshot,
  ApiServerHealth,
  ApiServerInfo,
  ApiServerJobStatus,
} from '../../types/apiServer';
import { extractErrorMessage } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { ToolHeroHeader, type ToolHeroStatusTone } from '../tools/ToolHeroHeader';
import { SettingsItem, SettingsSection, SettingsTabContainer } from './SettingsLayout';

export function SettingsApiServerTab(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useApiServerConfig();
  const setConfig = useSetConfig();
  const transport = useTransport();

  const [copied, setCopied] = useState(false);
  const [copiedEndpoint, setCopiedEndpoint] = useState(false);
  const [health, setHealth] = useState<ApiServerHealth | null>(null);
  const [info, setInfo] = useState<ApiServerInfo | null>(null);
  const [jobs, setJobs] = useState<Record<string, ApiServerJobStatus>>({});
  const [lastError, setLastError] = useState<string | null>(null);
  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const healthRef = useRef<ApiServerHealth | null>(null);
  healthRef.current = health;

  useEffect(() => {
    if (copied) {
      const timer = setTimeout(() => setCopied(false), 2000);
      return () => clearTimeout(timer);
    }
  }, [copied]);

  useEffect(() => {
    if (copiedEndpoint) {
      const timer = setTimeout(() => setCopiedEndpoint(false), 2000);
      return () => clearTimeout(timer);
    }
  }, [copiedEndpoint]);

  const handleCopyEndpoint = async () => {
    const host = config.httpServerHost || '127.0.0.1';
    const port = config.httpServerPort ?? 14200;
    const formattedHost = host.includes(':') && !host.startsWith('[') ? `[${host}]` : host;
    const url = `http://${formattedHost}:${port}`;
    try {
      await navigator.clipboard.writeText(url);
      setCopiedEndpoint(true);
    } catch (e) {
      logger.error('[ApiServer] Failed to copy endpoint URL:', e);
    }
  };
  const fetchData = useCallback(async () => {
    if (!config.httpServerEnabled) return;

    try {
      const snapshot = await transport.invoke<ApiServerDashboardSnapshot>(
        TauriCommand.apiServer.dashboardSnapshot
      );
      setHealth(snapshot?.health ?? null);
      setInfo(snapshot?.info ?? null);
      setJobs(snapshot?.jobs ?? {});
      setLastError(null);
    } catch (err) {
      setHealth(null);
      setInfo(null);
      setJobs({});
      const errMsg = extractErrorMessage(err);
      if (!(healthRef.current === null && errMsg.toLowerCase().includes('not running'))) {
        setLastError(errMsg);
      }
    }
  }, [config.httpServerEnabled, transport.invoke]);

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

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
  };

  const getStatusLabel = (status: ApiServerJobStatus) => {
    if (status === 'Pending') return <span className="badge badge-pending">Pending</span>;
    if (status === 'Processing') return <span className="badge badge-processing">Processing</span>;
    if (typeof status === 'object') {
      if ('Completed' in status) return <span className="badge badge-completed">Completed</span>;
      if ('Failed' in status)
        return (
          <span className="badge badge-failed" title={status.Failed}>
            Failed
          </span>
        );
    }
    return <span className="badge">Unknown</span>;
  };

  const handleHostChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setConfig({ httpServerHost: e.target.value });
  };

  const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const port = parseInt(e.target.value, 10);
    if (!Number.isNaN(port)) {
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
    navigator.clipboard
      .writeText(key)
      .then(() => {
        setCopied(true);
      })
      .catch((err) => {
        logger.error('[ApiServer] Failed to copy API key:', err);
      });
  }, [config.httpServerApiKey]);

  useEffect(() => {
    const timer = setTimeout(() => {
      if (config.httpServerEnabled) {
        transport
          .invoke<string>(TauriCommand.apiServer.start, {
            host: config.httpServerHost ?? '127.0.0.1',
            port: config.httpServerPort ?? 14200,
            apiKey: config.httpServerApiKey ?? '',
            maxConcurrent: config.httpServerMaxConcurrent ?? 2,
            maxQueueSize: config.httpServerMaxQueueSize ?? 100,
            maxUploadSizeMb: config.httpServerMaxUploadSizeMB ?? 50,
            jobTtlMinutes: config.httpServerJobTtlMinutes ?? 60,
            maxStreaming: 2,
            ipWhitelist: config.httpServerIpWhitelist ?? 'localhost',
            gpuAcceleration: config.gpuAcceleration ?? 'auto',
          })
          .then((normalizedWhitelist) => {
            setLastError(null);
            void fetchData();
            if (
              typeof normalizedWhitelist === 'string' &&
              normalizedWhitelist !== config.httpServerIpWhitelist
            ) {
              setConfig({ httpServerIpWhitelist: normalizedWhitelist });
            }
          })
          .catch((e) => {
            logger.error('[ApiServer] Failed to start server:', e);
            const errMsg = extractErrorMessage(e);
            setLastError(errMsg);
            setHealth(null);
            setJobs({});
            setInfo(null);
            useDialogStore.getState().alert(errMsg, {
              title: t('settings.api_server.start_error', {
                defaultValue: 'Failed to start API Server',
              }),
              variant: 'error',
            });
          });
      } else {
        setLastError(null);
        transport.invoke(TauriCommand.apiServer.stop, { force: true }).catch((e) => {
          logger.error('[ApiServer] Failed to stop server:', e);
        });
      }
    }, 1000);
    return () => clearTimeout(timer);
  }, [
    config.httpServerEnabled,
    config.httpServerHost,
    config.httpServerPort,
    config.httpServerApiKey,
    config.httpServerMaxConcurrent,
    config.httpServerMaxQueueSize,
    config.httpServerMaxUploadSizeMB,
    config.httpServerJobTtlMinutes,
    config.httpServerIpWhitelist,
    config.gpuAcceleration,
    fetchData,
    setConfig,
    t,
    transport.invoke,
  ]);
  const handleToggleEnabled = async (checked: boolean) => {
    if (!checked) {
      try {
        const activeInfo = await transport.invoke<{
          hasActive: boolean;
          processing?: number;
          pending?: number;
        }>(TauriCommand.apiServer.hasActiveJobs);
        if (activeInfo?.hasActive) {
          const count = (activeInfo.processing ?? 0) + (activeInfo.pending ?? 0);
          const confirmed = await useDialogStore.getState().confirm(
            t('settings.api_server.stop_warning_message', {
              defaultValue: `There are still ${count} active transcription task(s). Stopping the API server will terminate these tasks. Are you sure you want to stop?`,
              count,
            }),
            {
              title: t('settings.api_server.stop_warning_title', {
                defaultValue: 'Active Tasks Running',
              }),
              variant: 'warning',
              confirmLabel: t('settings.api_server.stop_confirm', {
                defaultValue: 'Stop Server',
              }),
              cancelLabel: t('common.cancel', { defaultValue: 'Cancel' }),
            }
          );
          if (!confirmed) {
            return;
          }
        }
      } catch (e) {
        logger.error('[ApiServer] Failed to check active jobs:', e);
      }
    }
    setConfig({ httpServerEnabled: checked });
  };

  const isEnabled = Boolean(config.httpServerEnabled);
  const isRunning = Boolean(health);
  const isError = isEnabled && Boolean(lastError && !health);
  const statusTone: ToolHeroStatusTone = !isEnabled
    ? 'inactive'
    : isError
      ? 'error'
      : isRunning
        ? 'active'
        : 'inactive';
  const statusText = !isEnabled
    ? t('settings.api_server.state_stopped', { defaultValue: 'Stopped' })
    : isError
      ? t('common.error', { defaultValue: 'Error' })
      : isRunning
        ? `${t('settings.api_server.state_running', { defaultValue: 'Running' })} :${config.httpServerPort ?? 14200}`
        : t('settings.api_server.state_stopped', { defaultValue: 'Stopped' });
  return (
    <SettingsTabContainer id="tools-panel-api_server" ariaLabelledby="tools-tab-api_server">
      <ToolHeroHeader
        id="settings-api-server-switch"
        icon={<Server size={24} />}
        title={t('settings.api_server.title', { defaultValue: 'API Server' })}
        description={t('settings.api_server.description', {
          defaultValue:
            'Expose an HTTP API for external headless integration with Sona. Features like live recording and batch transcription can be invoked via HTTP.',
        })}
        statusText={statusText}
        statusTone={statusTone}
        enabled={isEnabled}
        onToggle={handleToggleEnabled}
        switchAriaLabel={t('settings.api_server.enable_label', {
          defaultValue: 'Enable API Server',
        })}
        actions={
          isEnabled ? (
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={handleCopyEndpoint}
              title={t('tools.copy_api_url', { defaultValue: 'Copy Endpoint' })}
              style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}
            >
              {copiedEndpoint ? <Check size={13} /> : <Copy size={13} />}
              <span>
                {copiedEndpoint
                  ? t('settings.api_server.copied', { defaultValue: 'Copied!' })
                  : t('tools.copy_api_url', { defaultValue: 'Copy Endpoint' })}
              </span>
            </button>
          ) : undefined
        }
      />

      {isEnabled && (
        <SettingsSection
          title={t('settings.api_server.status_title', { defaultValue: 'Server Status' })}
          icon={<Activity size={20} />}
        >
          <div className="settings-api-server-panel">
            <div className="api-server-status-grid">
              <div className="status-card">
                <div className="status-card-icon">
                  <Activity size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_state', { defaultValue: 'State' })}
                  </div>
                  <div className={`status-card-value ${health ? 'text-success' : 'text-error'}`}>
                    {health
                      ? t('settings.api_server.state_running', { defaultValue: 'Running' })
                      : t('settings.api_server.state_stopped', { defaultValue: 'Stopped' })}
                  </div>
                </div>
              </div>
              <div className="status-card">
                <div className="status-card-icon">
                  <Clock size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_uptime', { defaultValue: 'Uptime' })}
                  </div>
                  <div className="status-card-value">
                    {health ? formatUptime(health.uptime) : '-'}
                  </div>
                </div>
              </div>
              <div className="status-card">
                <div className="status-card-icon">
                  <Zap size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_gpu', {
                      defaultValue: 'GPU Acceleration',
                    })}
                  </div>
                  <div className="status-card-value">
                    {info?.gpuAvailable
                      ? t('common.enabled', { defaultValue: 'Enabled' })
                      : t('common.disabled', { defaultValue: 'Disabled' })}
                  </div>
                </div>
              </div>
              <div className="status-card">
                <div className="status-card-icon">
                  <Activity size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_active_jobs', {
                      defaultValue: 'Active Tasks',
                    })}
                  </div>
                  <div className="status-card-value">{health ? health.activeJobs : '-'}</div>
                </div>
              </div>
              <div className="status-card">
                <div className="status-card-icon">
                  <List size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_pending_jobs', {
                      defaultValue: 'Pending Tasks',
                    })}
                  </div>
                  <div className="status-card-value">{health ? health.pendingJobs : '-'}</div>
                </div>
              </div>
              <div className="status-card">
                <div className="status-card-icon">
                  <HardDrive size={20} />
                </div>
                <div className="status-card-content">
                  <div className="status-card-label">
                    {t('settings.api_server.status_label_cache_space', {
                      defaultValue: 'Cache Usage',
                    })}
                  </div>
                  <div className="status-card-value">
                    {health ? formatBytes(health.cacheSpaceBytes) : '-'}
                  </div>
                </div>
              </div>
            </div>
            {lastError && !health && (
              <div className="settings-error-notice" style={{ marginTop: '12px' }}>
                {t('settings.api_server.connection_error', { defaultValue: 'Connection Error' })}:{' '}
                {lastError}
              </div>
            )}
          </div>
        </SettingsSection>
      )}

      <SettingsSection
        title={t('settings.api_server.network_security_title', {
          defaultValue: 'Network & Security',
        })}
        icon={<Globe size={20} />}
      >
        {/* Host */}
        <SettingsItem
          title={t('settings.api_server.host_label', { defaultValue: 'Host' })}
          hint={t('settings.api_server.host_hint', {
            defaultValue:
              "Bind address for the server. '127.0.0.1' restricts access to localhost, while '0.0.0.0' allows remote access.",
          })}
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
          hint={t('settings.api_server.port_hint', {
            defaultValue:
              'TCP port for the API server. Must be between 1 and 65535 (default: 14200).',
          })}
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

        {/* IP Whitelist */}
        <SettingsItem
          title={t('settings.api_server.ip_whitelist_label', { defaultValue: 'IP Whitelist' })}
          hint={t('settings.api_server.ip_whitelist_hint', {
            defaultValue:
              "Comma-separated list of allowed IPs, CIDRs, or wildcards (e.g. '192.168.1.*'). Use 'localhost' to only allow local access.",
          })}
        >
          <input
            type="text"
            className="input-text"
            value={config.httpServerIpWhitelist ?? 'localhost'}
            onChange={(e) => setConfig({ httpServerIpWhitelist: e.target.value })}
            style={{ width: '400px' }}
          />
        </SettingsItem>

        {/* API Key */}
        <SettingsItem
          id="settings-api-server-auth"
          title={t('settings.api_server.api_key_label', { defaultValue: 'API Key' })}
          hint={t('settings.api_server.api_key_hint', {
            defaultValue: 'Optional Bearer token for authenticating HTTP requests.',
          })}
        >
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <input
              type="password"
              className="input-text"
              value={config.httpServerApiKey ?? ''}
              onChange={handleKeyChange}
              style={{ width: '280px' }}
              placeholder={t('settings.api_server.api_key_placeholder', {
                defaultValue: 'Optional bearer token',
              })}
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
              data-tooltip={
                copied
                  ? t('settings.api_server.copied', { defaultValue: 'Copied!' })
                  : t('settings.api_server.copy_key', { defaultValue: 'Copy' })
              }
              data-tooltip-pos="top"
              aria-label={t('settings.api_server.copy_key', { defaultValue: 'Copy' })}
            >
              <Copy size={16} />
            </button>
          </div>
        </SettingsItem>

        {/* Max Concurrent Tasks */}
        <SettingsItem
          title={t('settings.api_server.max_concurrent_label', {
            defaultValue: 'Max Concurrent Tasks',
          })}
          hint={t('settings.api_server.max_concurrent_hint', {
            defaultValue: 'Number of concurrent transcription tasks allowed (default: 2).',
          })}
        >
          <input
            type="number"
            className="input-text"
            value={config.httpServerMaxConcurrent ?? 2}
            onChange={(e) =>
              setConfig({ httpServerMaxConcurrent: parseInt(e.target.value, 10) || 2 })
            }
            min={1}
            max={32}
            style={{ width: '200px' }}
          />
        </SettingsItem>

        {/* Max Queue Size */}
        <SettingsItem
          title={t('settings.api_server.max_queue_size_label', { defaultValue: 'Max Queue Size' })}
          hint={t('settings.api_server.max_queue_size_hint', {
            defaultValue:
              'Maximum number of tasks allowed in the queue (0 = unlimited, default: 100).',
          })}
        >
          <input
            type="number"
            className="input-text"
            value={config.httpServerMaxQueueSize ?? 100}
            onChange={(e) =>
              setConfig({ httpServerMaxQueueSize: parseInt(e.target.value, 10) || 0 })
            }
            min={0}
            max={1000}
            style={{ width: '200px' }}
          />
        </SettingsItem>

        {/* Max Upload Size */}
        <SettingsItem
          title={t('settings.api_server.max_upload_size_label', {
            defaultValue: 'Max Upload Size (MB)',
          })}
          hint={t('settings.api_server.max_upload_size_hint', {
            defaultValue:
              'Maximum file size allowed for API uploads (0 = unlimited, default: 50MB).',
          })}
        >
          <input
            type="number"
            className="input-text"
            value={config.httpServerMaxUploadSizeMB ?? 50}
            onChange={(e) =>
              setConfig({ httpServerMaxUploadSizeMB: parseInt(e.target.value, 10) || 0 })
            }
            min={0}
            max={10000}
            style={{ width: '200px' }}
          />
        </SettingsItem>

        {/* Job TTL */}
        <SettingsItem
          title={t('settings.api_server.job_ttl_label', { defaultValue: 'Job TTL (Minutes)' })}
          hint={t('settings.api_server.job_ttl_hint', {
            defaultValue:
              'Time to keep completed or failed jobs before auto-cleanup (0 = unlimited, default: 60).',
          })}
        >
          <input
            type="number"
            className="input-text"
            value={config.httpServerJobTtlMinutes ?? 60}
            onChange={(e) =>
              setConfig({ httpServerJobTtlMinutes: parseInt(e.target.value, 10) || 0 })
            }
            min={0}
            max={10080}
            style={{ width: '200px' }}
          />
        </SettingsItem>
      </SettingsSection>

      {isEnabled && (
        <SettingsSection title={t('settings.api_server.jobs_title', { defaultValue: 'Job Queue' })}>
          <div className="settings-api-server-panel">
            <div className="api-server-jobs-container">
              {Object.keys(jobs || {}).length === 0 ? (
                <div className="empty-state-mini">
                  <p>
                    {t('settings.api_server.no_jobs', {
                      defaultValue: 'No active or recent jobs.',
                    })}
                  </p>
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
                    {Object.entries(jobs || {}).map(([id, status]) => (
                      <tr key={id}>
                        <td className="job-id-cell" title={id}>
                          {id.slice(0, 8)}...
                        </td>
                        <td>{getStatusLabel(status)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        </SettingsSection>
      )}
    </SettingsTabContainer>
  );
}

export default SettingsApiServerTab;
