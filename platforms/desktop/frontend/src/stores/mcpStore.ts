import { create } from 'zustand';
import { TauriCommand } from '../services/tauri/commands';
import { invokeTauri } from '../services/tauri/invoke';
import type {
  McpActivityLogEntry,
  McpClientConfigMap,
  McpStatus,
  McpToolCatalogItem,
} from '../types/mcp';

interface McpFilter {
  status: string; // 'all' | 'success' | 'error' | 'blocked'
  query: string;
}

interface McpState {
  status: McpStatus | null;
  logs: McpActivityLogEntry[];
  tools: McpToolCatalogItem[];
  clientConfigs: McpClientConfigMap | null;
  filter: McpFilter;
  isLoading: boolean;
  error: string | null;

  fetchStatus: () => Promise<void>;
  toggleEnabled: (enabled: boolean) => Promise<void>;
  disconnectAll: () => Promise<void>;
  fetchLogs: () => Promise<void>;
  clearLogs: () => Promise<void>;
  fetchTools: () => Promise<void>;
  fetchClientConfigs: () => Promise<void>;
  addLogEntry: (entry: McpActivityLogEntry) => void;
  setFilterStatus: (status: string) => void;
  setFilterQuery: (query: string) => void;
}

export const useMcpStore = create<McpState>((set, get) => ({
  status: null,
  logs: [],
  tools: [],
  clientConfigs: null,
  filter: {
    status: 'all',
    query: '',
  },
  isLoading: false,
  error: null,
  fetchStatus: async () => {
    try {
      const status = await invokeTauri(TauriCommand.mcp.getStatus);
      if (status) {
        set({ status, error: null });
      }
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  toggleEnabled: async (enabled: boolean) => {
    try {
      await invokeTauri(TauriCommand.mcp.setEnabled, { enabled });
      await get().fetchStatus();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  disconnectAll: async () => {
    try {
      await invokeTauri(TauriCommand.mcp.disconnectAll);
      await get().fetchStatus();
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  fetchLogs: async () => {
    try {
      const logs = await invokeTauri(TauriCommand.mcp.getActivityLogs);
      if (Array.isArray(logs)) {
        set({ logs, error: null });
      }
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  clearLogs: async () => {
    try {
      await invokeTauri(TauriCommand.mcp.clearActivityLogs);
      set({ logs: [] });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  fetchTools: async () => {
    try {
      const tools = await invokeTauri(TauriCommand.mcp.getToolsCatalog);
      if (Array.isArray(tools)) {
        set({ tools, error: null });
      }
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  fetchClientConfigs: async () => {
    try {
      const clientConfigs = await invokeTauri(TauriCommand.mcp.getClientConfigs);
      if (clientConfigs) {
        set({ clientConfigs, error: null });
      }
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  addLogEntry: (entry: McpActivityLogEntry) => {
    set((state) => {
      const existing = state.logs;
      // Prepend or append? Most recent first makes viewing live logs effortless!
      const updated = [entry, ...existing.filter((item) => item.id !== entry.id)].slice(0, 100);
      return { logs: updated };
    });
  },

  setFilterStatus: (status: string) => {
    set((state) => ({ filter: { ...state.filter, status } }));
  },

  setFilterQuery: (query: string) => {
    set((state) => ({ filter: { ...state.filter, query } }));
  },
}));
