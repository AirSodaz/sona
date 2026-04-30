import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { remove, writeFile } from '@tauri-apps/plugin-fs';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useHistoryStore } from '../stores/historyStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useProjectStore } from '../stores/projectStore';
import {
    clearTranscriptSegments,
    setTranscriptSegments,
    syncSavedRecordingMeta as syncTranscriptSavedRecordingMeta,
} from '../stores/transcriptCoordinator';
import { useTranscriptPlaybackStore } from '../stores/transcriptPlaybackStore';
import { useTranscriptRuntimeStore } from '../stores/transcriptRuntimeStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { historyService } from '../services/historyService';
import { speakerService } from '../services/speakerService';
import { summaryService } from '../services/summaryService';
import { transcriptionService } from '../services/transcriptionService';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { getResumeOnboardingStep } from '../utils/onboarding';
import { createAudioRecorderCapture, getSupportedMimeType } from './audioRecorder/capture';
import {
    createRecordingPersistence,
} from './audioRecorder/persistence';
import { createRecordController } from './audioRecorder/recordController';
import { createRecordSessionController } from './audioRecorder/session';
import { createRecordTimingController } from './audioRecorder/timing';
import type {
    InputSource,
    RecordSegmentDeliveryMeta,
    RecordSessionPhase,
} from './audioRecorder/types';
import type { LiveRecordingDraftHandle } from '../services/historyService';

export type {
    RecordSegmentDeliveryMeta,
    RecordSessionPhase,
} from './audioRecorder/types';

export { getSupportedMimeType };

interface UseAudioRecorderProps {
    inputSource: InputSource;
    onSegment: (update: TranscriptUpdate, meta: RecordSegmentDeliveryMeta) => void;
}

export function useAudioRecorder({ inputSource, onSegment }: UseAudioRecorderProps) {
    const config = useConfigStore((state) => state.config);
    const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
    const isPaused = useTranscriptRuntimeStore((state) => state.isPaused);
    const setIsRecording = useTranscriptRuntimeStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptRuntimeStore((state) => state.setIsPaused);
    const setAudioUrl = useTranscriptPlaybackStore((state) => state.setAudioUrl);
    const setAudioFile = useTranscriptPlaybackStore((state) => state.setAudioFile);
    const showError = useDialogStore((state) => state.showError);

    const audioContextRef = useRef<AudioContext | null>(null);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const activeStreamRef = useRef<MediaStream | null>(null);
    const nativeAudioUnlistenRef = useRef<(() => void) | null>(null);
    const usingNativeCaptureRef = useRef(false);
    const mimeTypeRef = useRef<string>('');
    const peakLevelRef = useRef<number>(0);
    const activeInputSourceRef = useRef<InputSource>(inputSource);
    const onSegmentRef = useRef(onSegment);
    const recordSessionIdRef = useRef<string | null>(null);
    const recordSessionPhaseRef = useRef<RecordSessionPhase>('idle');
    const recordedDurationMsRef = useRef(0);
    const activeDurationStartedAtRef = useRef<number | null>(null);
    const finalizedDurationSecondsRef = useRef<number | null>(null);
    const segmentTimeOffsetSecondsRef = useRef(0);
    const recordTimelineCursorSecondsRef = useRef(0);
    const liveDraftRef = useRef<LiveRecordingDraftHandle | null>(null);

    const [isInitializing, setIsInitializing] = useState(false);
    const [isTransitioning, setIsTransitioning] = useState(false);
    const [recordingElapsedMs, setRecordingElapsedMs] = useState(0);

    const isRecordingRef = useRef(isRecording);
    const isPausedRef = useRef(isPaused);

    useEffect(() => {
        isRecordingRef.current = isRecording;
    }, [isRecording]);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    useEffect(() => {
        onSegmentRef.current = onSegment;
    }, [onSegment]);

    const setPeakFromInt16 = useCallback((samples: Int16Array) => {
        let maxAbs = 0;
        for (let i = 0; i < samples.length; i++) {
            const abs = Math.abs(samples[i]);
            if (abs > maxAbs) {
                maxAbs = abs;
            }
        }
        peakLevelRef.current = Math.min(1, maxAbs / 32767);
    }, []);

    const timing = useMemo(() => createRecordTimingController({
        refs: {
            recordedDurationMsRef,
            activeDurationStartedAtRef,
            finalizedDurationSecondsRef,
            segmentTimeOffsetSecondsRef,
            recordTimelineCursorSecondsRef,
        },
        logger,
        setRecordingElapsedMs,
        getSessionId: () => recordSessionIdRef.current,
    }), []);

    const session = useMemo(() => createRecordSessionController({
        refs: {
            recordSessionIdRef,
            recordSessionPhaseRef,
            peakLevelRef,
        },
        logger,
        clearSegments: clearTranscriptSegments,
        resetLiveTimingState: timing.resetLiveTimingState,
        clearFinalizedDurationSeconds: timing.clearFinalizedDurationSeconds,
        beginRecordedDurationWindow: timing.beginRecordedDurationWindow,
        syncRecordingElapsedMs: timing.syncRecordingElapsedMs,
        setIsRecording,
        setIsPaused,
        setIsTransitioning,
        getInputSource: () => inputSource,
        getIsRecording: () => isRecordingRef.current,
        softStopRecordRuntime: () => transcriptionService.softStop(),
    }), [
        inputSource,
        setIsPaused,
        setIsRecording,
        timing.beginRecordedDurationWindow,
        timing.clearFinalizedDurationSeconds,
        timing.resetLiveTimingState,
        timing.syncRecordingElapsedMs,
    ]);

    const persistence = useMemo(() => createRecordingPersistence({
        logger,
        history: {
            createLiveRecordingDraft: (...args) => historyService.createLiveRecordingDraft(...args),
            completeLiveRecordingDraft: (...args) => historyService.completeLiveRecordingDraft(...args),
            discardLiveRecordingDraft: (...args) => historyService.discardLiveRecordingDraft(...args),
            saveRecording: (...args) => historyService.saveRecording(...args),
            saveNativeRecording: (...args) => historyService.saveNativeRecording(...args),
        },
        getTranscriptState: () => ({
            config: getEffectiveConfigSnapshot(),
            segments: useTranscriptSessionStore.getState().segments,
            setAudioUrl: useTranscriptPlaybackStore.getState().setAudioUrl,
            setSegments: setTranscriptSegments,
        }),
        getActiveProjectId: () => useProjectStore.getState().activeProjectId,
        setActiveProjectId: (projectId) => useProjectStore.getState().setActiveProjectId(projectId),
        addHistoryItem: (item) => useHistoryStore.getState().addItem(item),
        upsertHistoryItem: (item) => useHistoryStore.getState().upsertItem(item),
        deleteHistoryItem: (id) => useHistoryStore.getState().deleteItem(id),
        persistSummary: (historyId) => summaryService.persistSummary(historyId),
        annotateSegmentsForFile: (filePath, segments, transcriptConfig) => (
            speakerService.annotateSegmentsForFile(filePath, segments, transcriptConfig)
        ),
        syncSavedRecordingMeta: (title, historyId, icon) => (
            syncTranscriptSavedRecordingMeta(title, historyId, icon)
        ),
        writeFile: (filePath, contents) => writeFile(filePath, contents),
        removeFile: (filePath) => remove(filePath),
        fileSrcFromPath: convertFileSrc,
    }), []);

    const handleWebRecordingStop = useCallback(async (blob: Blob, mimeType: string) => {
        const persistedBlob = blob.type ? blob : new Blob([blob], { type: mimeType });
        const liveDraft = liveDraftRef.current;
        if (!liveDraft) {
            timing.clearFinalizedDurationSeconds();
            return;
        }
        try {
            await persistence.persistBrowserRecording(
                liveDraft,
                persistedBlob,
                timing.getFinalizedDurationSeconds() ?? timing.getRecordedDurationSeconds(),
            );
        } finally {
            liveDraftRef.current = null;
            timing.clearFinalizedDurationSeconds();
        }
    }, [persistence, timing]);

    const forwardRecordSegment = useCallback((update: TranscriptUpdate) => {
        const normalizedUpdate = timing.normalizeRecordTranscriptUpdate(update);
        const representativeSegment = normalizedUpdate.upsertSegments[normalizedUpdate.upsertSegments.length - 1];
        const meta = session.getRecordSegmentDeliveryMeta(
            representativeSegment || ({
                id: 'record-update-empty',
                text: '',
                start: 0,
                end: 0,
                isFinal: false,
            } as TranscriptSegment)
        );

        if (meta.accepted) {
            timing.trackAcceptedTranscriptUpdate(normalizedUpdate);
        }

        logger.info(
            `[useAudioRecorder] Record update received. session=${meta.sessionId ?? 'none'} phase=${meta.phase} accepted=${meta.accepted} removes=${normalizedUpdate.removeIds.length} upserts=${normalizedUpdate.upsertSegments.length} offset=${segmentTimeOffsetSecondsRef.current.toFixed(3)}`
        );
        onSegmentRef.current(normalizedUpdate, meta);
    }, [session, timing]);

    const capture = useMemo(() => createAudioRecorderCapture({
        refs: {
            audioContextRef,
            mediaRecorderRef,
            activeStreamRef,
            nativeAudioUnlistenRef,
            usingNativeCaptureRef,
            mimeTypeRef,
            peakLevelRef,
            activeInputSourceRef,
            isPausedRef,
            recordSessionPhaseRef,
        },
        logger,
        onSegment: forwardRecordSegment,
        showError,
        activateRecordSession: session.activateRecordSession,
        canMutateActiveRecordResources: session.canMutateActiveRecordResources,
        rollbackRecognizer: session.softStopRecordSessionIfActive,
        setPeakFromInt16,
        onWebRecordingStop: handleWebRecordingStop,
        setIsRecording,
        setIsPaused,
    }), [
        forwardRecordSegment,
        handleWebRecordingStop,
        session,
        setIsPaused,
        setIsRecording,
        setPeakFromInt16,
        showError,
    ]);

    const recordController = useMemo(() => createRecordController({
        logger,
        config,
        inputSource,
        activeInputSourceRef,
        usingNativeCaptureRef,
        liveDraftRef,
        setAudioUrl,
        setAudioFile,
        setIsInitializing,
        setIsTransitioning,
        setIsPaused,
        showError,
        capture,
        session,
        timing,
        persistence,
    }), [
        capture,
        config,
        inputSource,
        persistence,
        session,
        setAudioFile,
        setAudioUrl,
        setIsPaused,
        showError,
        timing,
    ]);

    useEffect(() => {
        if (!isRecording || isPaused) {
            return;
        }

        timing.syncRecordingElapsedMs();
        const interval = window.setInterval(timing.syncRecordingElapsedMs, 250);
        return () => window.clearInterval(interval);
    }, [isPaused, isRecording, timing]);

    const startRecording = useCallback(async () => {
        if (!config.streamingModelPath) {
            const onboardingStore = useOnboardingStore.getState();
            useTranscriptRuntimeStore.getState().setMode('live');
            onboardingStore.reopen(
                getResumeOnboardingStep(config, 'live_record', onboardingStore.persistedState),
                'live_record'
            );
            return false;
        }
        peakLevelRef.current = 0;
        return recordController.startRecording();
    }, [config, recordController]);

    const stopRecording = useCallback(async () => {
        return recordController.stopRecording();
    }, [recordController]);

    const pauseRecording = useCallback(async () => {
        peakLevelRef.current = 0;
        return recordController.pauseRecording();
    }, [recordController]);

    const resumeRecording = useCallback(async () => {
        return recordController.resumeRecording();
    }, [recordController]);

    useEffect(() => {
        return () => {
            void capture.teardownWebCaptureResources().catch((error) => {
                logger.error('Error closing audio recorder web capture resources:', error);
            });
            transcriptionService.terminate().catch((error) => logger.error('Error stopping transcription service:', error));
        };
    }, [capture]);

    return {
        startRecording,
        stopRecording,
        pauseRecording,
        resumeRecording,
        isRecording,
        isPaused,
        isInitializing,
        isTransitioning,
        recordingElapsedMs,
        peakLevelRef
    };
}
