import { create } from 'zustand';
import type { ToolId } from '../types/tools';

export interface ToolsState {
  activeTool: ToolId;
  setActiveTool: (tool: ToolId) => void;
}

export const useToolsStore = create<ToolsState>((set) => ({
  activeTool: 'subtitles',
  setActiveTool: (tool: ToolId) => set({ activeTool: tool }),
}));
