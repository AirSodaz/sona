import { create } from 'zustand';
import { useShallow } from 'zustand/shallow';

export type VoiceTypingRuntimeLifecycleStatus = 'idle' | 'preparing' | 'ready' | 'error';
export type VoiceTypingRuntimeErrorSource =
    | 'shortcut_registration'
    | 'warmup'
    | 'microphone'
    | 'session';

export interface VoiceTypingRuntimeStatus {
    shortcutRegistration: Exclude<VoiceTypingRuntimeLifecycleStatus, 'preparing'>;
    warmup: VoiceTypingRuntimeLifecycleStatus;
    lastErrorSource: VoiceTypingRuntimeErrorSource | null;
    lastErrorMessage: string | null;
    updatedAt: number | null;
}

export const DEFAULT_VOICE_TYPING_RUNTIME_STATUS: VoiceTypingRuntimeStatus = {
    shortcutRegistration: 'idle',
    warmup: 'idle',
    lastErrorSource: null,
    lastErrorMessage: null,
    updatedAt: null,
};

interface VoiceTypingRuntimeStore extends VoiceTypingRuntimeStatus {
    setShortcutRegistrationStatus: (
        status: VoiceTypingRuntimeStatus['shortcutRegistration'],
        errorMessage?: string | null
    ) => void;
    setWarmupStatus: (
        status: VoiceTypingRuntimeLifecycleStatus,
        options?: {
            errorSource?: VoiceTypingRuntimeErrorSource;
            errorMessage?: string | null;
        }
    ) => void;
    reportRuntimeError: (source: VoiceTypingRuntimeErrorSource, errorMessage: string) => void;
    clearRuntimeFailure: (options?: {
        resetShortcutRegistration?: boolean;
        resetWarmup?: boolean;
    }) => void;
    resetRuntimeStatus: () => void;
}

function withTimestamp<T extends Partial<VoiceTypingRuntimeStatus>>(patch: T): T & {
    updatedAt: number;
} {
    return {
        ...patch,
        updatedAt: Date.now(),
    };
}

function maybeClearRecoveredFailure(state: VoiceTypingRuntimeStore) {
    if (state.shortcutRegistration === 'ready' && state.warmup === 'ready') {
        return {
            lastErrorSource: null,
            lastErrorMessage: null,
        };
    }

    return {};
}

export const useVoiceTypingRuntimeStore = create<VoiceTypingRuntimeStore>((set) => ({
    ...DEFAULT_VOICE_TYPING_RUNTIME_STATUS,

    setShortcutRegistrationStatus: (status, errorMessage) =>
        set((state) => {
            if (status === 'error') {
                return withTimestamp({
                    shortcutRegistration: 'error',
                    lastErrorSource: 'shortcut_registration',
                    lastErrorMessage: errorMessage ?? null,
                });
            }

            return withTimestamp({
                shortcutRegistration: status,
                ...(status === 'idle' && state.lastErrorSource === 'shortcut_registration'
                    ? { lastErrorSource: null, lastErrorMessage: null }
                    : {}),
                ...(status === 'ready' ? maybeClearRecoveredFailure({ ...state, shortcutRegistration: status }) : {}),
            });
        }),

    setWarmupStatus: (status, options) =>
        set((state) => {
            if (status === 'error') {
                return withTimestamp({
                    warmup: 'error',
                    lastErrorSource: options?.errorSource ?? 'warmup',
                    lastErrorMessage: options?.errorMessage ?? null,
                });
            }

            return withTimestamp({
                warmup: status,
                ...(status === 'idle' &&
                (state.lastErrorSource === 'warmup' || state.lastErrorSource === 'microphone')
                    ? { lastErrorSource: null, lastErrorMessage: null }
                    : {}),
                ...(status === 'ready' ? maybeClearRecoveredFailure({ ...state, warmup: status }) : {}),
            });
        }),

    reportRuntimeError: (source, errorMessage) =>
        set(() =>
            withTimestamp({
                ...(source === 'shortcut_registration'
                    ? { shortcutRegistration: 'error' as const }
                    : {}),
                ...(source === 'warmup' || source === 'microphone'
                    ? { warmup: 'error' as const }
                    : {}),
                lastErrorSource: source,
                lastErrorMessage: errorMessage,
            })
        ),

    clearRuntimeFailure: (options) =>
        set(() =>
            withTimestamp({
                lastErrorSource: null,
                lastErrorMessage: null,
                ...(options?.resetShortcutRegistration
                    ? { shortcutRegistration: 'idle' as const }
                    : {}),
                ...(options?.resetWarmup ? { warmup: 'idle' as const } : {}),
            })
        ),

    resetRuntimeStatus: () => set(DEFAULT_VOICE_TYPING_RUNTIME_STATUS),
}));

export function useVoiceTypingRuntimeStatus(): VoiceTypingRuntimeStatus {
    return useVoiceTypingRuntimeStore(
        useShallow((state) => ({
            shortcutRegistration: state.shortcutRegistration,
            warmup: state.warmup,
            lastErrorSource: state.lastErrorSource,
            lastErrorMessage: state.lastErrorMessage,
            updatedAt: state.updatedAt,
        }))
    );
}
