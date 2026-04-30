import { listen } from '@tauri-apps/api/event';
import { transcriptionService } from '../../services/transcriptionService';
import {
    setMicrophoneBoost as setMicrophoneBoostTauri,
    setMicrophoneCapturePaused,
    setSystemAudioCapturePaused,
    setSystemAudioMute as setSystemAudioMuteTauri,
    startMicrophoneCapture,
    startSystemAudioCapture,
    stopMicrophoneCapture,
    stopSystemAudioCapture,
} from '../../services/tauri/audio';
import { TauriEvent } from '../../services/tauri/events';
import type { TranscriptUpdate } from '../../types/transcript';
import { shouldFeedWebAudioForPhase } from './timing';
import type {
    AudioRecorderCaptureRefs,
    AudioRecorderLogger,
    InputSource,
} from './types';

interface CaptureErrorDialogInput {
    code: string;
    messageKey: string;
    cause: unknown;
}

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

interface CreateAudioRecorderCaptureArgs {
    refs: AudioRecorderCaptureRefs;
    logger: AudioRecorderLogger;
    onSegment: (update: TranscriptUpdate) => void;
    showError: (input: CaptureErrorDialogInput) => Promise<void>;
    activateRecordSession: (sessionId: string) => boolean;
    canMutateActiveRecordResources: (sessionId: string) => boolean;
    rollbackRecognizer: (sessionId: string, reason: string) => Promise<void>;
    setPeakFromInt16: (samples: Int16Array) => void;
    onWebRecordingStop: (blob: Blob, mimeType: string) => Promise<void>;
    setIsRecording: (value: boolean) => void;
    setIsPaused: (value: boolean) => void;
}

export function createAudioRecorderCapture({
    refs,
    logger,
    onSegment,
    showError,
    activateRecordSession,
    canMutateActiveRecordResources,
    rollbackRecognizer,
    setPeakFromInt16,
    onWebRecordingStop,
    setIsRecording,
    setIsPaused,
}: CreateAudioRecorderCaptureArgs) {
    type NativePeakEventName =
        | typeof TauriEvent.audio.systemPeak
        | typeof TauriEvent.audio.microphonePeak;

    function isDesktopCaptureActive(): boolean {
        return refs.activeInputSourceRef.current === 'desktop';
    }

    async function stopActiveNativeCapture(instanceId: string): Promise<string> {
        return isDesktopCaptureActive()
            ? stopSystemAudioCapture(instanceId)
            : stopMicrophoneCapture(instanceId);
    }

    async function setActiveNativeCapturePaused(instanceId: string, paused: boolean): Promise<void> {
        await (
            isDesktopCaptureActive()
                ? setSystemAudioCapturePaused({ instanceId, paused })
                : setMicrophoneCapturePaused({ instanceId, paused })
        );
    }

    async function setSystemAudioMute(mute: boolean, errorMessage: string): Promise<void> {
        try {
            await setSystemAudioMuteTauri(mute);
        } catch (error) {
            logger.error(errorMessage, error);
        }
    }

    async function cleanupPartialStart(sessionId: string): Promise<void> {
        if (!canMutateActiveRecordResources(sessionId)) {
            logger.info(
                `[useAudioRecorder] Skipping shared resource rollback for stale session. requested=${sessionId} active=stale`
            );
            return;
        }

        // Roll back whichever capture resources were acquired before the start
        // path failed so fallback/retry attempts begin from a clean baseline.
        if (refs.usingNativeCaptureRef.current) {
            if (refs.nativeAudioUnlistenRef.current) {
                refs.nativeAudioUnlistenRef.current();
                refs.nativeAudioUnlistenRef.current = null;
            }
            try {
                await stopActiveNativeCapture('record');
                logger.info(
                    `[useAudioRecorder] Rolled back native capture. session=${sessionId} source=${isDesktopCaptureActive() ? 'desktop' : 'microphone'}`
                );
            } catch (error) {
                logger.warn(
                    `[useAudioRecorder] Failed to roll back native capture. session=${sessionId} source=${isDesktopCaptureActive() ? 'desktop' : 'microphone'}`,
                    error,
                );
            }
            refs.usingNativeCaptureRef.current = false;
        }

        if (refs.activeStreamRef.current) {
            refs.activeStreamRef.current.getTracks().forEach((track) => track.stop());
            refs.activeStreamRef.current = null;
        }

        if (refs.audioContextRef.current) {
            try {
                if (refs.audioContextRef.current.state !== 'closed') {
                    await refs.audioContextRef.current.close();
                }
            } catch (error) {
                logger.warn(`[useAudioRecorder] Failed to close audio context during rollback. session=${sessionId}`, error);
            }
            refs.audioContextRef.current = null;
        }

        refs.mediaRecorderRef.current = null;
    }

    async function attachNativePeakListener(
        eventName: NativePeakEventName,
        sessionId: string,
    ): Promise<boolean> {
        try {
            const unlisten = await listen<number>(eventName, (event) => {
                const peak = Math.abs(event.payload);
                const sample = Math.min(32767, Math.round(peak));

                // This event only drives the live waveform meter. Native capture and
                // backend transcription keep running even if the UI listener is unavailable.
                if (!refs.isPausedRef.current) {
                    refs.peakLevelRef.current = sample / 32767;
                }
            });

            refs.nativeAudioUnlistenRef.current = unlisten;
            return true;
        } catch (error) {
            refs.nativeAudioUnlistenRef.current = null;
            logger.warn(
                `[useAudioRecorder] Failed to attach native peak listener. session=${sessionId} event=${eventName}. Continuing without live meter.`,
                error,
            );
            return false;
        }
    }

    async function initializeNativeSession(sessionId: string): Promise<void> {
        logger.info(`[useAudioRecorder] Initializing transcription service (native). session=${sessionId}`);
        try {
            await transcriptionService.start(
                onSegment,
                (error) => {
                    logger.error(`[useAudioRecorder] Transcription error callback. session=${sessionId}:`, error);
                    void showError({
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
        } catch (error) {
            logger.error(`[useAudioRecorder] Failed to start transcription service. session=${sessionId}:`, error);
            throw error;
        }
    }

    async function initializeAudioSession(stream: MediaStream, sessionId: string): Promise<void> {
        if (!refs.audioContextRef.current || refs.audioContextRef.current.state === 'closed') {
            refs.audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        } else if (refs.audioContextRef.current.state === 'suspended') {
            await refs.audioContextRef.current.resume();
        }

        // CRITICAL: Initialize transcription service BEFORE connecting the audio graph.
        // This ensures isRunning=true before any audio samples arrive via onmessage,
        // preventing initial audio data from being silently dropped.
        logger.info(`[useAudioRecorder] Initializing transcription service (web audio). session=${sessionId}`);
        await transcriptionService.start(
            onSegment,
            (error) => { logger.error(`[useAudioRecorder] Transcription error. session=${sessionId}:`, error); },
            {
                callbackOwner: 'live-record',
                callbackSessionId: sessionId
            }
        );
        logger.info(`[useAudioRecorder] Record session recognizer ready. session=${sessionId} transport=web-audio`);

        const source = refs.audioContextRef.current.createMediaStreamSource(stream);

        try {
            await refs.audioContextRef.current.audioWorklet.addModule('/audio-processor.js');
        } catch (error) {
            logger.error('Failed to load audio worklet module:', error);
            throw Object.assign(new Error('Audio worklet failed to load'), { cause: error });
        }

        const processor = new AudioWorkletNode(refs.audioContextRef.current, 'audio-processor');
        processor.port.onmessage = (event) => {
            const samples = event.data as Int16Array;
            // Only forward samples while the session phase says the recognizer
            // should still be accumulating audio for this run.
            if (shouldFeedWebAudioForPhase(refs.recordSessionPhaseRef.current)) {
                void transcriptionService.sendAudioInt16(samples);
            }
            if (!refs.isPausedRef.current) {
                setPeakFromInt16(samples);
            }
        };

        source.connect(processor);
        processor.connect(refs.audioContextRef.current.destination);
    }

    async function tryStartNativeDesktopCapture(
        sessionId: string,
        deviceName: string | null,
        outputPath: string,
    ): Promise<boolean> {
        try {
            logger.info(`[useAudioRecorder] Attempting native system audio capture. session=${sessionId}`);

            // CRITICAL: Initialize recognizer BEFORE starting capture to avoid
            // a race condition where audio feeds Sherpa before it is ready.
            await initializeNativeSession(sessionId);

            await startSystemAudioCapture({
                deviceName,
                instanceId: 'record',
                outputPath,
            });
            refs.usingNativeCaptureRef.current = true;

            const peakListenerAttached = await attachNativePeakListener(TauriEvent.audio.systemPeak, sessionId);
            logger.info(
                `[useAudioRecorder] Record session capture attached. session=${sessionId} source=desktop transport=native peak_listener=${peakListenerAttached ? 'attached' : 'unavailable'}`
            );
            return true;
        } catch (error) {
            logger.warn(`[useAudioRecorder] Native capture failed, fallback to Web API. session=${sessionId}`, error);
            await cleanupPartialStart(sessionId);
            await rollbackRecognizer(sessionId, 'desktop_native_fallback');
            return false;
        }
    }

    async function tryStartNativeMicrophoneCapture(
        sessionId: string,
        options: {
            deviceName: string | null;
            boost: number;
            muteDuringRecording: boolean;
            outputPath: string;
        },
    ): Promise<boolean> {
        try {
            logger.info(`[useAudioRecorder] Attempting native microphone capture. session=${sessionId}`);

            await setMicrophoneBoostTauri(options.boost).catch((error) => {
                logger.warn(`[useAudioRecorder] Failed to set initial microphone boost. session=${sessionId}:`, error);
            });

            // CRITICAL: Initialize recognizer BEFORE starting capture to avoid
            // a race condition where audio feeds Sherpa before it is ready.
            await initializeNativeSession(sessionId);

            await startMicrophoneCapture({
                deviceName: options.deviceName,
                instanceId: 'record',
                outputPath: options.outputPath,
            });
            refs.usingNativeCaptureRef.current = true;

            const peakListenerAttached = await attachNativePeakListener(TauriEvent.audio.microphonePeak, sessionId);
            logger.info(
                `[useAudioRecorder] Record session capture attached. session=${sessionId} source=microphone transport=native peak_listener=${peakListenerAttached ? 'attached' : 'unavailable'}`
            );

            if (options.muteDuringRecording) {
                void setSystemAudioMute(true, 'Failed to mute system audio:');
            }

            return true;
        } catch (error) {
            logger.warn(`[useAudioRecorder] Native microphone capture failed, fallback to Web API. session=${sessionId}`, error);
            await cleanupPartialStart(sessionId);
            await rollbackRecognizer(sessionId, 'microphone_native_fallback');
            return false;
        }
    }

    async function requestWebFallbackStream(inputSource: InputSource, microphoneId: string | undefined): Promise<MediaStream> {
        if (inputSource === 'desktop') {
            if (!navigator.mediaDevices || !navigator.mediaDevices.getDisplayMedia) {
                throw new Error('Display media not supported');
            }

            let stream = await navigator.mediaDevices.getDisplayMedia({
                video: { width: 1, height: 1, frameRate: 1 },
                audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }
            });

            const audioTracks = stream.getAudioTracks();
            if (audioTracks.length === 0) {
                throw new Error('No audio track found in display media');
            }
            stream.getVideoTracks().forEach((track) => track.stop());
            stream = new MediaStream([audioTracks[0]]);
            return stream;
        }

        if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
            throw new Error('Media devices API not supported');
        }

        const constraints: MediaStreamConstraints = {
            audio: microphoneId && microphoneId !== 'default'
                ? {
                    deviceId: { exact: microphoneId },
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

        return navigator.mediaDevices.getUserMedia(constraints);
    }

    async function attachWebStream(
        sessionId: string,
        stream: MediaStream,
        inputSource: InputSource,
        muteDuringRecording: boolean,
    ): Promise<void> {
        refs.activeStreamRef.current = stream;
        await initializeAudioSession(stream, sessionId);
        logger.info(`[useAudioRecorder] Record session capture attached. session=${sessionId} source=${inputSource} transport=web-audio`);

        if (muteDuringRecording && inputSource === 'microphone') {
            void setSystemAudioMute(true, 'Failed to mute system audio:');
        }
    }

    function startFileRecording(sessionId: string): boolean {
        if (refs.usingNativeCaptureRef.current) {
            // Native capture writes its own WAV via Rust, so the browser-side
            // recorder only exists for the Web API fallback path.
            return activateRecordSession(sessionId);
        }

        const stream = refs.activeStreamRef.current;
        if (!stream) {
            logger.error('No active stream to record');
            return false;
        }

        const mimeType = getSupportedMimeType();
        refs.mimeTypeRef.current = mimeType;
        const options = mimeType ? { mimeType } : undefined;

        const recorder = new MediaRecorder(stream, options);
        refs.mediaRecorderRef.current = recorder;

        const chunks: Blob[] = [];

        recorder.ondataavailable = (event) => {
            chunks.push(event.data);
        };

        recorder.onstop = () => {
            const type = refs.mimeTypeRef.current || recorder.mimeType || 'audio/webm';
            const blob = new Blob(chunks, { type });
            void onWebRecordingStop(blob, type).catch((error) => {
                logger.error('[useAudioRecorder] Failed to persist MediaRecorder fallback audio:', error);
            });
        };

        recorder.start();
        return activateRecordSession(sessionId);
    }

    function stopFileRecording(): void {
        if (refs.mediaRecorderRef.current && refs.mediaRecorderRef.current.state !== 'inactive') {
            refs.mediaRecorderRef.current.stop();
        }
        setIsRecording(false);
        setIsPaused(false);
    }

    async function stopCaptureForSession(sessionId: string): Promise<string | null> {
        let savedWavPath: string | null = null;

        if (refs.usingNativeCaptureRef.current) {
            if (refs.nativeAudioUnlistenRef.current) {
                refs.nativeAudioUnlistenRef.current();
                refs.nativeAudioUnlistenRef.current = null;
            }
            try {
                savedWavPath = await stopActiveNativeCapture('record');
                logger.info('[useAudioRecorder] Saved raw audio to:', savedWavPath);
            } catch (error) {
                logger.error(`[useAudioRecorder] Failed to stop native capture. session=${sessionId}:`, error);
            }
            return savedWavPath;
        }

        if (refs.audioContextRef.current && refs.audioContextRef.current.state === 'running') {
            try {
                await refs.audioContextRef.current.suspend();
            } catch (error) {
                logger.error('Failed to suspend audio context:', error);
            }
        }

        return null;
    }

    async function pauseCapture(sessionId: string): Promise<void> {
        if (refs.usingNativeCaptureRef.current) {
            await setActiveNativeCapturePaused('record', true);
            logger.info(
                `[useAudioRecorder] Paused native capture instance. session=${sessionId} source=${isDesktopCaptureActive() ? 'desktop' : 'microphone'}`
            );
            return;
        }

        if (refs.mediaRecorderRef.current && refs.mediaRecorderRef.current.state === 'recording') {
            refs.mediaRecorderRef.current.pause();
        }
        if (refs.audioContextRef.current && refs.audioContextRef.current.state === 'running') {
            await refs.audioContextRef.current.suspend();
        }
    }

    async function resumeCapture(sessionId: string): Promise<void> {
        if (refs.usingNativeCaptureRef.current) {
            await setActiveNativeCapturePaused('record', false);
            logger.info(
                `[useAudioRecorder] Resumed native capture instance. session=${sessionId} source=${isDesktopCaptureActive() ? 'desktop' : 'microphone'}`
            );
            return;
        }

        if (refs.audioContextRef.current && refs.audioContextRef.current.state === 'suspended') {
            await refs.audioContextRef.current.resume();
        }
        if (refs.mediaRecorderRef.current && refs.mediaRecorderRef.current.state === 'paused') {
            refs.mediaRecorderRef.current.resume();
        }
    }

    async function teardownWebCaptureResources(): Promise<void> {
        if (refs.activeStreamRef.current) {
            refs.activeStreamRef.current.getTracks().forEach((track) => track.stop());
            refs.activeStreamRef.current = null;
        }

        if (refs.audioContextRef.current) {
            if (refs.audioContextRef.current.state !== 'closed') {
                await refs.audioContextRef.current.close();
            }
            refs.audioContextRef.current = null;
        }
    }

    return {
        cleanupPartialStart,
        tryStartNativeDesktopCapture,
        tryStartNativeMicrophoneCapture,
        requestWebFallbackStream,
        attachWebStream,
        startFileRecording,
        stopFileRecording,
        stopCaptureForSession,
        pauseCapture,
        resumeCapture,
        teardownWebCaptureResources,
        setSystemAudioMute,
    };
}
