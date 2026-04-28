import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { remove, writeFile } from '@tauri-apps/plugin-fs';
import { useConfigStore } from '../stores/configStore';
import { useDialogStore } from '../stores/dialogStore';
import { useHistoryStore } from '../stores/historyStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useProjectStore } from '../stores/projectStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from '../services/historyService';
import { speakerService } from '../services/speakerService';
import { summaryService } from '../services/summaryService';
import { transcriptionService } from '../services/transcriptionService';
import type { TranscriptSegment } from '../types/transcript';
import { logger } from '../utils/logger';
import { getResumeOnboardingStep } from '../utils/onboarding';
import { createAudioRecorderCapture, getSupportedMimeType } from './audioRecorder/capture';
import {
    createRecordingPersistence,
    getRecordedAudioExtension,
    syncSavedRecordingMeta,
} from './audioRecorder/persistence';
import { createRecordSessionController } from './audioRecorder/session';
import { createRecordTimingController } from './audioRecorder/timing';
import type {
    InputSource,
    RecordSegmentDeliveryMeta,
    RecordSessionPhase,
} from './audioRecorder/types';
import type { LiveRecordingDraftHandle } from '../services/historyService';
import { flushPendingAutoSave } from './useAutoSaveTranscript';

export type {
    RecordSegmentDeliveryMeta,
    RecordSessionPhase,
} from './audioRecorder/types';

export { getSupportedMimeType };

interface UseAudioRecorderProps {
    inputSource: InputSource;
    onSegment: (segment: TranscriptSegment, meta: RecordSegmentDeliveryMeta) => void;
}

export function useAudioRecorder({ inputSource, onSegment }: UseAudioRecorderProps) {
    const config = useConfigStore((state) => state.config);
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const setIsRecording = useTranscriptStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptStore((state) => state.setIsPaused);
    const finalizeLastSegment = useTranscriptStore((state) => state.finalizeLastSegment);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);
    const setAudioUrl = useTranscriptStore((state) => state.setAudioUrl);
    const setAudioFile = useTranscriptStore((state) => state.setAudioFile);
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
        clearSegments,
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
        clearSegments,
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
        getTranscriptState: () => useTranscriptStore.getState(),
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
            syncSavedRecordingMeta(useTranscriptStore.getState(), title, historyId, icon)
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

    const forwardRecordSegment = useCallback((segment: TranscriptSegment) => {
        const meta = session.getRecordSegmentDeliveryMeta(segment);
        const normalizedSegment = timing.normalizeRecordSegmentTiming(segment);
        if (meta.accepted) {
            timing.trackAcceptedSegment(normalizedSegment);
        }
        logger.info(
            `[useAudioRecorder] Record segment received. session=${meta.sessionId ?? 'none'} phase=${meta.phase} accepted=${meta.accepted} final=${segment?.isFinal === true} offset=${segmentTimeOffsetSecondsRef.current.toFixed(3)} start=${normalizedSegment.start.toFixed(3)} end=${normalizedSegment.end.toFixed(3)}`
        );
        onSegmentRef.current(normalizedSegment, meta);
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
            useTranscriptStore.getState().setMode('live');
            onboardingStore.reopen(
                getResumeOnboardingStep(config, 'live_record', onboardingStore.persistedState),
                'live_record'
            );
            return false;
        }

        const sessionId = session.openRecordSession();
        setAudioUrl(null);
        setAudioFile(null);
        setIsInitializing(true);
        setIsTransitioning(true);

        try {
            activeInputSourceRef.current = inputSource;
            liveDraftRef.current = null;

            const createLiveDraft = async (audioExtension: string) => {
                const draft = await persistence.createLiveRecordingDraft(audioExtension);
                liveDraftRef.current = draft;
                return draft;
            };

            let fallbackStream: MediaStream | undefined;
            if (inputSource === 'desktop') {
                let nativeDraft = await createLiveDraft('wav');
                const nativeStarted = await capture.tryStartNativeDesktopCapture(
                    sessionId,
                    config.systemAudioDeviceId && config.systemAudioDeviceId !== 'default'
                        ? config.systemAudioDeviceId
                        : null,
                    nativeDraft.audioAbsolutePath,
                );

                if (!nativeStarted) {
                    await persistence.discardLiveRecordingDraft(nativeDraft);
                    nativeDraft = await createLiveDraft(
                        getRecordedAudioExtension(getSupportedMimeType() || 'audio/webm'),
                    );
                    fallbackStream = await capture.requestWebFallbackStream('desktop', config.microphoneId);
                }
            } else {
                let nativeDraft = await createLiveDraft('wav');
                const nativeStarted = await capture.tryStartNativeMicrophoneCapture(sessionId, {
                    deviceName: config.microphoneId && config.microphoneId !== 'default'
                        ? config.microphoneId
                        : null,
                    boost: config.microphoneBoost ?? 1.0,
                    muteDuringRecording: config.muteDuringRecording ?? false,
                    outputPath: nativeDraft.audioAbsolutePath,
                });

                if (!nativeStarted) {
                    await persistence.discardLiveRecordingDraft(nativeDraft);
                    nativeDraft = await createLiveDraft(
                        getRecordedAudioExtension(getSupportedMimeType() || 'audio/webm'),
                    );
                    fallbackStream = await capture.requestWebFallbackStream('microphone', config.microphoneId);
                }
            }

            if (!usingNativeCaptureRef.current && fallbackStream) {
                await capture.attachWebStream(sessionId, fallbackStream, inputSource, config.muteDuringRecording ?? false);
            }

            if (!capture.startFileRecording(sessionId)) {
                throw new Error(`Failed to activate record session ${sessionId}`);
            }

            logger.info(
                `[useAudioRecorder] Record session UI ready. session=${sessionId} native=${usingNativeCaptureRef.current}`
            );
            return true;
        } catch (error) {
            logger.error(`[useAudioRecorder] Record session start failed. session=${sessionId}:`, error);
            await capture.cleanupPartialStart(sessionId);
            await session.softStopRecordSessionIfActive(sessionId, 'start_failed');
            if (liveDraftRef.current) {
                try {
                    await persistence.discardLiveRecordingDraft(liveDraftRef.current);
                } finally {
                    liveDraftRef.current = null;
                }
            }
            session.resetRecordSession(sessionId, 'start_failed', true);

            if (session.canMutateActiveRecordResources(sessionId)) {
                await showError({
                    code: inputSource === 'microphone' ? 'audio.microphone_failed' : 'audio.capture_failed',
                    messageKey: inputSource === 'microphone' ? 'errors.audio.microphone_failed' : 'errors.audio.capture_failed',
                    cause: error,
                });
            } else {
                logger.info(
                    `[useAudioRecorder] Suppressed stale start failure dialog. requested=${sessionId} active=${session.getSessionId() ?? 'none'}`
                );
            }
            return false;
        } finally {
            if (session.isActiveSession(sessionId) && session.getPhase() === 'recording') {
                setIsTransitioning(false);
            }
            if (session.shouldFinalizeRecordStartAttempt(sessionId)) {
                setIsInitializing(false);
            } else {
                logger.info(
                    `[useAudioRecorder] Skipping stale start finalization. requested=${sessionId} active=${session.getSessionId() ?? 'none'}`
                );
            }
        }
    }, [capture, config, inputSource, persistence, session, setAudioFile, setAudioUrl, showError]);

    const stopRecording = useCallback(async () => {
        const sessionId = session.getSessionId();
        if (!sessionId) {
            return;
        }
        const liveDraft = liveDraftRef.current;

        setIsTransitioning(true);
        const previousPhase = session.getPhase();
        session.setPhase('stopping');
        logger.info(`[useAudioRecorder] Stopping recording session. session=${sessionId} input=${activeInputSourceRef.current}`);
        const duration = timing.finalizeRecordedDurationSeconds();

        const savedWavPath = await capture.stopCaptureForSession(sessionId);

        // Stop recognizer delivery before we persist outputs so no late segment
        // can land after the saved recording has already been finalized.
        await transcriptionService.softStop();
        finalizeLastSegment();
        const latestSegments = useTranscriptStore.getState().segments;
        if (liveDraft?.item.id) {
            await flushPendingAutoSave(liveDraft.item.id, latestSegments);
        }

        if (!liveDraft || latestSegments.length === 0) {
            liveDraftRef.current = null;
            capture.stopFileRecording();
            await capture.teardownWebCaptureResources();

            if (config.muteDuringRecording) {
                void capture.setSystemAudioMute(false, 'Failed to unmute system audio:');
            }

            if (savedWavPath) {
                try {
                    await remove(savedWavPath);
                } catch (error) {
                    logger.warn('[useAudioRecorder] Failed to remove discarded native recording file:', error);
                }
            }

            if (liveDraft) {
                await persistence.discardLiveRecordingDraft(liveDraft);
            }

            timing.clearFinalizedDurationSeconds();
            logger.info(
                `[useAudioRecorder] Recording session stopped without transcript. session=${sessionId} previous_phase=${previousPhase} duration=${duration.toFixed(3)}`
            );
            session.resetRecordSession(sessionId, 'stop_completed');
            return;
        }

        if (usingNativeCaptureRef.current) {
            if (savedWavPath) {
                await persistence.persistNativeRecording(liveDraft, savedWavPath, duration);
            }
            liveDraftRef.current = null;
            usingNativeCaptureRef.current = false;
            timing.clearFinalizedDurationSeconds();
        }

        capture.stopFileRecording();
        await capture.teardownWebCaptureResources();

        if (config.muteDuringRecording) {
            void capture.setSystemAudioMute(false, 'Failed to unmute system audio:');
        }
        logger.info(
            `[useAudioRecorder] Recording session stopped. session=${sessionId} previous_phase=${previousPhase} duration=${duration.toFixed(3)}`
        );
        session.resetRecordSession(sessionId, 'stop_completed');
    }, [capture, config.muteDuringRecording, finalizeLastSegment, persistence, session, timing]);

    const pauseRecording = useCallback(async () => {
        const sessionId = session.getSessionId();
        if (!sessionId || session.getPhase() !== 'recording') {
            return;
        }

        logger.info(`[useAudioRecorder] Pausing recording session. session=${sessionId}`);
        session.setPhase('pausing');
        setIsTransitioning(true);
        peakLevelRef.current = 0;
        timing.pauseRecordedDurationWindow();
        timing.syncRecordingElapsedMs();

        try {
            // Pause capture first, then pause the recognizer stream, so no new
            // samples sneak in after we freeze the visible recording duration.
            await capture.pauseCapture(sessionId);
            await transcriptionService.pauseStream();
            if (liveDraftRef.current?.item.id) {
                await flushPendingAutoSave(
                    liveDraftRef.current.item.id,
                    useTranscriptStore.getState().segments,
                );
            }

            if (config.muteDuringRecording) {
                void capture.setSystemAudioMute(false, 'Failed to unmute system audio on pause:');
            }

            if (!session.isActiveSession(sessionId)) {
                return;
            }

            setIsPaused(true);
            session.setPhase('paused');
            logger.info(`[useAudioRecorder] Recording session paused. session=${sessionId}`);
        } catch (error) {
            logger.error(`[useAudioRecorder] Failed to pause recording session. session=${sessionId}:`, error);

            if (!session.isActiveSession(sessionId)) {
                return;
            }

            // If pause fails midway, restore recognizer/capture state and move
            // the segment offset forward so resumed timestamps never overlap the
            // already accepted timeline.
            try {
                await transcriptionService.resumeStream();
                timing.setSegmentTimeOffsetSeconds(timing.getNextSegmentTimeOffsetSeconds(), 'pause_error_recovery');
            } catch (resumeError) {
                logger.warn(`[useAudioRecorder] Failed to restore recognizer after pause error. session=${sessionId}:`, resumeError);
            }

            try {
                await capture.resumeCapture(sessionId);
            } catch (restoreError) {
                logger.warn(`[useAudioRecorder] Failed to restore capture after pause error. session=${sessionId}:`, restoreError);
            }

            if (config.muteDuringRecording && activeInputSourceRef.current === 'microphone') {
                void capture.setSystemAudioMute(true, 'Failed to remute system audio after pause rollback:');
            }

            timing.beginRecordedDurationWindow();
            timing.syncRecordingElapsedMs();
            setIsPaused(false);
            session.setPhase('recording');
        } finally {
            if (session.isActiveSession(sessionId) && session.getPhase() !== 'stopping') {
                setIsTransitioning(false);
            }
        }
    }, [capture, config.muteDuringRecording, session, timing]);

    const resumeRecording = useCallback(async () => {
        const sessionId = session.getSessionId();
        if (!sessionId || session.getPhase() !== 'paused') {
            return;
        }

        logger.info(`[useAudioRecorder] Resuming recording session. session=${sessionId}`);
        session.setPhase('resuming');
        setIsTransitioning(true);

        try {
            // Resume recognizer first, then advance the segment time offset, and
            // only then let capture continue feeding audio into the pipeline.
            await transcriptionService.resumeStream();
            timing.setSegmentTimeOffsetSeconds(timing.getNextSegmentTimeOffsetSeconds(), 'resume');

            await capture.resumeCapture(sessionId);

            if (config.muteDuringRecording && activeInputSourceRef.current === 'microphone') {
                void capture.setSystemAudioMute(true, 'Failed to mute system audio on resume:');
            }

            if (!session.isActiveSession(sessionId)) {
                return;
            }

            timing.beginRecordedDurationWindow();
            timing.syncRecordingElapsedMs();
            setIsPaused(false);
            session.setPhase('recording');
            logger.info(`[useAudioRecorder] Recording session resumed. session=${sessionId}`);
        } catch (error) {
            logger.error(`[useAudioRecorder] Failed to resume recording session. session=${sessionId}:`, error);
            if (session.isActiveSession(sessionId)) {
                setIsPaused(true);
                session.setPhase('paused');
            }
        } finally {
            if (session.isActiveSession(sessionId) && session.getPhase() !== 'stopping') {
                setIsTransitioning(false);
            }
        }
    }, [capture, config.muteDuringRecording, session, timing]);

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
