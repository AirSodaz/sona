import type React from 'react';
import { useEffect, useRef } from 'react';
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
  const activeTool = useToolsStore((state) => state.activeTool);
  const detailPaneRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activeTool && detailPaneRef.current) {
      detailPaneRef.current.scrollTop = 0;
    }
  }, [activeTool]);

  return (
    <div
      className="tools-workbench panel-container"
      aria-hidden={!isActive ? true : undefined}
      role="region"
      aria-label="Tools Workbench"
    >
      <ToolsSidebar />
      <div ref={detailPaneRef} className="tools-detail-pane panel-right" tabIndex={-1}>
        {activeTool === 'subtitles' && <ToolSubtitlesPanel />}
        {activeTool === 'voice_typing' && <ToolVoiceTypingPanel />}
        {activeTool === 'api_server' && <ToolApiServerPanel />}
        {activeTool === 'mcp' && <ToolMcpPanel />}
      </div>
    </div>
  );
}

export default ToolsView;
