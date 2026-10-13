import {
  Activity,
  AlertTriangle,
  Bot,
  Check,
  CheckCircle2,
  Copy,
  ExternalLink,
  Power,
  RefreshCw,
  Search,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  XCircle,
  Zap,
} from 'lucide-react';
import type React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMcpConfig, useSetConfig } from '../../stores/configStore';
import { useMcpStore } from '../../stores/mcpStore';
import type { McpPermissionMode } from '../../types/config';
import { Switch } from '../Switch';
import {
  SettingsAccordion,
  SettingsItem,
  SettingsPageHeader,
  SettingsSection,
  SettingsTabContainer,
} from './SettingsLayout';
import './SettingsMcpTab.css';

export function SettingsMcpTab(): React.JSX.Element {
  const { t } = useTranslation();
  const config = useMcpConfig();
  const setConfig = useSetConfig();

  const {
    status,
    logs,
    tools,
    clientConfigs,
    filter,
    fetchStatus,
    toggleEnabled,
    disconnectAll,
    fetchLogs,
    clearLogs,
    fetchTools,
    fetchClientConfigs,
    setFilterStatus,
    setFilterQuery,
  } = useMcpStore();

  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [selectedGuideId, setSelectedGuideId] = useState<string>('claude-desktop');
  const [pingLatency, setPingLatency] = useState<number | null>(null);
  const [isPinging, setIsPinging] = useState(false);

  useEffect(() => {
    void fetchStatus();
    void fetchLogs();
    void fetchTools();
    void fetchClientConfigs();
  }, [fetchStatus, fetchLogs, fetchTools, fetchClientConfigs]);

  const handleCopy = useCallback((text: string, key: string) => {
    void navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey(null);
    }, 2000);
  }, []);

  const handleToggleEnabled = useCallback(
    async (nextEnabled: boolean) => {
      setConfig({ mcpEnabled: nextEnabled });
      await toggleEnabled(nextEnabled);
    },
    [setConfig, toggleEnabled]
  );

  const handleRestart = useCallback(async () => {
    await toggleEnabled(false);
    setTimeout(() => {
      void toggleEnabled(true);
    }, 200);
  }, [toggleEnabled]);

  const handleModeChange = useCallback(
    (mode: McpPermissionMode) => {
      setConfig({ mcpPermissionMode: mode });
      setTimeout(() => {
        void fetchTools();
        void fetchStatus();
      }, 50);
    },
    [setConfig, fetchTools, fetchStatus]
  );

  const handlePing = useCallback(async () => {
    setIsPinging(true);
    const start = performance.now();
    try {
      await fetchStatus();
      const elapsed = Math.round((performance.now() - start) * 10) / 10;
      setPingLatency(elapsed);
    } catch {
      setPingLatency(-1);
    } finally {
      setIsPinging(false);
    }
  }, [fetchStatus]);

  const filteredLogs = useMemo(() => {
    return (logs || []).filter((entry) => {
      if (filter?.status && filter.status !== 'all' && entry.status !== filter.status) {
        return false;
      }
      if (filter?.query?.trim()) {
        const q = filter.query.toLowerCase();
        const matchMethod = entry.method.toLowerCase().includes(q);
        const matchSummary = entry.paramsSummary?.toLowerCase().includes(q) ?? false;
        const matchError = entry.errorMessage?.toLowerCase().includes(q) ?? false;
        return matchMethod || matchSummary || matchError;
      }
      return true;
    });
  }, [logs, filter]);

  const activeGuide = useMemo(() => {
    if (!clientConfigs?.guides) return null;
    return (
      clientConfigs.guides.find((g) => g.id === selectedGuideId) ?? clientConfigs.guides[0] ?? null
    );
  }, [clientConfigs, selectedGuideId]);

  const currentMode = config.mcpPermissionMode ?? status?.permissionMode ?? 'full';

  return (
    <SettingsTabContainer id="settings-panel-mcp" ariaLabelledby="settings-tab-mcp">
      <SettingsPageHeader
        icon={<Bot width={28} height={28} />}
        title={t('settings.mcp.title', { defaultValue: 'MCP Agents' })}
        description={t('settings.mcp.description', {
          defaultValue:
            'Securely expose transcription, recording control, and historical data to external AI Agents (Claude Desktop, Cursor, Windsurf, Zed) via Model Context Protocol.',
        })}
      />

      {/* 1. Service Lifecycle & Status */}
      <SettingsSection
        title={t('settings.mcp.status_title', { defaultValue: 'Service Lifecycle & Status' })}
        icon={<Activity size={18} />}
      >
        <SettingsItem
          title={t('settings.mcp.enable_label', { defaultValue: 'Enable MCP Agent Service' })}
          hint={t('settings.mcp.enable_hint', {
            defaultValue:
              'Start the local IPC listener (Named Pipe / Unix Domain Socket) allowing external AI Agents to control Sona.',
          })}
        >
          <Switch
            checked={config.mcpEnabled ?? status?.enabled ?? true}
            onChange={handleToggleEnabled}
            aria-label={t('settings.mcp.enable_label', {
              defaultValue: 'Enable MCP Agent Service',
            })}
          />
        </SettingsItem>

        <SettingsItem
          title={t('settings.mcp.status_listening', { defaultValue: 'Service Status & Clients' })}
          hint={
            status?.running
              ? t('settings.mcp.status_active_hint', {
                  defaultValue:
                    'Local IPC service is running in background and accepting connections.',
                })
              : t('settings.mcp.status_stopped_hint', {
                  defaultValue: 'Service is stopped. External agents cannot connect.',
                })
          }
        >
          <div className="mcp-actions-row">
            <span className={`badge ${status?.running ? 'badge-completed' : 'badge-pending'}`}>
              <span className={`mcp-status-dot ${status?.running ? 'active' : ''}`} />
              {status?.running
                ? t('settings.mcp.status_listening', { defaultValue: 'Listening' })
                : t('settings.mcp.status_stopped', { defaultValue: 'Stopped' })}
            </span>
            <span className="badge badge-subtle">
              {t('settings.mcp.active_clients_count', {
                count: status?.activeConnections ?? 0,
                defaultValue: `${status?.activeConnections ?? 0} Agent connected`,
              })}
            </span>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              disabled={!status?.running || (status?.activeConnections ?? 0) === 0}
              onClick={() => void disconnectAll()}
              title={t('settings.mcp.btn_disconnect_all', { defaultValue: 'Disconnect All' })}
            >
              <Power size={13} />
              {t('settings.mcp.btn_disconnect_all', { defaultValue: 'Disconnect All' })}
            </button>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              disabled={!config.mcpEnabled}
              onClick={() => void handleRestart()}
              title={t('settings.mcp.btn_restart_service', { defaultValue: 'Restart Service' })}
            >
              <RefreshCw size={13} />
              {t('settings.mcp.btn_restart_service', { defaultValue: 'Restart Service' })}
            </button>
          </div>
        </SettingsItem>

        <SettingsItem
          title={t('settings.mcp.ipc_endpoint', { defaultValue: 'IPC Endpoint' })}
          hint={t('settings.mcp.ipc_endpoint_hint', {
            defaultValue: 'Unified local IPC endpoint used by external agent processes.',
          })}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <code className="settings-code-inline">{status?.endpoint || 'N/A'}</code>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => handleCopy(status?.endpoint || '', 'endpoint')}
            >
              {copiedKey === 'endpoint' ? <Check size={13} /> : <Copy size={13} />}
              {copiedKey === 'endpoint'
                ? t('settings.mcp.copied', { defaultValue: 'Copied!' })
                : t('settings.mcp.btn_copy_config', { defaultValue: 'Copy' })}
            </button>
          </div>
        </SettingsItem>

        <SettingsItem
          title={t('settings.mcp.bridge_binary', {
            defaultValue: 'Bridge Executable (sona-mcp)',
          })}
          hint={
            status?.binaryPath ||
            t('settings.mcp.bridge_binary_hint', {
              defaultValue: 'Standard MCP Stdio protocol bridge executable',
            })
          }
        >
          <div>
            {status?.binaryExists ? (
              <span
                className="badge badge-completed"
                style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
              >
                <CheckCircle2 size={13} />
                {t('settings.mcp.bridge_ready', { defaultValue: 'Ready' })}
              </span>
            ) : (
              <span
                className="badge badge-failed"
                style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
              >
                <AlertTriangle size={13} />
                {t('settings.mcp.bridge_missing', { defaultValue: 'Executable not found' })}
              </span>
            )}
          </div>
        </SettingsItem>
      </SettingsSection>

      {/* 2. Security & Permissions Policy */}
      <SettingsSection
        title={t('settings.mcp.security_title', { defaultValue: 'Security & Permissions' })}
        icon={<Shield size={18} />}
      >
        <SettingsItem
          layout="vertical"
          title={t('settings.mcp.mode_title', { defaultValue: 'Authorization Mode' })}
          hint={t('settings.mcp.mode_hint', {
            defaultValue:
              'Control permission boundaries and allowable tool executions for external agents.',
          })}
        >
          <div
            className="settings-scenario-cards three-columns"
            style={{
              width: '100%',
              padding: '0 0 var(--spacing-xs) 0',
              background: 'transparent',
            }}
          >
            <button
              type="button"
              className={`settings-scenario-card ${currentMode === 'full' ? 'active' : ''}`}
              onClick={() => handleModeChange('full')}
            >
              <div className="settings-scenario-card-icon">
                <ShieldCheck size={18} />
              </div>
              <div className="settings-scenario-card-text">
                <div className="settings-scenario-card-label">
                  {t('settings.mcp.mode_full', { defaultValue: 'Full Access' })}
                </div>
                <div className="settings-scenario-card-description">
                  {t('settings.mcp.mode_full_desc', {
                    defaultValue: 'External agents can execute all 40 tools without restriction.',
                  })}
                </div>
              </div>
            </button>

            <button
              type="button"
              className={`settings-scenario-card ${currentMode === 'read_only' ? 'active' : ''}`}
              onClick={() => handleModeChange('read_only')}
            >
              <div className="settings-scenario-card-icon">
                <Shield size={18} />
              </div>
              <div className="settings-scenario-card-text">
                <div className="settings-scenario-card-label">
                  {t('settings.mcp.mode_read_only', { defaultValue: 'Read-Only Protection' })}
                </div>
                <div className="settings-scenario-card-description">
                  {t('settings.mcp.mode_read_only_desc', {
                    defaultValue:
                      'Agents can only read transcripts, query history, and inspect settings. All mutations and recording actions are blocked.',
                  })}
                </div>
              </div>
            </button>

            <button
              type="button"
              className={`settings-scenario-card ${currentMode === 'granular' ? 'active' : ''}`}
              onClick={() => handleModeChange('granular')}
            >
              <div className="settings-scenario-card-icon">
                <ShieldAlert size={18} />
              </div>
              <div className="settings-scenario-card-text">
                <div className="settings-scenario-card-label">
                  {t('settings.mcp.mode_granular', {
                    defaultValue: 'Custom Granular Permissions',
                  })}
                </div>
                <div className="settings-scenario-card-description">
                  {t('settings.mcp.mode_granular_desc', {
                    defaultValue:
                      'Select specific permission categories allowed for external agents.',
                  })}
                </div>
              </div>
            </button>
          </div>
        </SettingsItem>

        {currentMode === 'granular' && (
          <>
            <SettingsItem
              indent={true}
              title={t('settings.mcp.perm_recording', {
                defaultValue: 'Allow audio recording control (start/stop/pause/resume)',
              })}
            >
              <Switch
                checked={config.mcpAllowRecording ?? true}
                onChange={(val) => {
                  setConfig({ mcpAllowRecording: val });
                  setTimeout(() => void fetchTools(), 50);
                }}
              />
            </SettingsItem>

            <SettingsItem
              indent={true}
              title={t('settings.mcp.perm_transcript_edit', {
                defaultValue:
                  'Allow editing transcripts and generating summaries (edit_transcript/patch_segments/save_summary)',
              })}
            >
              <Switch
                checked={config.mcpAllowTranscriptEdit ?? true}
                onChange={(val) => {
                  setConfig({ mcpAllowTranscriptEdit: val });
                  setTimeout(() => void fetchTools(), 50);
                }}
              />
            </SettingsItem>

            <SettingsItem
              indent={true}
              title={
                <span
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                    color: 'var(--color-error)',
                  }}
                >
                  <AlertTriangle size={14} />
                  {t('settings.mcp.perm_destructive', {
                    defaultValue:
                      'Allow destructive actions (delete history/delete project/discard draft)',
                  })}
                </span>
              }
              hint={t('settings.mcp.perm_destructive_warning', {
                defaultValue: 'Warning: Allows agents to permanently delete history and projects',
              })}
            >
              <Switch
                checked={config.mcpAllowDestructive ?? false}
                onChange={(val) => {
                  setConfig({ mcpAllowDestructive: val });
                  setTimeout(() => void fetchTools(), 50);
                }}
              />
            </SettingsItem>

            <SettingsItem
              indent={true}
              title={t('settings.mcp.perm_settings_update', {
                defaultValue: 'Allow updating client settings (update_setting)',
              })}
            >
              <Switch
                checked={config.mcpAllowSettingsUpdate ?? false}
                onChange={(val) => {
                  setConfig({ mcpAllowSettingsUpdate: val });
                  setTimeout(() => void fetchTools(), 50);
                }}
              />
            </SettingsItem>

            <SettingsItem
              indent={true}
              title={t('settings.mcp.perm_model_management', {
                defaultValue: 'Allow model management and downloads (download_preset_model/cancel)',
              })}
            >
              <Switch
                checked={config.mcpAllowModelManagement ?? true}
                onChange={(val) => {
                  setConfig({ mcpAllowModelManagement: val });
                  setTimeout(() => void fetchTools(), 50);
                }}
              />
            </SettingsItem>
          </>
        )}

        <SettingsItem
          title={t('settings.mcp.notify_on_mutation', {
            defaultValue: 'Show desktop notification when agents mutate data or control recording',
          })}
          hint={t('settings.mcp.notify_on_mutation_hint', {
            defaultValue:
              'Show desktop toast notice when an external agent modifies transcripts, controls recording, or updates settings',
          })}
        >
          <Switch
            checked={config.mcpNotifyOnMutation ?? true}
            onChange={(val) => setConfig({ mcpNotifyOnMutation: val })}
          />
        </SettingsItem>
      </SettingsSection>

      {/* 3. Client Integration Guide */}
      <SettingsSection
        title={t('settings.mcp.integration_title', {
          defaultValue: 'Client Integration Guide',
        })}
        description={t('settings.mcp.integration_desc', {
          defaultValue:
            "Copy the generated JSON configuration into your AI agent's config file to connect Sona MCP.",
        })}
        icon={<ExternalLink size={18} />}
      >
        <SettingsItem
          layout="vertical"
          title={t('settings.mcp.select_client', {
            defaultValue: 'Target Client Configuration',
          })}
        >
          <div className="mcp-client-tabs">
            {clientConfigs?.guides.map((guide) => (
              <button
                key={guide.id}
                type="button"
                className={`btn btn-sm ${selectedGuideId === guide.id ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setSelectedGuideId(guide.id)}
              >
                {guide.name}
              </button>
            ))}
          </div>

          {activeGuide && (
            <div style={{ width: '100%', marginTop: 'var(--spacing-sm)' }}>
              <div className="mcp-guide-path-row">
                <span className="mcp-guide-path-label">
                  {t('settings.mcp.config_file_path', { defaultValue: 'Config File Path' })}:
                </span>
                <code className="settings-code-inline">{activeGuide.configPath}</code>
                <button
                  type="button"
                  className="btn btn-secondary btn-xs"
                  onClick={() => handleCopy(activeGuide.configPath, 'guide-path')}
                >
                  {copiedKey === 'guide-path' ? <Check size={12} /> : <Copy size={12} />}
                  {copiedKey === 'guide-path'
                    ? t('settings.mcp.copied', { defaultValue: 'Copied!' })
                    : t('settings.mcp.btn_copy_config', { defaultValue: 'Copy' })}
                </button>
              </div>

              <div className="mcp-code-container">
                <div className="mcp-code-header">
                  <span className="mcp-code-lang">{activeGuide.format.toUpperCase()}</span>
                  <button
                    type="button"
                    className="btn btn-secondary btn-xs"
                    onClick={() => handleCopy(activeGuide.configContent, 'guide-content')}
                  >
                    {copiedKey === 'guide-content' ? <Check size={12} /> : <Copy size={12} />}
                    {copiedKey === 'guide-content'
                      ? t('settings.mcp.copied', { defaultValue: 'Copied!' })
                      : t('settings.mcp.btn_copy_config', {
                          defaultValue: 'Copy Configuration',
                        })}
                  </button>
                </div>
                <pre className="mcp-code-body">{activeGuide.configContent}</pre>
              </div>
            </div>
          )}
        </SettingsItem>
      </SettingsSection>

      {/* 4. Live Activity & Audit Stream */}
      <SettingsSection
        title={t('settings.mcp.audit_log_title', {
          defaultValue: 'Live Activity & Audit Stream',
        })}
        description={t('settings.mcp.audit_log_desc', {
          defaultValue:
            'Real-time log of RPC calls made by external agents, including timing, parameters, and security gate status.',
        })}
        icon={<Activity size={18} />}
        actions={
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            disabled={(logs || []).length === 0}
            onClick={() => void clearLogs()}
          >
            <Trash2 size={13} />
            {t('settings.mcp.btn_clear_logs', { defaultValue: 'Clear Logs' })}
          </button>
        }
      >
        <SettingsItem layout="vertical" title="">
          <div className="mcp-log-toolbar">
            <div className="mcp-log-pills">
              {(['all', 'success', 'blocked', 'error'] as const).map((st) => (
                <button
                  key={st}
                  type="button"
                  className={`btn btn-xs ${filter?.status === st ? 'btn-primary' : 'btn-secondary'}`}
                  onClick={() => setFilterStatus(st)}
                >
                  {t(`settings.mcp.filter_${st}`, { defaultValue: st })}
                </button>
              ))}
            </div>

            <div className="mcp-log-search-box">
              <Search size={14} color="var(--color-text-muted)" />
              <input
                type="text"
                className="mcp-log-search-input"
                placeholder={t('settings.mcp.search_placeholder', {
                  defaultValue: 'Search method or summary...',
                })}
                value={filter?.query || ''}
                onChange={(e) => setFilterQuery(e.target.value)}
              />
            </div>
          </div>

          <div className="mcp-log-table">
            {filteredLogs.length === 0 ? (
              <div className="empty-state-mini">
                {t('settings.mcp.no_logs', { defaultValue: 'No activity logs recorded yet' })}
              </div>
            ) : (
              filteredLogs.map((entry) => (
                <div key={entry.id} className={`mcp-log-entry-row status-${entry.status}`}>
                  <div className="mcp-log-entry-main">
                    <span className="mcp-log-time">
                      {new Date(entry.timestamp).toLocaleTimeString()}
                    </span>
                    <span className="mcp-log-method">{entry.method}</span>
                    <span className="mcp-log-category-tag">{entry.category}</span>
                    {entry.paramsSummary && (
                      <span className="mcp-log-summary-text" title={entry.paramsSummary}>
                        {entry.paramsSummary}
                      </span>
                    )}
                    {entry.errorMessage && (
                      <span
                        className="mcp-log-summary-text"
                        style={{ color: 'var(--color-error)' }}
                        title={entry.errorMessage}
                      >
                        {entry.errorMessage}
                      </span>
                    )}
                  </div>

                  <div className="mcp-log-entry-meta">
                    <span className="mcp-log-duration-label">{entry.durationMs}ms</span>
                    <span
                      className={`badge ${
                        entry.status === 'success'
                          ? 'badge-completed'
                          : entry.status === 'blocked'
                            ? 'badge-failed'
                            : 'badge-pending'
                      }`}
                    >
                      {entry.status === 'success' && <CheckCircle2 size={12} />}
                      {entry.status === 'blocked' && <XCircle size={12} />}
                      {entry.status === 'error' && <AlertTriangle size={12} />}
                      {t(`settings.mcp.filter_${entry.status}`, { defaultValue: entry.status })}
                    </span>
                  </div>
                </div>
              ))
            )}
          </div>
        </SettingsItem>
      </SettingsSection>

      {/* 5. Tool Capability Matrix */}
      <SettingsAccordion
        title={t('settings.mcp.tools_matrix_title', {
          defaultValue: `Tool Capability Matrix (${(tools || []).length} Tools)`,
        })}
        status={
          <span className="badge badge-subtle">
            {(tools || []).filter((t) => t.allowed).length}/{(tools || []).length}{' '}
            {t('settings.mcp.allowed', { defaultValue: 'Allowed' })}
          </span>
        }
        actions={
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {pingLatency !== null && (
              <span
                style={{
                  fontSize: 12,
                  fontFamily: 'monospace',
                  color: pingLatency >= 0 ? 'var(--color-success)' : 'var(--color-error)',
                }}
              >
                {pingLatency >= 0 ? `${pingLatency} ms` : 'Ping failed'}
              </span>
            )}
            <button
              type="button"
              className="btn btn-secondary btn-xs"
              disabled={isPinging || !status?.running}
              onClick={(e) => {
                e.stopPropagation();
                void handlePing();
              }}
            >
              <Zap size={12} />
              Ping
            </button>
          </div>
        }
      >
        <div className="mcp-tools-matrix-grid">
          {(tools || []).map((tool) => (
            <div key={tool.name} className="mcp-tool-unified-card">
              <div className="mcp-tool-card-head">
                <div>
                  <div className="mcp-tool-card-name">{tool.name}</div>
                  <div className="mcp-tool-card-title">{tool.title}</div>
                </div>
                <span className={`badge ${tool.allowed ? 'badge-completed' : 'badge-failed'}`}>
                  {tool.allowed ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
                  {tool.allowed
                    ? t('settings.mcp.allowed', { defaultValue: 'Allowed' })
                    : t('settings.mcp.blocked', { defaultValue: 'Blocked by Policy' })}
                </span>
              </div>
              <div className="mcp-tool-card-desc">{tool.description}</div>
            </div>
          ))}
        </div>
      </SettingsAccordion>
    </SettingsTabContainer>
  );
}

export default SettingsMcpTab;
