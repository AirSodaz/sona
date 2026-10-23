import type React from 'react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useToolsStore } from '../../stores/toolsStore';
import { ToolApiServerPanel } from './panels/ToolApiServerPanel';
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
    activeTool === 'mcp'
      ? activeTool
      : 'subtitles';
  const detailPaneRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isActive && resolvedTool && detailPaneRef.current) {
      detailPaneRef.current.scrollTop = 0;
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
        {resolvedTool === 'subtitles' && <ToolSubtitlesPanel />}
        {resolvedTool === 'voice_typing' && <ToolVoiceTypingPanel />}
        {resolvedTool === 'api_server' && <ToolApiServerPanel />}
        {resolvedTool === 'mcp' && <ToolMcpPanel />}
      </div>
    </div>
  );
}

export default ToolsView;
