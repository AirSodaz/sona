import type React from 'react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useToolsStore } from '../../stores/toolsStore';
import { ToolApiServerPanel } from './panels/ToolApiServerPanel';
import { ToolEmbySyncPanel } from './panels/ToolEmbySyncPanel';
import { ToolMcpPanel } from './panels/ToolMcpPanel';
import { ToolSubtitlesPanel } from './panels/ToolSubtitlesPanel';
import { ToolVoiceTypingPanel } from './panels/ToolVoiceTypingPanel';
import { ToolsSidebar } from './ToolsSidebar';
import './ToolsView.css';

interface ToolsViewProps {
  isActive?: boolean;
}

export function ToolsView({ isActive = true }: ToolsViewProps): React.JSX.Element {
  const { t } = useTranslation();
  const activeTool = useToolsStore((state) => state.activeTool);
  const resolvedTool =
    activeTool === 'subtitles' ||
    activeTool === 'voice_typing' ||
    activeTool === 'api_server' ||
    activeTool === 'mcp' ||
    activeTool === 'emby_sync'
      ? activeTool
      : 'subtitles';
  const detailPaneRef = useRef<HTMLDivElement>(null);
  const prevToolRef = useRef(resolvedTool);

  useEffect(() => {
    if (prevToolRef.current !== resolvedTool) {
      if (isActive && detailPaneRef.current) {
        prevToolRef.current = resolvedTool;
        detailPaneRef.current.scrollTop = 0;
      }
    }
  }, [isActive, resolvedTool]);

  return (
    <div
      className="tools-workbench panel-container"
      hidden={!isActive}
      aria-hidden={!isActive ? true : undefined}
      role="region"
      aria-label={t('tools.title', { defaultValue: 'Tools' })}
    >
      <ToolsSidebar />
      <div ref={detailPaneRef} className="tools-detail-pane panel-right" tabIndex={-1}>
        <div style={{ display: resolvedTool === 'subtitles' ? 'block' : 'none' }}>
          <ToolSubtitlesPanel />
        </div>
        <div style={{ display: resolvedTool === 'voice_typing' ? 'block' : 'none' }}>
          <ToolVoiceTypingPanel />
        </div>
        <div style={{ display: resolvedTool === 'api_server' ? 'block' : 'none' }}>
          <ToolApiServerPanel />
        </div>
        <div style={{ display: resolvedTool === 'mcp' ? 'block' : 'none' }}>
          <ToolMcpPanel />
        </div>
        <div style={{ display: resolvedTool === 'emby_sync' ? 'block' : 'none' }}>
          <ToolEmbySyncPanel />
        </div>
      </div>
    </div>
  );
}

export default ToolsView;
