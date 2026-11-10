import { create } from 'zustand';
import { type CliStatus, cliService } from '../services/cliService';

interface CliState {
  status: CliStatus | null;
  isLoading: boolean;
  isTogglingPath: boolean;
  error: string | null;
  pathJustUpdated: boolean;

  fetchStatus: () => Promise<void>;
  setPathEnabled: (enabled: boolean) => Promise<void>;
  resetPathJustUpdated: () => void;
  resetError: () => void;
}

export const useCliStore = create<CliState>((set) => ({
  status: null,
  isLoading: false,
  isTogglingPath: false,
  error: null,
  pathJustUpdated: false,

  fetchStatus: async () => {
    set({ isLoading: true, error: null });
    try {
      const status = await cliService.getStatus();
      set({ status, isLoading: false });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        isLoading: false,
      });
    }
  },

  setPathEnabled: async (enabled: boolean) => {
    set({ isTogglingPath: true, error: null });
    try {
      const status = await cliService.setPathEnabled(enabled);
      set({ status, isTogglingPath: false, pathJustUpdated: true });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        isTogglingPath: false,
      });
    }
  },

  resetPathJustUpdated: () => set({ pathJustUpdated: false }),
  resetError: () => set({ error: null }),
}));
