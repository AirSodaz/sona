import { create } from 'zustand';
import type { ToolId } from '../types/tools';

export interface ToolsState {
  activeTool: ToolId;
  setActiveTool: (tool: ToolId) => void;
  embyServerName: string | null;
  setEmbyServerName: (name: string | null) => void;
}

export const useToolsStore = create<ToolsState>((set) => ({
  activeTool: 'subtitles',
  setActiveTool: (tool: ToolId) => set({ activeTool: tool }),
  embyServerName: null,
  setEmbyServerName: (name: string | null) => set({ embyServerName: name }),
}));
