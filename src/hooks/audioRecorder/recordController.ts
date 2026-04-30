import { remove } from '@tauri-apps/plugin-fs';
import { finalizeLastTranscriptSegment } from '../../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { transcriptionService } from '../../services/transcriptionService';
import type { AppConfig } from '../../types/config';
import type { LiveRecordingDraftHandle } from '../../services/historyService';
import { flushPendingAutoSave } from '../useAutoSaveTranscript';
import { getSupportedMimeType } from './capture';
import { getRecordedAudioExtension } from './persistence';
import type {
    AudioRecorderLogger,
    InputSource,
    MutableRefLike,
    RecordSessionPhase,
} from './types';

interface RecordControllerCapture {
    tryStartNativeDesktopCapture: (
        sessionId: string,
        deviceName: string | null,
        outputPath: string,
    ) => Promise<boolean>;
    tryStartNativeMicrophoneCapture: (
        sessionId: string,
        options: {
            deviceName: string | null;
            boost: number;
            muteDuringRecording: boolean;
            outputPath: string;
        },
    ) => Promise<boolean>;
    requestWebFallbackStream: (inputSource: InputSource, microphoneId: string | undefined) => Promise<MediaStream>;
    attachWebStream: (
        sessionId: string,
        stream: MediaStream,
        inputSource: InputSource,
        muteDuringRecording: boolean,
    ) => Promise<void>;
    startFileRecording: (sessionId: string) => boolean;
    cleanupPartialStart: (sessionId: string) => Promise<void>;
    stopCaptureForSession: (sessionId: string) => Promise<string | null>;
    stopFileRecording: () => void;
    teardownWebCaptureResources: () => Promise<void>;
    pauseCapture: (sessionId: string) => Promise<void>;
    resumeCapture: (sessionId: string) => Promise<void>;
    setSystemAudioMute: (mute: boolean, errorMessage: string) => Promise<void>;
}

interface RecordControllerSession {
    openRecordSession: () => string;
    softStopRecordSessionIfActive: (sessionId: string, reason: string) => Promise<void>;
    resetRecordSession: (sessionId: string | null, reason: string, clearTranscript?: boolean) => void;
    canMutateActiveRecordResources: (sessionId: string) => boolean;
    shouldFinalizeRecordStartAttempt: (sessionId: string) => boolean;
    isActiveSession: (sessionId: string) => boolean;
    getSessionId: () => string | null;
    getPhase: () => RecordSessionPhase;
    setPhase: (phase: RecordSessionPhase) => void;
}

interface RecordControllerTiming {
    clearFinalizedDurationSeconds: () => void;
    getFinalizedDurationSeconds: () => number | null;
    getRecordedDurationSeconds: () => number;
    finalizeRecordedDurationSeconds: () => number;
    pauseRecordedDurationWindow: () => void;
    syncRecordingElapsedMs: () => void;
    setSegmentTimeOffsetSeconds: (offsetSeconds: number, reason: string) => void;
    getNextSegmentTimeOffsetSeconds: () => number;
    beginRecordedDurationWindow: () => void;
}

interface RecordControllerPersistence {
    createLiveRecordingDraft: (audioExtension: string) => Promise<LiveRecordingDraftHandle>;
    discardLiveRecordingDraft: (draft: LiveRecordingDraftHandle) => Promise<void>;
    persistNativeRecording: (
        draft: LiveRecordingDraftHandle,
        savedWavPath: string,
        duration: number,
    ) => Promise<void>;
}

interface CreateRecordControllerArgs {
    logger: AudioRecorderLogger;
    config: AppConfig;
    inputSource: InputSource;
    activeInputSourceRef: MutableRefLike<InputSource>;
    usingNativeCaptureRef: MutableRefLike<boolean>;
    liveDraftRef: MutableRefLike<LiveRecordingDraftHandle | null>;
    setAudioUrl: (url: string | null) => void;
    setAudioFile: (file: File | null) => void;
    setIsInitializing: (value: boolean) => void;
    setIsTransitioning: (value: boolean) => void;
    setIsPaused: (value: boolean) => void;
    showError: (input: {
        code: string;
        messageKey: string;
        cause: unknown;
    }) => Promise<void>;
    capture: RecordControllerCapture;
    session: RecordControllerSession;
    timing: RecordControllerTiming;
    persistence: RecordControllerPersistence;
}

export function createRecordController({
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
}: CreateRecordControllerArgs) {
    async function startRecording(): Promise<boolean> {
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
                await capture.attachWebStream(
                    sessionId,
                    fallbackStream,
                    inputSource,
                    config.muteDuringRecording ?? false,
                );
            }

            if (!capture.startFileRecording(sessionId)) {
                throw new Error(`Failed to activate record session ${sessionId}`);
            }

            logger.info(
                `[useAudioRecorder] Record session UI ready. session=${sessionId} native=${usingNativeCaptureRef.current}`,
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
                    `[useAudioRecorder] Suppressed stale start failure dialog. requested=${sessionId} active=${session.getSessionId() ?? 'none'}`,
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
                    `[useAudioRecorder] Skipping stale start finalization. requested=${sessionId} active=${session.getSessionId() ?? 'none'}`,
                );
            }
        }
    }

    async function stopRecording(): Promise<void> {
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
        finalizeLastTranscriptSegment();
        const latestSegments = useTranscriptSessionStore.getState().segments;
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
                `[useAudioRecorder] Recording session stopped without transcript. session=${sessionId} previous_phase=${previousPhase} duration=${duration.toFixed(3)}`,
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
            `[useAudioRecorder] Recording session stopped. session=${sessionId} previous_phase=${previousPhase} duration=${duration.toFixed(3)}`,
        );
        session.resetRecordSession(sessionId, 'stop_completed');
    }

    async function pauseRecording(): Promise<void> {
        const sessionId = session.getSessionId();
        if (!sessionId || session.getPhase() !== 'recording') {
            return;
        }

        logger.info(`[useAudioRecorder] Pausing recording session. session=${sessionId}`);
        session.setPhase('pausing');
        setIsTransitioning(true);
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
                    useTranscriptSessionStore.getState().segments,
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
                logger.warn(
                    `[useAudioRecorder] Failed to restore recognizer after pause error. session=${sessionId}:`,
                    resumeError,
                );
            }

            try {
                await capture.resumeCapture(sessionId);
            } catch (restoreError) {
                logger.warn(
                    `[useAudioRecorder] Failed to restore capture after pause error. session=${sessionId}:`,
                    restoreError,
                );
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
    }

    async function resumeRecording(): Promise<void> {
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
    }

    return {
        startRecording,
        stopRecording,
        pauseRecording,
        resumeRecording,
    };
}
