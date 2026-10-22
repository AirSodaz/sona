import type React from 'react';

export type ToolId = 'subtitles' | 'voice_typing' | 'api_server' | 'mcp';

export interface ToolMeta {
  id: ToolId;
  nameKey: string;
  descriptionKey: string;
  Icon: React.ComponentType<{ size?: number; className?: string }>;
}
