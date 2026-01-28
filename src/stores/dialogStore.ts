import { create } from 'zustand';

export type DialogType = 'alert' | 'confirm';
export type DialogVariant = 'info' | 'success' | 'warning' | 'error';

export interface DialogOptions {
    title?: string;
    message: string;
    type?: DialogType;
    variant?: DialogVariant;
    confirmLabel?: string;
    cancelLabel?: string;
}

interface DialogState {
    isOpen: boolean;
    options: DialogOptions | null;
    resolveRef: ((value: boolean) => void) | null;

    // Actions
    alert: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<void>;
    confirm: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<boolean>;
    close: (result: boolean) => void;
}

export const useDialogStore = create<DialogState>((set, get) => ({
    isOpen: false,
    options: null,
    resolveRef: null,

    alert: (message, options) => {
        return new Promise<void>((resolve) => {
            set({
                isOpen: true,
                options: {
                    message,
                    type: 'alert',
                    variant: 'info',
                    ...options,
                },
                resolveRef: () => resolve(),
            });
        });
    },

    confirm: (message, options) => {
        return new Promise<boolean>((resolve) => {
            set({
                isOpen: true,
                options: {
                    message,
                    type: 'confirm',
                    variant: 'warning', // Default to warning for confirmations usually
                    ...options,
                },
                resolveRef: resolve,
            });
        });
    },

    close: (result: boolean) => {
        const { resolveRef } = get();
        if (resolveRef) {
            resolveRef(result);
        }
        set({
            isOpen: false,
            options: null,
            resolveRef: null,
        });
    },
}));
