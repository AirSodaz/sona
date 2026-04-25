import { create } from 'zustand';
import i18n from '../i18n';
import { logger } from '../utils/logger';
import { AppErrorInput, buildErrorDialogOptions, normalizeError } from '../utils/errorUtils';

/** Supported dialog types. */
export type DialogType = 'alert' | 'confirm' | 'prompt';

/** Visual variants for dialogs. */
export type DialogVariant = 'info' | 'success' | 'warning' | 'error';

/** Options for configuring a dialog. */
export interface DialogOptions {
    /** The dialog title. */
    title?: string;
    /** The content message. */
    message: string;
    /** Optional detail text rendered below the main message. */
    details?: string;
    /** The type of dialog (alert or confirm). */
    type?: DialogType;
    /** The visual style variant. */
    variant?: DialogVariant;
    /** Label for the confirm button. */
    confirmLabel?: string;
    /** Label for the cancel button. */
    cancelLabel?: string;
    /** Default value for prompt input. */
    defaultValue?: string;
    /** Placeholder for prompt input. */
    inputPlaceholder?: string;
    /** Optional callback for an AI action (e.g., auto-generating text). */
    onAiAction?: () => Promise<string>;
}

/** State interface for the dialog store. */
interface DialogState {
    /** Whether the dialog is currently open. */
    isOpen: boolean;
    /** Current dialog options. */
    options: DialogOptions | null;
    /** Resolver function for the current dialog promise. */
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    resolveRef: ((value: any) => void) | null;

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
     * Shows a standardized error dialog.
     *
     * @param input Error metadata for localization, normalization, and logging.
     * @return A promise that resolves when the dialog is closed.
     */
    showError: (input: AppErrorInput) => Promise<void>;

    /**
     * Shows a confirmation dialog.
     *
     * @param message The question/message to display.
     * @param options Additional options.
     * @return A promise that resolves to true if confirmed, false otherwise.
     */
    confirm: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<boolean>;

    /**
     * Shows a prompt dialog with a text input.
     *
     * @param message The message to display.
     * @param options Additional options including defaultValue.
     * @return A promise that resolves to the input string or null if cancelled.
     */
    prompt: (message: string, options?: Omit<DialogOptions, 'message' | 'type'>) => Promise<string | null>;

    /**
     * Closes the dialog with a result.
     *
     * @param result The result value.
     */
    close: (result: any) => void;
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

    showError: async (input) => {
        const normalized = input.cause === undefined ? undefined : normalizeError(input.cause);
        const { title, message, details } = buildErrorDialogOptions(i18n.t.bind(i18n), input);

        void logger.error(`[DialogError:${input.code}] ${message}`, {
            input,
            normalized,
        });

        return get().alert(message, {
            title,
            details,
            variant: 'error',
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

    prompt: (message, options) => {
        return new Promise<string | null>((resolve) => {
            set({
                isOpen: true,
                options: {
                    message,
                    type: 'prompt',
                    variant: 'info',
                    ...options,
                },
                resolveRef: resolve,
            });
        });
    },

    close: (result: any) => {
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
