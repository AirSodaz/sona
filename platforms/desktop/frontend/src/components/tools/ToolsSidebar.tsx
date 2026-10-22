import { Bot, Keyboard, Server, Subtitles, Wrench } from 'lucide-react';
import type React from 'react';
import { useTranslation } from 'react-i18next';
import { useVoiceTypingReadiness } from '../../hooks/useVoiceTypingReadiness';
import { useApiServerConfig, useMcpConfig, useVoiceTypingConfig } from '../../stores/configStore';
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
  statusType: 'active' | 'warning' | 'inactive';
}

export function ToolsSidebar(): React.JSX.Element {
  const { t } = useTranslation();
  const activeTool = useToolsStore((state) => state.activeTool);
  const setActiveTool = useToolsStore((state) => state.setActiveTool);

  const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);
  const vtConfig = useVoiceTypingConfig();
  const readiness = useVoiceTypingReadiness();
  const apiConfig = useApiServerConfig();
  const mcpConfig = useMcpConfig();
  const mcpStatus = useMcpStore((state) => state.status);

  const isVtActive = Boolean(vtConfig.voiceTypingEnabled);
  const isApiActive = Boolean(apiConfig.httpServerEnabled);
  const isMcpActive = Boolean(mcpConfig.mcpEnabled);

  const activeCount =
    (isCaptionMode ? 1 : 0) + (isVtActive ? 1 : 0) + (isApiActive ? 1 : 0) + (isMcpActive ? 1 : 0);

  const tools: ToolItemDef[] = [
    {
      id: 'subtitles',
      nameKey: 'tools.subtitles',
      defaultName: 'Live Subtitles',
      descKey: 'tools.subtitles_desc',
      defaultDesc: 'Desktop floating bilingual subtitles',
      Icon: Subtitles,
      statusText: isCaptionMode
        ? t('tools.status_running', { defaultValue: 'Running' })
        : t('tools.status_stopped', { defaultValue: 'Stopped' }),
      statusType: isCaptionMode ? 'active' : 'inactive',
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
          : t('tools.status_needs_config', { defaultValue: 'Needs Config' }),
      statusType: !isVtActive ? 'inactive' : readiness.state === 'ready' ? 'active' : 'warning',
    },
    {
      id: 'api_server',
      nameKey: 'tools.api_server',
      defaultName: 'API Server',
      descKey: 'tools.api_server_desc',
      defaultDesc: 'Local HTTP audio transcription API',
      Icon: Server,
      statusText: isApiActive
        ? `:${apiConfig.httpServerPort || 3000}`
        : t('tools.status_stopped', { defaultValue: 'Stopped' }),
      statusType: isApiActive ? 'active' : 'inactive',
    },
    {
      id: 'mcp',
      nameKey: 'tools.mcp',
      defaultName: 'MCP Agents',
      descKey: 'tools.mcp_desc',
      defaultDesc: 'Model Context Protocol agent server',
      Icon: Bot,
      statusText: !isMcpActive
        ? t('tools.status_disabled', { defaultValue: 'Disabled' })
        : mcpStatus?.activeConnections && mcpStatus.activeConnections > 0
          ? `${mcpStatus.activeConnections} ${t('tools.clients_unit', { defaultValue: 'clients' })}`
          : t('tools.status_running', { defaultValue: 'Running' }),
      statusType: isMcpActive ? 'active' : 'inactive',
    },
  ];

  return (
    <aside
      className="tools-sidebar panel-left"
      aria-label={t('tools.title', { defaultValue: 'Tools' })}
    >
      <div className="tools-sidebar-header">
        <div className="tools-sidebar-title-row">
          <Wrench size={18} className="tools-sidebar-header-icon" aria-hidden="true" />
          <h2 className="tools-sidebar-title">{t('tools.title', { defaultValue: 'Tools' })}</h2>
        </div>
        <p className="tools-sidebar-subtitle">
          {t('tools.active_count', {
            count: activeCount,
            defaultValue: `${activeCount} services running`,
          })}
        </p>
      </div>

      <div className="tools-sidebar-nav" role="tablist" aria-orientation="vertical">
        {tools.map((tool) => {
          const isSelected = activeTool === tool.id;
          const Icon = tool.Icon;

          return (
            <button
              key={tool.id}
              type="button"
              role="tab"
              aria-selected={isSelected}
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
                  <span className={`tools-status-badge tools-status-${tool.statusType}`}>
                    <span className="tools-status-dot" aria-hidden="true" />
                    <span>{tool.statusText}</span>
                  </span>
                </div>
                <span className="tools-sidebar-item-desc">
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
