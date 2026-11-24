import { create } from 'zustand';
import {
  type CliStatus,
  cliService,
  type ShellCompletionStatus,
  type ShellKind,
} from '../services/cliService';

interface CliState {
  status: CliStatus | null;
  completionStatuses: Partial<Record<ShellKind, ShellCompletionStatus>>;
  isLoading: boolean;
  isTogglingPath: boolean;
  removingPathDir: string | null;
  busyCompletionShell: ShellKind | null;
  error: string | null;
  pathJustUpdated: boolean;
  completionNotice: ShellKind | null;

  fetchStatus: () => Promise<void>;
  setPathEnabled: (enabled: boolean) => Promise<void>;
  switchPathToCurrent: () => Promise<void>;
  removePathEntry: (pathDir: string) => Promise<void>;
  installCompletion: (shell: ShellKind) => Promise<void>;
  uninstallCompletion: (shell: ShellKind) => Promise<void>;
  resetPathJustUpdated: () => void;
  resetCompletionNotice: () => void;
  resetError: () => void;
}

function mapCompletionStatuses(
  items: ShellCompletionStatus[] | undefined
): Partial<Record<ShellKind, ShellCompletionStatus>> {
  if (!Array.isArray(items)) {
    return {};
  }
  const next: Partial<Record<ShellKind, ShellCompletionStatus>> = {};
  for (const item of items) {
    if (item && typeof item.shell === 'string') {
      next[item.shell] = item;
    }
  }
  return next;
}

export const useCliStore = create<CliState>((set) => ({
  status: null,
  completionStatuses: {},
  isLoading: false,
  isTogglingPath: false,
  removingPathDir: null,
  busyCompletionShell: null,
  error: null,
  pathJustUpdated: false,
  completionNotice: null,

  fetchStatus: async () => {
    set({ isLoading: true, error: null });
    const [statusResult, completionResult] = await Promise.allSettled([
      cliService.getStatus(),
      cliService.getCompletionStatuses(),
    ]);

    if (statusResult.status === 'rejected') {
      const err = statusResult.reason;
      set({
        error: err instanceof Error ? err.message : String(err),
        isLoading: false,
      });
      return;
    }

    set((state) => ({
      status: statusResult.value,
      completionStatuses:
        completionResult.status === 'fulfilled'
          ? mapCompletionStatuses(completionResult.value)
          : state.completionStatuses,
      isLoading: false,
    }));
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

  switchPathToCurrent: async () => {
    set({ isTogglingPath: true, error: null });
    try {
      const status = await cliService.switchPathToCurrent();
      set({ status, isTogglingPath: false, pathJustUpdated: true });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        isTogglingPath: false,
      });
    }
  },

  removePathEntry: async (pathDir: string) => {
    set({ removingPathDir: pathDir, error: null });
    try {
      const status = await cliService.removePathEntry(pathDir);
      set({ status, removingPathDir: null, pathJustUpdated: true });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        removingPathDir: null,
      });
    }
  },

  installCompletion: async (shell: ShellKind) => {
    set({ busyCompletionShell: shell, error: null });
    try {
      const updated = await cliService.installCompletion(shell);
      set((state) => ({
        completionStatuses: {
          ...state.completionStatuses,
          [shell]: updated,
        },
        busyCompletionShell: null,
        completionNotice: shell,
      }));
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        busyCompletionShell: null,
      });
    }
  },

  uninstallCompletion: async (shell: ShellKind) => {
    set({ busyCompletionShell: shell, error: null });
    try {
      const updated = await cliService.uninstallCompletion(shell);
      set((state) => ({
        completionStatuses: {
          ...state.completionStatuses,
          [shell]: updated,
        },
        busyCompletionShell: null,
      }));
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        busyCompletionShell: null,
      });
    }
  },

  resetPathJustUpdated: () => set({ pathJustUpdated: false }),
  resetCompletionNotice: () => set({ completionNotice: null }),
  resetError: () => set({ error: null }),
}));
