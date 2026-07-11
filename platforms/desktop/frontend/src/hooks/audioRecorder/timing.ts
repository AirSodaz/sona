import type { TranscriptSegment, TranscriptUpdate } from '../../types/transcript';
import { normalizeTranscriptUpdate, shiftTranscriptSegment } from '../../utils/transcriptTiming';
import type { AudioRecorderLogger, RecordSessionPhase, RecordTimingRefs } from './types';

export function shouldFeedWebAudioForPhase(phase: RecordSessionPhase): boolean {
    // Web-audio fallback should stop feeding new samples while paused/stopping,
    // because those transitions are controlled entirely on the client side.
    return phase === 'starting' || phase === 'recording' || phase === 'resuming';
}

interface CreateRecordTimingControllerArgs {
    refs: RecordTimingRefs;
    logger: AudioRecorderLogger;
    setRecordingElapsedMs: (value: number) => void;
    getSessionId: () => string | null;
    now?: () => number;
}

export function createRecordTimingController({
    refs,
    logger,
    setRecordingElapsedMs,
    getSessionId,
    now = () => Date.now(),
}: CreateRecordTimingControllerArgs) {
    // Duration tracking is maintained separately from transcript timing because
    // recognizer segment boundaries can drift around pause/resume transitions.
    function resetRecordedDuration(): void {
        refs.recordedDurationMsRef.current = 0;
        refs.activeDurationStartedAtRef.current = null;
    }

    function beginRecordedDurationWindow(): void {
        refs.activeDurationStartedAtRef.current = now();
    }

    function pauseRecordedDurationWindow(): void {
        if (refs.activeDurationStartedAtRef.current !== null) {
            refs.recordedDurationMsRef.current += now() - refs.activeDurationStartedAtRef.current;
            refs.activeDurationStartedAtRef.current = null;
        }
    }

    function getRecordedDurationMs(): number {
        let durationMs = refs.recordedDurationMsRef.current;
        if (refs.activeDurationStartedAtRef.current !== null) {
            durationMs += now() - refs.activeDurationStartedAtRef.current;
        }
        return durationMs;
    }

    function syncRecordingElapsedMs(): void {
        setRecordingElapsedMs(getRecordedDurationMs());
    }

    function getRecordedDurationSeconds(): number {
        return getRecordedDurationMs() / 1000;
    }

    function finalizeRecordedDurationSeconds(): number {
        pauseRecordedDurationWindow();
        syncRecordingElapsedMs();
        const durationSeconds = getRecordedDurationSeconds();
        refs.finalizedDurationSecondsRef.current = durationSeconds;
        return durationSeconds;
    }

    // Timeline offsets keep emitted segment timestamps monotonic across
    // pause/resume, even if the recognizer restarts its internal clock at 0.
    function resetRecordTimeline(): void {
        refs.segmentTimeOffsetSecondsRef.current = 0;
        refs.recordTimelineCursorSecondsRef.current = 0;
    }

    function resetLiveTimingState(): void {
        resetRecordedDuration();
        resetRecordTimeline();
        setRecordingElapsedMs(0);
    }

    function clearFinalizedDurationSeconds(): void {
        refs.finalizedDurationSecondsRef.current = null;
    }

    function getFinalizedDurationSeconds(): number | null {
        return refs.finalizedDurationSecondsRef.current;
    }

    function getNextSegmentTimeOffsetSeconds(): number {
        return Math.max(refs.recordTimelineCursorSecondsRef.current, getRecordedDurationSeconds());
    }

    function setSegmentTimeOffsetSeconds(offsetSeconds: number, reason: string): void {
        refs.segmentTimeOffsetSecondsRef.current = offsetSeconds;
        logger.info(
            `[useAudioRecorder] Updated record segment timeline offset. session=${getSessionId() ?? 'none'} offset=${offsetSeconds.toFixed(3)} reason=${reason}`
        );
    }

    function normalizeRecordSegmentTiming(segment: TranscriptSegment): TranscriptSegment {
        return shiftTranscriptSegment(segment, refs.segmentTimeOffsetSecondsRef.current);
    }

    function normalizeRecordTranscriptUpdate(update: TranscriptUpdate): TranscriptUpdate {
        const normalized = normalizeTranscriptUpdate(update);
        if (refs.segmentTimeOffsetSecondsRef.current === 0) {
            return normalized;
        }

        return {
            ...normalized,
            upsertSegments: normalized.upsertSegments.map(normalizeRecordSegmentTiming),
        };
    }

    function trackAcceptedSegment(segment: TranscriptSegment): void {
        refs.recordTimelineCursorSecondsRef.current = Math.max(
            refs.recordTimelineCursorSecondsRef.current,
            segment.end,
        );
    }

    function trackAcceptedTranscriptUpdate(update: TranscriptUpdate): void {
        update.upsertSegments.forEach(trackAcceptedSegment);
    }

    return {
        beginRecordedDurationWindow,
        pauseRecordedDurationWindow,
        getRecordedDurationMs,
        syncRecordingElapsedMs,
        getRecordedDurationSeconds,
        finalizeRecordedDurationSeconds,
        resetLiveTimingState,
        clearFinalizedDurationSeconds,
        getFinalizedDurationSeconds,
        getNextSegmentTimeOffsetSeconds,
        setSegmentTimeOffsetSeconds,
        normalizeRecordSegmentTiming,
        normalizeRecordTranscriptUpdate,
        trackAcceptedSegment,
        trackAcceptedTranscriptUpdate,
    };
}
