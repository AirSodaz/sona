import { useMemo } from 'react';
import { useShallow } from 'zustand/shallow';
import { modelService } from '../services/modelService';
import { useConfigStore } from '../stores/configStore';
import {
    VoiceTypingRuntimeErrorSource,
    type VoiceTypingRuntimeStatus,
    useVoiceTypingRuntimeStatus,
} from '../stores/voiceTypingRuntimeStore';
import type { AppConfig } from '../types/config';
import { findSelectedModelByMode } from '../utils/modelSelection';

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

export function resolveVoiceTypingReadinessSnapshot(
    config: Pick<
        AppConfig,
        'voiceTypingEnabled' | 'voiceTypingShortcut' | 'streamingModelPath' | 'vadModelPath' | 'microphoneId'
    >,
    runtime: VoiceTypingRuntimeStatus,
): VoiceTypingReadinessSnapshot {
    const shortcutConfigured = (config.voiceTypingShortcut ?? '').trim().length > 0;
    const liveModelConfigured = (config.streamingModelPath ?? '').trim().length > 0;
    const selectedStreamingModel = liveModelConfigured
        ? findSelectedModelByMode(config.streamingModelPath ?? '', 'streaming')
        : null;
    const requiresVad = selectedStreamingModel
        ? modelService.getModelRules(selectedStreamingModel.id).requiresVad
        : false;
    const vadConfigured = !requiresVad || (config.vadModelPath ?? '').trim().length > 0;
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

    return useMemo(() => resolveVoiceTypingReadinessSnapshot(config, runtime), [config, runtime]);
}
