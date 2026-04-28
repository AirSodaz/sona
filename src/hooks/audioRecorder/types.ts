import type { AppConfig } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { TranscriptSegment } from '../../types/transcript';

export type InputSource = 'microphone' | 'desktop';

export type RecordSessionPhase =
    | 'idle'
    | 'starting'
    | 'recording'
    | 'pausing'
    | 'paused'
    | 'resuming'
    | 'stopping';

export interface RecordSegmentDeliveryMeta {
    sessionId: string | null;
    phase: RecordSessionPhase;
    isRecording: boolean;
    accepted: boolean;
}

export interface MutableRefLike<T> {
    current: T;
}

export interface RecordSessionRefs {
    recordSessionIdRef: MutableRefLike<string | null>;
    recordSessionPhaseRef: MutableRefLike<RecordSessionPhase>;
    peakLevelRef: MutableRefLike<number>;
}

export interface RecordTimingRefs {
    recordedDurationMsRef: MutableRefLike<number>;
    activeDurationStartedAtRef: MutableRefLike<number | null>;
    finalizedDurationSecondsRef: MutableRefLike<number | null>;
    segmentTimeOffsetSecondsRef: MutableRefLike<number>;
    recordTimelineCursorSecondsRef: MutableRefLike<number>;
}

export interface AudioRecorderCaptureRefs {
    audioContextRef: MutableRefLike<AudioContext | null>;
    mediaRecorderRef: MutableRefLike<MediaRecorder | null>;
    activeStreamRef: MutableRefLike<MediaStream | null>;
    nativeAudioUnlistenRef: MutableRefLike<(() => void) | null>;
    usingNativeCaptureRef: MutableRefLike<boolean>;
    mimeTypeRef: MutableRefLike<string>;
    peakLevelRef: MutableRefLike<number>;
    activeInputSourceRef: MutableRefLike<InputSource>;
    isPausedRef: MutableRefLike<boolean>;
    recordSessionPhaseRef: MutableRefLike<RecordSessionPhase>;
}

export interface AudioRecorderLogger {
    info: (message: string, ...args: unknown[]) => unknown;
    warn: (message: string, ...args: unknown[]) => unknown;
    error: (message: string, ...args: unknown[]) => unknown;
}

export interface RecordingMetaState {
    setSourceHistoryId: (id: string | null) => void;
    setTitle: (title: string | null) => void;
    setIcon: (icon: string | null) => void;
}

export interface RecordingPersistenceTranscriptState {
    config: AppConfig;
    segments: TranscriptSegment[];
    setAudioUrl: (url: string | null) => void;
    setSegments: (segments: TranscriptSegment[]) => void;
}

export interface RecordingHistorySaver {
    saveRecording: (
        blob: Blob,
        segments: TranscriptSegment[],
        duration: number,
        projectId?: string | null,
    ) => Promise<HistoryItem | null>;
    saveNativeRecording: (
        absoluteWavPath: string,
        segments: TranscriptSegment[],
        duration: number,
        projectId?: string | null,
    ) => Promise<HistoryItem | null>;
}
