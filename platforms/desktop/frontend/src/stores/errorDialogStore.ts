import { create } from 'zustand';
import type { ErrorDialogViewModel } from '../utils/errorUtils';
import { logger } from '../utils/logger';

export type ErrorDialogResult = 'dismiss' | 'primary';

interface ErrorDialogState {
  isOpen: boolean;
  options: ErrorDialogViewModel | null;
  resolveRef: ((result: ErrorDialogResult) => void) | null;
  showError: (options: ErrorDialogViewModel) => Promise<ErrorDialogResult>;
  close: (result: ErrorDialogResult) => void;
}

/**
 * Zustand store for managing dedicated error dialogs.
 */
export const useErrorDialogStore = create<ErrorDialogState>((set, get) => ({
  isOpen: false,
  options: null,
  resolveRef: null,

  showError: (options) => {
    void logger.error(`[ErrorDialog] ${options.title || 'Error'}: ${options.message}`, {
      details: options.details,
      code: options.code,
      hasPrimaryAction: options.hasPrimaryAction,
    });

    return new Promise<ErrorDialogResult>((resolve) => {
      set({
        isOpen: true,
        options,
        resolveRef: resolve,
      });
    });
  },

  close: (result) => {
    const { resolveRef } = get();
    resolveRef?.(result);
    set({
      isOpen: false,
      options: null,
      resolveRef: null,
    });
  },
}));
