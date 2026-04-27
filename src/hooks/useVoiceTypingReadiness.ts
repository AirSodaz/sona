import { useMemo } from 'react';
import { useShallow } from 'zustand/shallow';
import { PRESET_MODELS, modelService } from '../services/modelService';
import { useConfigStore } from '../stores/configStore';
import {
    VoiceTypingRuntimeErrorSource,
    useVoiceTypingRuntimeStatus,
} from '../stores/voiceTypingRuntimeStore';

export type VoiceTypingReadinessState =
    | 'off'
    | 'needs_shortcut'
    | 'needs_live_model'
    | 'needs_vad'
    | 'failed'
    | 'preparing'
    | 'ready';

export interface VoiceTypingReadinessSnapshot {
    state: VoiceTypingReadinessState;
    shortcutConfigured: boolean;
    liveModelConfigured: boolean;
    requiresVad: boolean;
    vadConfigured: boolean;
    shortcutRegistration: 'idle' | 'ready' | 'error';
    warmup: 'idle' | 'preparing' | 'ready' | 'error';
    inputDeviceState: 'off' | 'ready' | 'failed';
    runtimeState: 'off' | 'preparing' | 'ready' | 'failed';
    lastErrorSource: VoiceTypingRuntimeErrorSource | null;
    lastErrorMessage: string | null;
}

function resolveStreamingModelId(modelPath: string) {
    return (
        PRESET_MODELS.find(
            (model) =>
                model.modes?.includes('streaming') &&
                modelPath.includes(model.filename || model.id)
        )?.id ?? null
    );
}

export function useVoiceTypingReadiness(): VoiceTypingReadinessSnapshot {
    const config = useConfigStore(
        useShallow((state) => ({
            voiceTypingEnabled: state.config.voiceTypingEnabled ?? false,
            voiceTypingShortcut: state.config.voiceTypingShortcut ?? '',
            streamingModelPath: state.config.streamingModelPath ?? '',
            vadModelPath: state.config.vadModelPath ?? '',
            microphoneId: state.config.microphoneId ?? 'default',
        }))
    );
    const runtime = useVoiceTypingRuntimeStatus();

    return useMemo(() => {
        const shortcutConfigured = config.voiceTypingShortcut.trim().length > 0;
        const liveModelConfigured = config.streamingModelPath.trim().length > 0;
        const selectedStreamingModelId = liveModelConfigured
            ? resolveStreamingModelId(config.streamingModelPath)
            : null;
        const requiresVad = !!selectedStreamingModelId
            ? modelService.getModelRules(selectedStreamingModelId).requiresVad
            : false;
        const vadConfigured = !requiresVad || config.vadModelPath.trim().length > 0;
        const hasRuntimeFailure =
            runtime.shortcutRegistration === 'error' ||
            runtime.warmup === 'error' ||
            runtime.lastErrorSource !== null;

        let state: VoiceTypingReadinessState;
        if (!config.voiceTypingEnabled) {
            state = 'off';
        } else if (!shortcutConfigured) {
            state = 'needs_shortcut';
        } else if (!liveModelConfigured) {
            state = 'needs_live_model';
        } else if (!vadConfigured) {
            state = 'needs_vad';
        } else if (hasRuntimeFailure) {
            state = 'failed';
        } else if (
            runtime.shortcutRegistration !== 'ready' ||
            runtime.warmup !== 'ready'
        ) {
            state = 'preparing';
        } else {
            state = 'ready';
        }

        return {
            state,
            shortcutConfigured,
            liveModelConfigured,
            requiresVad,
            vadConfigured,
            shortcutRegistration: runtime.shortcutRegistration,
            warmup: runtime.warmup,
            inputDeviceState: !config.voiceTypingEnabled
                ? 'off'
                : runtime.lastErrorSource === 'microphone'
                    ? 'failed'
                    : 'ready',
            runtimeState: !config.voiceTypingEnabled
                ? 'off'
                : state === 'failed'
                    ? 'failed'
                    : state === 'ready'
                        ? 'ready'
                        : 'preparing',
            lastErrorSource: runtime.lastErrorSource,
            lastErrorMessage: runtime.lastErrorMessage,
        };
    }, [config, runtime]);
}
