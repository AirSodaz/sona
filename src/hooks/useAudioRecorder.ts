import { useRef, useState, useCallback, useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useConfigStore } from '../stores/configStore';
import { useHistoryStore } from '../stores/historyStore';
import { useOnboardingStore } from '../stores/onboardingStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { historyService } from '../services/historyService';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { remove } from '@tauri-apps/plugin-fs';
import { getResumeOnboardingStep } from '../utils/onboarding';
import { logger } from '../utils/logger';
import { TranscriptSegment } from '../types/transcript';

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

interface UseAudioRecorderProps {
    inputSource: 'microphone' | 'desktop';
    onSegment: (segment: TranscriptSegment, meta: RecordSegmentDeliveryMeta) => void;
}

function createRecordSessionId(): string {
    return `record-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function shouldAcceptRecordSegment(phase: RecordSessionPhase, segment: TranscriptSegment): boolean {
    if (phase === 'starting' || phase === 'recording' || phase === 'resuming') {
        return true;
    }

    if ((phase === 'pausing' || phase === 'stopping') && segment.isFinal) {
        return true;
    }

    return false;
}

function shouldFeedWebAudioForPhase(phase: RecordSessionPhase): boolean {
    return phase === 'starting' || phase === 'recording' || phase === 'resuming';
}

/**
 * Determines the supported audio MIME type for the current browser.
 */
export function getSupportedMimeType(): string {
    const types = [
        'audio/webm;codecs=opus',
        'audio/webm',
        'audio/mp4',
        'audio/aac',
        'audio/ogg',
        ''
    ];

    for (const type of types) {
        if (type === '' || MediaRecorder.isTypeSupported(type)) {
            return type;
        }
    }
    return '';
}

export function useAudioRecorder({ inputSource, onSegment }: UseAudioRecorderProps) {
    // Store Access
    const config = useConfigStore((state) => state.config);
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const setIsRecording = useTranscriptStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptStore((state) => state.setIsPaused);
    const finalizeLastSegment = useTranscriptStore((state) => state.finalizeLastSegment);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);
    const showError = useDialogStore((state) => state.showError);

    // Refs
    const audioContextRef = useRef<AudioContext | null>(null);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const activeStreamRef = useRef<MediaStream | null>(null);
    const nativeAudioUnlistenRef = useRef<UnlistenFn | null>(null);
    const usingNativeCaptureRef = useRef(false);
    const mimeTypeRef = useRef<string>('');
    const peakLevelRef = useRef<number>(0);
    const activeInputSourceRef = useRef<'microphone' | 'desktop'>(inputSource);
    const onSegmentRef = useRef(onSegment);
    const recordSessionIdRef = useRef<string | null>(null);
    const recordSessionPhaseRef = useRef<RecordSessionPhase>('idle');
    const recordedDurationMsRef = useRef(0);
    const activeDurationStartedAtRef = useRef<number | null>(null);
    const finalizedDurationSecondsRef = useRef<number | null>(null);
    const segmentTimeOffsetSecondsRef = useRef(0);
    const recordTimelineCursorSecondsRef = useRef(0);

    // State
    const [isInitializing, setIsInitializing] = useState(false);
    const [isTransitioning, setIsTransitioning] = useState(false);
    const [recordingElapsedMs, setRecordingElapsedMs] = useState(0);

    // Sync refs for callbacks
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

    const resetRecordedDuration = useCallback(() => {
        recordedDurationMsRef.current = 0;
        activeDurationStartedAtRef.current = null;
    }, []);

    const beginRecordedDurationWindow = useCallback(() => {
        activeDurationStartedAtRef.current = Date.now();
    }, []);

    const pauseRecordedDurationWindow = useCallback(() => {
        if (activeDurationStartedAtRef.current !== null) {
            recordedDurationMsRef.current += Date.now() - activeDurationStartedAtRef.current;
            activeDurationStartedAtRef.current = null;
        }
    }, []);

    const getRecordedDurationMs = useCallback((): number => {
        let durationMs = recordedDurationMsRef.current;
        if (activeDurationStartedAtRef.current !== null) {
            durationMs += Date.now() - activeDurationStartedAtRef.current;
        }
        return durationMs;
    }, []);

    const syncRecordingElapsedMs = useCallback(() => {
        setRecordingElapsedMs(getRecordedDurationMs());
    }, [getRecordedDurationMs]);

    const getRecordedDurationSeconds = useCallback((): number => {
        return getRecordedDurationMs() / 1000;
    }, [getRecordedDurationMs]);

    const finalizeRecordedDurationSeconds = useCallback((): number => {
        pauseRecordedDurationWindow();
        syncRecordingElapsedMs();
        const durationSeconds = getRecordedDurationSeconds();
        finalizedDurationSecondsRef.current = durationSeconds;
        return durationSeconds;
    }, [getRecordedDurationSeconds, pauseRecordedDurationWindow, syncRecordingElapsedMs]);

    const resetRecordTimeline = useCallback(() => {
        segmentTimeOffsetSecondsRef.current = 0;
        recordTimelineCursorSecondsRef.current = 0;
    }, []);

    const getNextSegmentTimeOffsetSeconds = useCallback((): number => {
        return Math.max(recordTimelineCursorSecondsRef.current, getRecordedDurationSeconds());
    }, [getRecordedDurationSeconds]);

    const setSegmentTimeOffsetSeconds = useCallback((offsetSeconds: number, reason: string) => {
        segmentTimeOffsetSecondsRef.current = offsetSeconds;
        logger.info(
            `[useAudioRecorder] Updated record segment timeline offset. session=${recordSessionIdRef.current ?? 'none'} offset=${offsetSeconds.toFixed(3)} reason=${reason}`
        );
    }, []);

    const normalizeRecordSegmentTiming = useCallback((segment: TranscriptSegment): TranscriptSegment => {
        const adjustedStart = segment.start + segmentTimeOffsetSecondsRef.current;
        const adjustedEnd = segment.end + segmentTimeOffsetSecondsRef.current;

        return {
            ...segment,
            start: adjustedStart,
            end: adjustedEnd,
            timestamps: segment.timestamps?.map((timestamp) => timestamp + adjustedStart)
        };
    }, []);

    useEffect(() => {
        if (!isRecording || isPaused) {
            return;
        }

        syncRecordingElapsedMs();
        const interval = window.setInterval(syncRecordingElapsedMs, 250);
        return () => window.clearInterval(interval);
    }, [isPaused, isRecording, syncRecordingElapsedMs]);

    const getRecordSegmentDeliveryMeta = useCallback((segment: TranscriptSegment): RecordSegmentDeliveryMeta => {
        const phase = recordSessionPhaseRef.current;
        return {
            sessionId: recordSessionIdRef.current,
            phase,
            isRecording: isRecordingRef.current,
            accepted: recordSessionIdRef.current !== null && shouldAcceptRecordSegment(phase, segment)
        };
    }, []);

    const openRecordSession = useCallback((): string => {
        const sessionId = createRecordSessionId();
        recordSessionIdRef.current = sessionId;
        recordSessionPhaseRef.current = 'starting';
        peakLevelRef.current = 0;
        finalizedDurationSecondsRef.current = null;
        resetRecordedDuration();
        resetRecordTimeline();
        setRecordingElapsedMs(0);
        setIsRecording(false);
        setIsPaused(false);
        clearSegments();
        logger.info(`[useAudioRecorder] Record session opened. session=${sessionId} input=${inputSource}`);
        return sessionId;
    }, [clearSegments, inputSource, resetRecordTimeline, resetRecordedDuration, setIsPaused, setIsRecording]);

    const activateRecordSession = useCallback((sessionId: string): boolean => {
        if (recordSessionIdRef.current !== sessionId) {
            logger.warn(
                `[useAudioRecorder] Skipping record session activation because the active session changed. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'}`
            );
            return false;
        }

        recordSessionPhaseRef.current = 'recording';
        setIsRecording(true);
        setIsPaused(false);
        beginRecordedDurationWindow();
        syncRecordingElapsedMs();
        logger.info(`[useAudioRecorder] Record session UI ready. session=${sessionId} native=${usingNativeCaptureRef.current}`);
        return true;
    }, [beginRecordedDurationWindow, setIsPaused, setIsRecording, syncRecordingElapsedMs]);

    const canMutateActiveRecordResources = useCallback((sessionId: string): boolean => {
        return recordSessionIdRef.current === sessionId || recordSessionIdRef.current === null;
    }, []);

    const resetRecordSession = useCallback((sessionId: string | null, reason: string, clearTranscript = false) => {
        if (sessionId && recordSessionIdRef.current !== sessionId) {
            logger.info(
                `[useAudioRecorder] Ignoring record session reset for stale session. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'} reason=${reason}`
            );
            return;
        }

        const activeSessionId = recordSessionIdRef.current;
        const previousPhase = recordSessionPhaseRef.current;
        if (clearTranscript) {
            clearSegments();
        }
        recordSessionIdRef.current = null;
        recordSessionPhaseRef.current = 'idle';
        peakLevelRef.current = 0;
        resetRecordedDuration();
        resetRecordTimeline();
        setRecordingElapsedMs(0);
        setIsRecording(false);
        setIsPaused(false);
        setIsTransitioning(false);
        logger.info(
            `[useAudioRecorder] Record session reset. session=${activeSessionId ?? 'none'} previous_phase=${previousPhase} reason=${reason} cleared=${clearTranscript}`
        );
    }, [clearSegments, resetRecordTimeline, resetRecordedDuration, setIsPaused, setIsRecording]);

    const cleanupPartialStart = useCallback(async (sessionId: string) => {
        if (!canMutateActiveRecordResources(sessionId)) {
            logger.info(
                `[useAudioRecorder] Skipping shared resource rollback for stale session. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'}`
            );
            return;
        }

        if (usingNativeCaptureRef.current) {
            if (nativeAudioUnlistenRef.current) {
                nativeAudioUnlistenRef.current();
                nativeAudioUnlistenRef.current = null;
            }
            const stopCmd = activeInputSourceRef.current === 'desktop'
                ? 'stop_system_audio_capture'
                : 'stop_microphone_capture';
            try {
                await invoke(stopCmd, { instanceId: 'record' });
                logger.info(`[useAudioRecorder] Rolled back native capture. session=${sessionId} command=${stopCmd}`);
            } catch (err) {
                logger.warn(`[useAudioRecorder] Failed to roll back native capture. session=${sessionId} command=${stopCmd}`, err);
            }
            usingNativeCaptureRef.current = false;
        }

        if (activeStreamRef.current) {
            activeStreamRef.current.getTracks().forEach((track) => track.stop());
            activeStreamRef.current = null;
        }

        if (audioContextRef.current) {
            try {
                if (audioContextRef.current.state !== 'closed') {
                    await audioContextRef.current.close();
                }
            } catch (err) {
                logger.warn(`[useAudioRecorder] Failed to close audio context during rollback. session=${sessionId}`, err);
            }
            audioContextRef.current = null;
        }

        mediaRecorderRef.current = null;
    }, [canMutateActiveRecordResources]);

    const softStopRecordSessionIfActive = useCallback(async (sessionId: string, reason: string) => {
        if (!canMutateActiveRecordResources(sessionId)) {
            logger.info(
                `[useAudioRecorder] Skipping recognizer rollback for stale session. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'} reason=${reason}`
            );
            return;
        }

        try {
            await transcriptionService.softStop();
            logger.info(`[useAudioRecorder] Rolled back recognizer state. session=${sessionId} reason=${reason}`);
        } catch (error) {
            logger.warn(`[useAudioRecorder] Failed to roll back recognizer state. session=${sessionId} reason=${reason}`, error);
        }
    }, [canMutateActiveRecordResources]);

    const shouldFinalizeRecordStartAttempt = useCallback((sessionId: string): boolean => {
        return recordSessionIdRef.current === sessionId || recordSessionIdRef.current === null;
    }, []);

    const attachNativePeakListener = useCallback(async (
        eventName: 'system-audio' | 'microphone-audio',
        sessionId: string
    ): Promise<boolean> => {
        try {
            const unlisten = await listen<number>(eventName, (event) => {
                const peak = Math.abs(event.payload);
                const sample = Math.min(32767, Math.round(peak));

                // This event only drives the live waveform meter. Native capture and
                // backend transcription keep running even if the UI listener is unavailable.
                if (!isPausedRef.current) {
                    peakLevelRef.current = sample / 32767;
                }
            });

            nativeAudioUnlistenRef.current = unlisten;
            return true;
        } catch (error) {
            nativeAudioUnlistenRef.current = null;
            logger.warn(
                `[useAudioRecorder] Failed to attach native peak listener. session=${sessionId} event=${eventName}. Continuing without live meter.`,
                error
            );
            return false;
        }
    }, []);

    const forwardRecordSegment = useCallback((segment: TranscriptSegment) => {
        const meta = getRecordSegmentDeliveryMeta(segment);
        const normalizedSegment = normalizeRecordSegmentTiming(segment);
        if (meta.accepted) {
            recordTimelineCursorSecondsRef.current = Math.max(recordTimelineCursorSecondsRef.current, normalizedSegment.end);
        }
        logger.info(
            `[useAudioRecorder] Record segment received. session=${meta.sessionId ?? 'none'} phase=${meta.phase} accepted=${meta.accepted} final=${segment?.isFinal === true} offset=${segmentTimeOffsetSecondsRef.current.toFixed(3)} start=${normalizedSegment.start.toFixed(3)} end=${normalizedSegment.end.toFixed(3)}`
        );
        onSegmentRef.current(normalizedSegment, meta);
    }, [getRecordSegmentDeliveryMeta, normalizeRecordSegmentTiming]);

    // Initialize Native Session
    // IMPORTANT: This must be called BEFORE starting audio capture to avoid a race
    // condition where audio is fed to Sherpa before the recognizer is initialized.
    const initializeNativeSession = useCallback(async (sessionId: string) => {
        logger.info(`[useAudioRecorder] Initializing transcription service (native). session=${sessionId}`);
        try {
            await transcriptionService.start(
                forwardRecordSegment,
                (error) => {
                    logger.error(`[useAudioRecorder] Transcription error callback. session=${sessionId}:`, error);
                    showError({
                        code: 'transcription.service_error',
                        messageKey: 'errors.transcription.service_error',
                        cause: error,
                    });
                },
                {
                    callbackOwner: 'live-record',
                    callbackSessionId: sessionId
                }
            );
            logger.info(`[useAudioRecorder] Record session recognizer ready. session=${sessionId} transport=native`);
        } catch (err) {
            logger.error(`[useAudioRecorder] Failed to start transcription service. session=${sessionId}:`, err);
            throw err; // Re-throw to be caught by startRecording
        }
    }, [forwardRecordSegment, showError]);

    // Initialize Audio Session (Web API)
    const initializeAudioSession = useCallback(async (stream: MediaStream, sessionId: string) => {
        if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        } else if (audioContextRef.current.state === 'suspended') {
            await audioContextRef.current.resume();
        }

        // CRITICAL: Initialize transcription service BEFORE connecting the audio graph.
        // This ensures isRunning=true before any audio samples arrive via onmessage,
        // preventing initial audio data from being silently dropped.
        logger.info(`[useAudioRecorder] Initializing transcription service (web audio). session=${sessionId}`);
        await transcriptionService.start(
            forwardRecordSegment,
            (error) => { logger.error(`[useAudioRecorder] Transcription error. session=${sessionId}:`, error); },
            {
                callbackOwner: 'live-record',
                callbackSessionId: sessionId
            }
        );
        logger.info(`[useAudioRecorder] Record session recognizer ready. session=${sessionId} transport=web-audio`);

        const source = audioContextRef.current.createMediaStreamSource(stream);

        // Processor
        try {
            await audioContextRef.current.audioWorklet.addModule('/audio-processor.js');
        } catch (err) {
            logger.error('Failed to load audio worklet module:', err);
            throw Object.assign(new Error('Audio worklet failed to load'), { cause: err });
        }

        const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');
        processor.port.onmessage = (e) => {
            const samples = e.data as Int16Array;
            if (shouldFeedWebAudioForPhase(recordSessionPhaseRef.current)) {
                transcriptionService.sendAudioInt16(samples);
            }
            if (!isPausedRef.current) {
                setPeakFromInt16(samples);
            }
        };

        source.connect(processor);
        processor.connect(audioContextRef.current.destination);
    }, [forwardRecordSegment, setPeakFromInt16]);

    // File Recording (MediaRecorder)
    const startFileRecording = useCallback((sessionId: string): boolean => {
        if (usingNativeCaptureRef.current) {
            return activateRecordSession(sessionId);
        }

        const stream = activeStreamRef.current;
        if (!stream) {
            logger.error("No active stream to record");
            return false;
        }

        const mimeType = getSupportedMimeType();
        mimeTypeRef.current = mimeType;
        const options = mimeType ? { mimeType } : undefined;

        mediaRecorderRef.current = new MediaRecorder(stream, options);
        const chunks: Blob[] = [];

        mediaRecorderRef.current.ondataavailable = (e) => {
            chunks.push(e.data);
        };

        mediaRecorderRef.current.onstop = async () => {
            const type = mimeTypeRef.current || mediaRecorderRef.current?.mimeType || 'audio/webm';
            const blob = new Blob(chunks, { type });
            const url = URL.createObjectURL(blob);
            useTranscriptStore.getState().setAudioUrl(url);

            const segments = useTranscriptStore.getState().segments;
            const duration = finalizedDurationSecondsRef.current ?? getRecordedDurationSeconds();

            if (segments.length > 0) {
                const newItem = await historyService.saveRecording(blob, segments, duration);
                if (newItem) {
                    useHistoryStore.getState().addItem(newItem);
                    useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                }
            }

            finalizedDurationSecondsRef.current = null;
        };

        mediaRecorderRef.current.start();
        return activateRecordSession(sessionId);
    }, [activateRecordSession, getRecordedDurationSeconds]);

    const stopFileRecording = useCallback(() => {
        if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
            mediaRecorderRef.current.stop();
        }
        setIsRecording(false);
        setIsPaused(false);
    }, [setIsRecording, setIsPaused]);

    const setNativeCapturePaused = useCallback(async (sessionId: string, paused: boolean) => {
        if (!usingNativeCaptureRef.current) {
            return;
        }

        const command = activeInputSourceRef.current === 'desktop'
            ? 'set_system_audio_capture_paused'
            : 'set_microphone_capture_paused';

        await invoke(command, { instanceId: 'record', paused });
        logger.info(
            `[useAudioRecorder] ${paused ? 'Paused' : 'Resumed'} native capture instance. session=${sessionId} command=${command}`
        );
    }, []);


    // Start Recording (Main Entry)
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

        const sessionId = openRecordSession();
        setIsInitializing(true);
        setIsTransitioning(true);

        try {
            let stream: MediaStream | undefined;
            activeInputSourceRef.current = inputSource;

            if (inputSource === 'desktop') {
                // Try Native Capture first
                let nativeSuccess = false;
                try {
                    logger.info(`[useAudioRecorder] Attempting native system audio capture. session=${sessionId}`);

                    // CRITICAL: Initialize recognizer BEFORE starting capture to avoid
                    // a race condition where audio feeds Sherpa before it is ready.
                    await initializeNativeSession(sessionId);

                    await invoke('start_system_audio_capture', {
                        deviceName: config.systemAudioDeviceId === 'default' ? null : config.systemAudioDeviceId,
                        instanceId: 'record'
                    });
                    usingNativeCaptureRef.current = true;

                    const peakListenerAttached = await attachNativePeakListener('system-audio', sessionId);
                    nativeSuccess = true;
                    logger.info(
                        `[useAudioRecorder] Record session capture attached. session=${sessionId} source=desktop transport=native peak_listener=${peakListenerAttached ? 'attached' : 'unavailable'}`
                    );

                } catch (e) {
                    logger.warn(`[useAudioRecorder] Native capture failed, fallback to Web API. session=${sessionId}`, e);
                    // If the session was partially initialized, roll it back so the
                    // Web API fallback path starts fresh.
                    await cleanupPartialStart(sessionId);
                    await softStopRecordSessionIfActive(sessionId, 'desktop_native_fallback');
                }

                if (!nativeSuccess) {
                    if (!navigator.mediaDevices || !navigator.mediaDevices.getDisplayMedia) {
                        throw new Error('Display media not supported');
                    }

                    stream = await navigator.mediaDevices.getDisplayMedia({
                        video: { width: 1, height: 1, frameRate: 1 },
                        audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }
                    });

                    const audioTracks = stream.getAudioTracks();
                    if (audioTracks.length === 0) throw new Error('No audio track found in display media');
                    stream.getVideoTracks().forEach(track => track.stop());
                    stream = new MediaStream([audioTracks[0]]);
                }
            } else {
                // Microphone
                let nativeSuccess = false;
                try {
                    logger.info(`[useAudioRecorder] Attempting native microphone capture. session=${sessionId}`);

                    // Set current microphone boost before starting
                    const currentBoost = config.microphoneBoost ?? 1.0;
                    await invoke('set_microphone_boost', { boost: currentBoost }).catch(err => {
                        logger.warn(`[useAudioRecorder] Failed to set initial microphone boost. session=${sessionId}:`, err);
                    });

                    // CRITICAL: Initialize recognizer BEFORE starting capture to avoid
                    // a race condition where audio feeds Sherpa before it is ready.
                    await initializeNativeSession(sessionId);

                    await invoke('start_microphone_capture', {
                        deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                        instanceId: 'record'
                    });
                    usingNativeCaptureRef.current = true;

                    const peakListenerAttached = await attachNativePeakListener('microphone-audio', sessionId);
                    nativeSuccess = true;
                    logger.info(
                        `[useAudioRecorder] Record session capture attached. session=${sessionId} source=microphone transport=native peak_listener=${peakListenerAttached ? 'attached' : 'unavailable'}`
                    );

                    if (config.muteDuringRecording) {
                        invoke('set_system_audio_mute', { mute: true })
                            .catch(err => logger.error('Failed to mute system audio:', err));
                    }

                } catch (e) {
                    logger.warn(`[useAudioRecorder] Native microphone capture failed, fallback to Web API. session=${sessionId}`, e);
                    // If the session was partially initialized, roll it back so the
                    // Web API fallback path starts fresh.
                    await cleanupPartialStart(sessionId);
                    await softStopRecordSessionIfActive(sessionId, 'microphone_native_fallback');
                }

                if (!nativeSuccess) {
                    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                        throw new Error('Media devices API not supported');
                    }

                    const constraints: MediaStreamConstraints = {
                        audio: config.microphoneId && config.microphoneId !== 'default'
                            ? {
                                deviceId: { exact: config.microphoneId },
                                autoGainControl: true,
                                noiseSuppression: true,
                                echoCancellation: true
                            }
                            : {
                                autoGainControl: true,
                                noiseSuppression: true,
                                echoCancellation: true
                            }
                    };

                    stream = await navigator.mediaDevices.getUserMedia(constraints);
                }
            }

            if (!usingNativeCaptureRef.current && stream) {
                activeStreamRef.current = stream;
                await initializeAudioSession(stream, sessionId);
                logger.info(`[useAudioRecorder] Record session capture attached. session=${sessionId} source=${inputSource} transport=web-audio`);

                if (config.muteDuringRecording && inputSource === 'microphone') {
                    invoke('set_system_audio_mute', { mute: true })
                        .catch(err => logger.error('Failed to mute system audio:', err));
                }
            }

            if (!startFileRecording(sessionId)) {
                throw new Error(`Failed to activate record session ${sessionId}`);
            }
            return true;

        } catch (error) {
            logger.error(`[useAudioRecorder] Record session start failed. session=${sessionId}:`, error);
            // Clean up any partially initialized state to ensure the next attempt starts fresh.
            await cleanupPartialStart(sessionId);
            await softStopRecordSessionIfActive(sessionId, 'start_failed');
            resetRecordSession(sessionId, 'start_failed', true);

            if (canMutateActiveRecordResources(sessionId)) {
                await showError({
                    code: inputSource === 'microphone' ? 'audio.microphone_failed' : 'audio.capture_failed',
                    messageKey: inputSource === 'microphone' ? 'errors.audio.microphone_failed' : 'errors.audio.capture_failed',
                    cause: error,
                });
            } else {
                logger.info(
                    `[useAudioRecorder] Suppressed stale start failure dialog. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'}`
                );
            }
            return false;
        } finally {
            if (recordSessionIdRef.current === sessionId && recordSessionPhaseRef.current === 'recording') {
                setIsTransitioning(false);
            }
            if (shouldFinalizeRecordStartAttempt(sessionId)) {
                setIsInitializing(false);
            } else {
                logger.info(
                    `[useAudioRecorder] Skipping stale start finalization. requested=${sessionId} active=${recordSessionIdRef.current ?? 'none'}`
                );
            }
        }
    }, [config, inputSource, showError, startFileRecording, initializeNativeSession, initializeAudioSession, openRecordSession, cleanupPartialStart, resetRecordSession, attachNativePeakListener, softStopRecordSessionIfActive, canMutateActiveRecordResources, shouldFinalizeRecordStartAttempt]);


    // Stop Recording
    const stopRecording = useCallback(async () => {
        const sessionId = recordSessionIdRef.current;
        if (!sessionId) {
            return;
        }

        setIsTransitioning(true);
        const previousPhase = recordSessionPhaseRef.current;
        recordSessionPhaseRef.current = 'stopping';
        logger.info(`[useAudioRecorder] Stopping recording session. session=${sessionId ?? 'none'} input=${activeInputSourceRef.current}`);
        const duration = finalizeRecordedDurationSeconds();

        // Stop Native Capture
        let savedWavPath: string | null = null;
        if (usingNativeCaptureRef.current) {
            if (nativeAudioUnlistenRef.current) {
                nativeAudioUnlistenRef.current();
                nativeAudioUnlistenRef.current = null;
            }
            try {
                if (activeInputSourceRef.current === 'desktop') {
                    savedWavPath = await invoke<string>('stop_system_audio_capture', { instanceId: 'record' });
                } else {
                    savedWavPath = await invoke<string>('stop_microphone_capture', { instanceId: 'record' });
                }
                logger.info('[useAudioRecorder] Saved raw audio to:', savedWavPath);
            } catch (e) {
                logger.error('Error:', e);
            }
        } else if (audioContextRef.current && audioContextRef.current.state === 'running') {
            try {
                await audioContextRef.current.suspend();
            } catch (e) {
                logger.error('Failed to suspend audio context:', e);
            }
        }

        // Soft stop service
        await transcriptionService.softStop();
        finalizeLastSegment();

        // Finalize Native Recording
        if (usingNativeCaptureRef.current) {
            if (savedWavPath) {
                const segments = useTranscriptStore.getState().segments;

                if (segments.length > 0) {
                    const url = convertFileSrc(savedWavPath);
                    useTranscriptStore.getState().setAudioUrl(url);

                    const newItem = await historyService.saveNativeRecording(savedWavPath, segments, duration);
                    if (newItem) {
                        useHistoryStore.getState().addItem(newItem);
                        useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                    }
                } else {
                    logger.info('[useAudioRecorder] Empty transcript, deleting unsaved WAV file:', savedWavPath);
                    try {
                        await remove(savedWavPath);
                    } catch (e) {
                        logger.error('[useAudioRecorder] Failed to delete empty WAV file:', e);
                    }
                }
            }
            usingNativeCaptureRef.current = false;
        }

        stopFileRecording();

        if (audioContextRef.current) {
            if (activeStreamRef.current) {
                activeStreamRef.current.getTracks().forEach(t => t.stop());
                activeStreamRef.current = null;
            }

            if (audioContextRef.current.state !== 'closed') {
                await audioContextRef.current.close();
            }
            audioContextRef.current = null;
        }

        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => logger.error('Failed to unmute system audio:', err));
        }
        logger.info(
            `[useAudioRecorder] Recording session stopped. session=${sessionId} previous_phase=${previousPhase} duration=${duration.toFixed(3)}`
        );
        resetRecordSession(sessionId, 'stop_completed');
    }, [config.muteDuringRecording, finalizeLastSegment, finalizeRecordedDurationSeconds, resetRecordSession, stopFileRecording]);

    const pauseRecording = useCallback(async () => {
        const sessionId = recordSessionIdRef.current;
        if (!sessionId || recordSessionPhaseRef.current !== 'recording') {
            return;
        }

        logger.info(`[useAudioRecorder] Pausing recording session. session=${sessionId}`);
        recordSessionPhaseRef.current = 'pausing';
        setIsTransitioning(true);
        peakLevelRef.current = 0;
        pauseRecordedDurationWindow();
        syncRecordingElapsedMs();

        try {
            if (usingNativeCaptureRef.current) {
                await setNativeCapturePaused(sessionId, true);
            } else {
                if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'recording') {
                    mediaRecorderRef.current.pause();
                }
                if (audioContextRef.current && audioContextRef.current.state === 'running') {
                    await audioContextRef.current.suspend();
                }
            }

            await transcriptionService.pauseStream();

            if (config.muteDuringRecording) {
                invoke('set_system_audio_mute', { mute: false })
                    .catch(err => logger.error('Failed to unmute system audio on pause:', err));
            }

            if (recordSessionIdRef.current !== sessionId) {
                return;
            }

            setIsPaused(true);
            recordSessionPhaseRef.current = 'paused';
            logger.info(`[useAudioRecorder] Recording session paused. session=${sessionId}`);
        } catch (error) {
            logger.error(`[useAudioRecorder] Failed to pause recording session. session=${sessionId}:`, error);

            if (recordSessionIdRef.current !== sessionId) {
                return;
            }

            try {
                await transcriptionService.resumeStream();
                setSegmentTimeOffsetSeconds(getNextSegmentTimeOffsetSeconds(), 'pause_error_recovery');
            } catch (resumeError) {
                logger.warn(`[useAudioRecorder] Failed to restore recognizer after pause error. session=${sessionId}:`, resumeError);
            }

            try {
                if (usingNativeCaptureRef.current) {
                    await setNativeCapturePaused(sessionId, false);
                } else {
                    if (audioContextRef.current && audioContextRef.current.state === 'suspended') {
                        await audioContextRef.current.resume();
                    }
                    if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'paused') {
                        mediaRecorderRef.current.resume();
                    }
                }
            } catch (restoreError) {
                logger.warn(`[useAudioRecorder] Failed to restore capture after pause error. session=${sessionId}:`, restoreError);
            }

            if (config.muteDuringRecording && activeInputSourceRef.current === 'microphone') {
                invoke('set_system_audio_mute', { mute: true })
                    .catch(err => logger.error('Failed to remute system audio after pause rollback:', err));
            }

            beginRecordedDurationWindow();
            syncRecordingElapsedMs();
            setIsPaused(false);
            recordSessionPhaseRef.current = 'recording';
        } finally {
            const currentPhase = recordSessionPhaseRef.current as RecordSessionPhase;
            if (recordSessionIdRef.current === sessionId && currentPhase !== 'stopping') {
                setIsTransitioning(false);
            }
        }
    }, [
        beginRecordedDurationWindow,
        config.muteDuringRecording,
        getNextSegmentTimeOffsetSeconds,
        pauseRecordedDurationWindow,
        setNativeCapturePaused,
        setSegmentTimeOffsetSeconds,
        syncRecordingElapsedMs
    ]);

    const resumeRecording = useCallback(async () => {
        const sessionId = recordSessionIdRef.current;
        if (!sessionId || recordSessionPhaseRef.current !== 'paused') {
            return;
        }

        logger.info(`[useAudioRecorder] Resuming recording session. session=${sessionId}`);
        recordSessionPhaseRef.current = 'resuming';
        setIsTransitioning(true);

        try {
            await transcriptionService.resumeStream();
            setSegmentTimeOffsetSeconds(getNextSegmentTimeOffsetSeconds(), 'resume');

            if (usingNativeCaptureRef.current) {
                await setNativeCapturePaused(sessionId, false);
            } else {
                if (audioContextRef.current && audioContextRef.current.state === 'suspended') {
                    await audioContextRef.current.resume();
                }
                if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'paused') {
                    mediaRecorderRef.current.resume();
                }
            }

            if (config.muteDuringRecording && activeInputSourceRef.current === 'microphone') {
                invoke('set_system_audio_mute', { mute: true })
                    .catch(err => logger.error('Failed to mute system audio on resume:', err));
            }

            if (recordSessionIdRef.current !== sessionId) {
                return;
            }

            beginRecordedDurationWindow();
            syncRecordingElapsedMs();
            setIsPaused(false);
            recordSessionPhaseRef.current = 'recording';
            logger.info(`[useAudioRecorder] Recording session resumed. session=${sessionId}`);
        } catch (error) {
            logger.error(`[useAudioRecorder] Failed to resume recording session. session=${sessionId}:`, error);
            if (recordSessionIdRef.current === sessionId) {
                setIsPaused(true);
                recordSessionPhaseRef.current = 'paused';
            }
        } finally {
            const currentPhase = recordSessionPhaseRef.current as RecordSessionPhase;
            if (recordSessionIdRef.current === sessionId && currentPhase !== 'stopping') {
                setIsTransitioning(false);
            }
        }
    }, [
        beginRecordedDurationWindow,
        config.muteDuringRecording,
        getNextSegmentTimeOffsetSeconds,
        setNativeCapturePaused,
        setSegmentTimeOffsetSeconds,
        syncRecordingElapsedMs
    ]);

    // Cleanup
    useEffect(() => {
        return () => {
            if (audioContextRef.current) {
                audioContextRef.current.close();
            }
            transcriptionService.terminate().catch(e => logger.error('Error stopping transcription service:', e));
        };
    }, []);

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
