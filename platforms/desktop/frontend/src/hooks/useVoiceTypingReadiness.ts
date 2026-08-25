import { useMemo } from 'react';
import { useShallow } from 'zustand/shallow';
import { resolveVoiceTypingReadinessSnapshot } from '../services/voiceTypingReadiness';
import { useConfigStore } from '../stores/configStore';
import {
    useVoiceTypingRuntimeStatus,
} from '../stores/voiceTypingRuntimeStore';
import type {
    VoiceTypingReadinessSnapshot,
} from '../types/voiceTyping';

export type {
    VoiceTypingReadinessSnapshot,
    VoiceTypingReadinessState,
} from '../types/voiceTyping';

export { resolveVoiceTypingReadinessSnapshot } from '../services/voiceTypingReadiness';

export function useVoiceTypingReadiness(): VoiceTypingReadinessSnapshot {
    const config = useConfigStore(
        useShallow((state) => ({
            voiceTypingEnabled: state.config.voiceTypingEnabled ?? false,
            voiceTypingShortcut: state.config.voiceTypingShortcut ?? '',
            streamingModelPath: state.config.streamingModelPath ?? '',
            liveVadModelPath: state.config.liveVadModelPath ?? '',
            microphoneId: state.config.microphoneId ?? 'default',
        }))
    );
    const runtime = useVoiceTypingRuntimeStatus();

    return useMemo(() => resolveVoiceTypingReadinessSnapshot(config, runtime), [config, runtime]);
}
