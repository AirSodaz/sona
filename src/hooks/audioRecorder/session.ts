import type { TranscriptSegment } from '../../types/transcript';
import type {
    AudioRecorderLogger,
    InputSource,
    RecordSegmentDeliveryMeta,
    RecordSessionPhase,
    RecordSessionRefs,
} from './types';

export function createRecordSessionId(): string {
    return `record-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function shouldAcceptRecordSegment(phase: RecordSessionPhase, segment: TranscriptSegment): boolean {
    // During pause/stop transitions we still accept final segments so late
    // recognizer flushes can close the current utterance instead of being lost.
    if (phase === 'starting' || phase === 'recording' || phase === 'resuming') {
        return true;
    }

    if ((phase === 'pausing' || phase === 'stopping') && segment.isFinal) {
        return true;
    }

    return false;
}

interface CreateRecordSessionControllerArgs {
    refs: RecordSessionRefs;
    logger: AudioRecorderLogger;
    clearSegments: () => void;
    resetLiveTimingState: () => void;
    clearFinalizedDurationSeconds: () => void;
    beginRecordedDurationWindow: () => void;
    syncRecordingElapsedMs: () => void;
    setIsRecording: (value: boolean) => void;
    setIsPaused: (value: boolean) => void;
    setIsTransitioning: (value: boolean) => void;
    getInputSource: () => InputSource;
    getIsRecording: () => boolean;
    softStopRecordRuntime: () => Promise<void>;
}

export function createRecordSessionController({
    refs,
    logger,
    clearSegments,
    resetLiveTimingState,
    clearFinalizedDurationSeconds,
    beginRecordedDurationWindow,
    syncRecordingElapsedMs,
    setIsRecording,
    setIsPaused,
    setIsTransitioning,
    getInputSource,
    getIsRecording,
    softStopRecordRuntime,
}: CreateRecordSessionControllerArgs) {
    function getSessionId(): string | null {
        return refs.recordSessionIdRef.current;
    }

    function getPhase(): RecordSessionPhase {
        return refs.recordSessionPhaseRef.current;
    }

    function setPhase(phase: RecordSessionPhase): void {
        refs.recordSessionPhaseRef.current = phase;
    }

    function isActiveSession(sessionId: string): boolean {
        return refs.recordSessionIdRef.current === sessionId;
    }

    function canMutateActiveRecordResources(sessionId: string): boolean {
        return refs.recordSessionIdRef.current === sessionId || refs.recordSessionIdRef.current === null;
    }

    function shouldFinalizeRecordStartAttempt(sessionId: string): boolean {
        return refs.recordSessionIdRef.current === sessionId || refs.recordSessionIdRef.current === null;
    }

    function openRecordSession(): string {
        // Opening a session clears UI/runtime state up front so any stale async
        // callback can be rejected by session id before it mutates the new run.
        const sessionId = createRecordSessionId();
        refs.recordSessionIdRef.current = sessionId;
        refs.recordSessionPhaseRef.current = 'starting';
        refs.peakLevelRef.current = 0;
        clearFinalizedDurationSeconds();
        resetLiveTimingState();
        setIsRecording(false);
        setIsPaused(false);
        clearSegments();
        logger.info(`[useAudioRecorder] Record session opened. session=${sessionId} input=${getInputSource()}`);
        return sessionId;
    }

    function activateRecordSession(sessionId: string): boolean {
        if (refs.recordSessionIdRef.current !== sessionId) {
            logger.warn(
                `[useAudioRecorder] Skipping record session activation because the active session changed. requested=${sessionId} active=${refs.recordSessionIdRef.current ?? 'none'}`
            );
            return false;
        }

        // Activation is the point where capture, recognizer, and UI agree that
        // this session is the current live recording owner.
        refs.recordSessionPhaseRef.current = 'recording';
        setIsRecording(true);
        setIsPaused(false);
        beginRecordedDurationWindow();
        syncRecordingElapsedMs();
        return true;
    }

    function getRecordSegmentDeliveryMeta(segment: TranscriptSegment): RecordSegmentDeliveryMeta {
        const phase = refs.recordSessionPhaseRef.current;
        return {
            sessionId: refs.recordSessionIdRef.current,
            phase,
            isRecording: getIsRecording(),
            accepted: refs.recordSessionIdRef.current !== null && shouldAcceptRecordSegment(phase, segment),
        };
    }

    function resetRecordSession(sessionId: string | null, reason: string, clearTranscript = false): void {
        // Ignore stale resets from older async branches so a failed start/stop
        // cannot wipe the active session that replaced it.
        if (sessionId && refs.recordSessionIdRef.current !== sessionId) {
            logger.info(
                `[useAudioRecorder] Ignoring record session reset for stale session. requested=${sessionId} active=${refs.recordSessionIdRef.current ?? 'none'} reason=${reason}`
            );
            return;
        }

        const activeSessionId = refs.recordSessionIdRef.current;
        const previousPhase = refs.recordSessionPhaseRef.current;
        if (clearTranscript) {
            clearSegments();
        }
        refs.recordSessionIdRef.current = null;
        refs.recordSessionPhaseRef.current = 'idle';
        refs.peakLevelRef.current = 0;
        resetLiveTimingState();
        setIsRecording(false);
        setIsPaused(false);
        setIsTransitioning(false);
        logger.info(
            `[useAudioRecorder] Record session reset. session=${activeSessionId ?? 'none'} previous_phase=${previousPhase} reason=${reason} cleared=${clearTranscript}`
        );
    }

    async function softStopRecordSessionIfActive(sessionId: string, reason: string): Promise<void> {
        if (!canMutateActiveRecordResources(sessionId)) {
            logger.info(
                `[useAudioRecorder] Skipping recognizer rollback for stale session. requested=${sessionId} active=${refs.recordSessionIdRef.current ?? 'none'} reason=${reason}`
            );
            return;
        }

        // Capture rollback alone is not enough when a partially started session
        // needs to unwind; the recognizer has its own runtime state to clear.
        try {
            await softStopRecordRuntime();
            logger.info(`[useAudioRecorder] Rolled back recognizer state. session=${sessionId} reason=${reason}`);
        } catch (error) {
            logger.warn(`[useAudioRecorder] Failed to roll back recognizer state. session=${sessionId} reason=${reason}`, error);
        }
    }

    return {
        openRecordSession,
        activateRecordSession,
        getRecordSegmentDeliveryMeta,
        canMutateActiveRecordResources,
        shouldFinalizeRecordStartAttempt,
        resetRecordSession,
        softStopRecordSessionIfActive,
        getSessionId,
        getPhase,
        setPhase,
        isActiveSession,
    };
}
