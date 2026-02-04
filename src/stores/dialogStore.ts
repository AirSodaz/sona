import { create } from 'zustand';

/** Supported dialog types. */
export type DialogType = 'alert' | 'confirm';

/** Visual variants for dialogs. */
export type DialogVariant = 'info' | 'success' | 'warning' | 'error';

/** Options for configuring a dialog. */
export interface DialogOptions {
    /** The dialog title. */
    title?: string;
    /** The content message. */
    message: string;
    /** The type of dialog (alert or confirm). */
    type?: DialogType;
    /** The visual style variant. */
    variant?: DialogVariant;
    /** Label for the confirm button. */
    confirmLabel?: string;
    /** Label for the cancel button. */
    cancelLabel?: string;
}

/** State interface for the dialog store. */
interface DialogState {
    /** Whether the dialog is currently open. */
    isOpen: boolean;
    /** Current dialog options. */
    options: DialogOptions | null;
    /** Resolver function for the current dialog promise. */
    resolveRef: ((value: boolean) => void) | null;

    // Actions
    /**
     * Shows an alert dialog.
     *
     * @param message The message to display.
     * @param options Additional options.
     * @return A promise that resolves when the alert is closed.
     */
    alert: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<void>;

    /**
     * Shows a confirmation dialog.
     *
     * @param message The question/message to display.
     * @param options Additional options.
     * @return A promise that resolves to true if confirmed, false otherwise.
     */
    confirm: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<boolean>;

    /**
     * Closes the dialog with a result.
     *
     * @param result The result value (true for confirm, false for cancel).
     */
    close: (result: boolean) => void;
}

/**
 * Zustand store for managing global dialogs.
 */
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
