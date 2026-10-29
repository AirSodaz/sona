import { Bot, Keyboard, Server, Subtitles, Tv } from 'lucide-react';
import type React from 'react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../../services/asrConfigService';
import { getEmbyConfig, testEmbyConnection } from '../../services/emby/embyService';
import {
  useApiServerConfig,
  useConfigStore,
  useMcpConfig,
  useVoiceTypingConfig,
} from '../../stores/configStore';
import { useMcpStore } from '../../stores/mcpStore';
import { useToolsStore } from '../../stores/toolsStore';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import type { ToolId } from '../../types/tools';

interface ToolItemDef {
  id: ToolId;
  nameKey: string;
  defaultName: string;
  descKey: string;
  defaultDesc: string;
  Icon: React.ComponentType<{ size?: number; className?: string }>;
  statusText: string;
  statusType: 'active' | 'warning' | 'inactive' | 'error';
}

export function ToolsSidebar(): React.JSX.Element {
  const { t } = useTranslation();
  const activeTool = useToolsStore((state) => state.activeTool);
  const setActiveTool = useToolsStore((state) => state.setActiveTool);

  const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);
  const globalConfig = useConfigStore((state) => state.config);
  const isCaptionConfigured = isAsrRequestConfigured(
    resolveAsrTranscriptionRequest(globalConfig, 'caption')
  );
  const vtConfig = useVoiceTypingConfig();
  const readiness = useVoiceTypingReadiness();
  const apiConfig = useApiServerConfig();
  const mcpConfig = useMcpConfig();
  const mcpStatus = useMcpStore((state) => state.status);

  const isVtActive = Boolean(vtConfig.voiceTypingEnabled);
  const isApiActive = Boolean(apiConfig.httpServerEnabled);
  const isMcpEnabled = Boolean(mcpConfig.mcpEnabled ?? mcpStatus?.enabled ?? true);
  const isMcpRunning = Boolean(mcpStatus?.running);
  useEffect(() => {
    if (!mcpStatus) {
      void useMcpStore.getState().fetchStatus();
    }
  }, [mcpStatus]);
  const [embyServerName, setEmbyServerName] = useState<string | null>(null);
  useEffect(() => {
    let unmounted = false;
    if (activeTool) {
      void getEmbyConfig().then((cfg) => {
        if (unmounted) return;
        if (cfg?.serverUrl && cfg.apiKey) {
          void testEmbyConnection(cfg)
            .then((info) => {
              if (!unmounted) setEmbyServerName(info.serverName);
            })
            .catch(() => {
              if (!unmounted) setEmbyServerName(null);
            });
        } else {
          if (!unmounted) setEmbyServerName(null);
        }
      });
    }
    return () => {
      unmounted = true;
    };
  }, [activeTool]);
  const tools: ToolItemDef[] = [
    {
      id: 'subtitles',
      nameKey: 'tools.subtitles',
      defaultName: 'Live Subtitles',
      descKey: 'tools.subtitles_desc',
      defaultDesc: 'Desktop floating bilingual subtitles',
      Icon: Subtitles,
      statusText: !isCaptionMode
        ? t('tools.status_stopped', { defaultValue: 'Stopped' })
        : !isCaptionConfigured
          ? t('tools.status_needs_config', { defaultValue: 'Needs Config' })
          : t('tools.status_running', { defaultValue: 'Running' }),
      statusType: !isCaptionMode ? 'inactive' : !isCaptionConfigured ? 'warning' : 'active',
    },
    {
      id: 'voice_typing',
      nameKey: 'tools.voice_typing',
      defaultName: 'Voice Typing',
      descKey: 'tools.voice_typing_desc',
      defaultDesc: 'System-wide dictation and smart polishing',
      Icon: Keyboard,
      statusText: !isVtActive
        ? t('tools.status_disabled', { defaultValue: 'Disabled' })
        : readiness.state === 'ready'
          ? t('tools.status_ready', { defaultValue: 'Ready' })
          : readiness.state === 'failed'
            ? t('common.error', { defaultValue: 'Error' })
            : readiness.state === 'preparing'
              ? t('common.preparing', { defaultValue: 'Preparing...' })
              : t('tools.status_needs_config', { defaultValue: 'Needs Config' }),
      statusType: !isVtActive
        ? 'inactive'
        : readiness.state === 'ready'
          ? 'active'
          : readiness.state === 'failed'
            ? 'error'
            : 'warning',
    },
    {
      id: 'api_server',
      nameKey: 'tools.api_server',
      defaultName: 'API Server',
      descKey: 'tools.api_server_desc',
      defaultDesc: 'Local HTTP audio transcription API',
      Icon: Server,
      statusText: isApiActive
        ? `:${apiConfig.httpServerPort ?? 14200}`
        : t('tools.status_disabled', { defaultValue: 'Disabled' }),
      statusType: isApiActive ? 'active' : 'inactive',
    },
    {
      id: 'mcp',
      nameKey: 'tools.mcp',
      defaultName: 'MCP Agents',
      descKey: 'tools.mcp_desc',
      defaultDesc: 'Model Context Protocol agent server',
      Icon: Bot,
      statusText: !isMcpEnabled
        ? t('tools.status_disabled', { defaultValue: 'Disabled' })
        : !isMcpRunning
          ? t('tools.status_stopped', { defaultValue: 'Stopped' })
          : mcpStatus?.activeConnections && mcpStatus.activeConnections > 0
            ? `${mcpStatus.activeConnections} ${
                mcpStatus.activeConnections === 1
                  ? t('tools.clients_unit_singular', { defaultValue: 'client' })
                  : t('tools.clients_unit', { defaultValue: 'clients' })
              }`
            : t('tools.status_running', { defaultValue: 'Running' }),
      statusType: !isMcpEnabled ? 'inactive' : isMcpRunning ? 'active' : 'warning',
    },
    {
      id: 'emby_sync',
      nameKey: 'tools.emby_sync',
      defaultName: 'Emby Sync',
      descKey: 'tools.emby_sync_desc',
      defaultDesc: 'Transcribe and sync subtitles to Emby',
      Icon: Tv,
      statusText: embyServerName
        ? embyServerName
        : t('tools.status_not_connected', { defaultValue: 'Not Connected' }),
      statusType: embyServerName ? 'active' : 'inactive',
    },
  ];
  const resolvedTool: ToolId =
    activeTool === 'subtitles' ||
    activeTool === 'voice_typing' ||
    activeTool === 'api_server' ||
    activeTool === 'mcp' ||
    activeTool === 'emby_sync'
      ? activeTool
      : 'subtitles';
  const selectedIndex = tools.findIndex((tool) => tool.id === resolvedTool);
  const effectiveIndex = Math.max(selectedIndex, 0);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    const currentIndex = Math.max(selectedIndex, 0);

    let nextIndex = currentIndex;
    if (e.key === 'ArrowDown') {
      nextIndex = (currentIndex + 1) % tools.length;
    } else if (e.key === 'ArrowUp') {
      nextIndex = (currentIndex - 1 + tools.length) % tools.length;
    } else if (e.key === 'Home') {
      nextIndex = 0;
    } else if (e.key === 'End') {
      nextIndex = tools.length - 1;
    }

    const nextTool = tools[nextIndex];
    if (nextTool) {
      setActiveTool(nextTool.id);
      document.getElementById(`tools-tab-${nextTool.id}`)?.focus();
    }
  };

  return (
    <aside
      className="tools-sidebar panel-left"
      aria-label={t('tools.sidebar_title', { defaultValue: 'Tools sidebar' })}
    >
      <div
        className="tools-sidebar-nav"
        role="tablist"
        aria-label={t('tools.tabs_title', { defaultValue: 'Tools list' })}
        aria-orientation="vertical"
        onKeyDown={handleKeyDown}
      >
        {tools.map((tool, index) => {
          const isSelected = resolvedTool === tool.id;
          const Icon = tool.Icon;

          return (
            <button
              key={tool.id}
              id={`tools-tab-${tool.id}`}
              type="button"
              role="tab"
              aria-label={t(tool.nameKey, { defaultValue: tool.defaultName })}
              aria-describedby={`tools-tab-status-${tool.id} tools-tab-desc-${tool.id}`}
              aria-selected={isSelected}
              aria-controls={`tools-panel-${tool.id}`}
              tabIndex={index === effectiveIndex ? 0 : -1}
              className={`tools-sidebar-item ${isSelected ? 'active' : ''}`}
              onClick={() => setActiveTool(tool.id)}
            >
              <div className="tools-sidebar-item-icon">
                <Icon size={18} />
              </div>
              <div className="tools-sidebar-item-content">
                <div className="tools-sidebar-item-main">
                  <span className="tools-sidebar-item-name">
                    {t(tool.nameKey, { defaultValue: tool.defaultName })}
                  </span>
                  <span
                    id={`tools-tab-status-${tool.id}`}
                    className={`tools-status-badge tools-status-${tool.statusType}`}
                  >
                    <span className="tools-status-dot" aria-hidden="true" />
                    <span>{tool.statusText}</span>
                  </span>
                </div>
                <span id={`tools-tab-desc-${tool.id}`} className="tools-sidebar-item-desc">
                  {t(tool.descKey, { defaultValue: tool.defaultDesc })}
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
