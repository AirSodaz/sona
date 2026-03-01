import { useRef, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { historyService } from '../services/historyService';
import { encodeWAV } from '../utils/wavUtils';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface UseAudioRecorderProps {
    inputSource: 'microphone' | 'desktop';
    onSegment: (segment: any) => void;
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
    const { t } = useTranslation();
    const { alert } = useDialogStore();

    // Store Access
    const config = useTranscriptStore((state) => state.config);
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const setIsRecording = useTranscriptStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptStore((state) => state.setIsPaused);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);

    // Refs
    const analyserRef = useRef<AnalyserNode | null>(null);
    const audioContextRef = useRef<AudioContext | null>(null);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const activeStreamRef = useRef<MediaStream | null>(null);
    const audioChunksRef = useRef<Int16Array[]>([]);
    const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);
    const usingNativeCaptureRef = useRef(false);
    const startTimeRef = useRef<number>(0);
    const mimeTypeRef = useRef<string>('');
    const nextAudioTimeRef = useRef<number>(0);

    // State
    const [isInitializing, setIsInitializing] = useState(false);

    // Sync refs for callbacks
    const isRecordingRef = useRef(isRecording);
    const isPausedRef = useRef(isPaused);

    useEffect(() => {
        isRecordingRef.current = isRecording;
    }, [isRecording]);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    // Initialize Native Session
    const initializeNativeSession = async () => {
        await transcriptionService.start(
            onSegment,
            (error) => { console.error('Transcription error:', error); }
        );
    };

    // Initialize Audio Session (Web API)
    const initializeAudioSession = async (stream: MediaStream) => {
        if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        } else if (audioContextRef.current.state === 'suspended') {
            await audioContextRef.current.resume();
        }

        const source = audioContextRef.current.createMediaStreamSource(stream);

        // Visualizer
        analyserRef.current = audioContextRef.current.createAnalyser();
        analyserRef.current.fftSize = 256;
        source.connect(analyserRef.current);

        // Processor
        try {
            await audioContextRef.current.audioWorklet.addModule('/audio-processor.js');
        } catch (err) {
            console.error('Failed to load audio worklet module:', err);
            throw new Error('Audio worklet failed to load');
        }

        const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');
        processor.port.onmessage = (e) => {
            transcriptionService.sendAudioInt16(e.data);
        };

        source.connect(processor);
        processor.connect(audioContextRef.current.destination);

        await transcriptionService.start(
            onSegment,
            (error) => { console.error('Transcription error:', error); }
        );
    };

    // File Recording (MediaRecorder)
    const startFileRecording = useCallback(() => {
        if (usingNativeCaptureRef.current) {
            audioChunksRef.current = [];
            setIsRecording(true);
            setIsPaused(false);
            startTimeRef.current = Date.now();
            clearSegments();
            return;
        }

        const stream = activeStreamRef.current;
        if (!stream) {
            console.error("No active stream to record");
            return;
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
            const duration = (Date.now() - startTimeRef.current) / 1000;

            if (segments.length > 0) {
                const newItem = await historyService.saveRecording(blob, segments, duration);
                if (newItem) {
                    useHistoryStore.getState().addItem(newItem);
                    useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                }
            }
        };

        mediaRecorderRef.current.start();
        setIsRecording(true);
        setIsPaused(false);
        startTimeRef.current = Date.now();
        clearSegments();
    }, [setIsRecording, setIsPaused, clearSegments, usingNativeCaptureRef]);

    const stopFileRecording = useCallback(() => {
        if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
            mediaRecorderRef.current.stop();
        }
        setIsRecording(false);
        setIsPaused(false);
    }, [setIsRecording, setIsPaused]);


    // Start Recording (Main Entry)
    const startRecording = useCallback(async () => {
        if (!config.offlineModelPath) {
            await alert(t('batch.no_model_error'), { variant: 'error' });
            return false;
        }

        setIsInitializing(true);

        try {
            let stream: MediaStream | undefined;

            // Try Native Capture first for both 'desktop' and 'microphone'
            let nativeSuccess = false;
            try {
                if (inputSource === 'desktop') {
                    console.log('[useAudioRecorder] Attempting native system audio capture...');
                    await invoke('start_system_audio_capture', {
                        deviceName: config.systemAudioDeviceId === 'default' ? null : config.systemAudioDeviceId
                    });
                } else {
                    console.log('[useAudioRecorder] Attempting native microphone capture...');
                    await invoke('start_microphone_capture', {
                        deviceName: config.microphoneId === 'default' ? null : config.microphoneId
                    });
                }

                // Initialize AudioContext for visualization
                if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
                    audioContextRef.current = new AudioContext({ sampleRate: 16000 });
                } else if (audioContextRef.current.state === 'suspended') {
                    await audioContextRef.current.resume();
                }

                // Setup Analyser
                if (!analyserRef.current && audioContextRef.current) {
                    analyserRef.current = audioContextRef.current.createAnalyser();
                    analyserRef.current.fftSize = 256;
                    const gainNode = audioContextRef.current.createGain();
                    gainNode.gain.value = 0;
                    analyserRef.current.connect(gainNode);
                    gainNode.connect(audioContextRef.current.destination);
                }

                if (audioContextRef.current) {
                    nextAudioTimeRef.current = audioContextRef.current.currentTime;
                }

                const eventName = inputSource === 'desktop' ? 'system-audio' : 'microphone-audio';

                const unlisten = await listen<number[]>(eventName, (event) => {
                    const samples = new Int16Array(event.payload);
                    transcriptionService.sendAudioInt16(samples);

                    if (isRecordingRef.current && !isPausedRef.current) {
                        audioChunksRef.current.push(samples);
                    }

                    // Visualization
                    if (audioContextRef.current && analyserRef.current && !isPausedRef.current) {
                        const float32Data = new Float32Array(samples.length);
                        for (let i = 0; i < samples.length; i++) {
                            const float = samples[i] < 0 ? samples[i] / 0x8000 : samples[i] / 0x7FFF;
                            float32Data[i] = float;
                        }

                        const buffer = audioContextRef.current.createBuffer(1, samples.length, 16000);
                        buffer.copyToChannel(float32Data, 0);

                        const source = audioContextRef.current.createBufferSource();
                        source.buffer = buffer;
                        source.connect(analyserRef.current);

                        let startTime = nextAudioTimeRef.current;
                        if (startTime < audioContextRef.current.currentTime) {
                            startTime = audioContextRef.current.currentTime;
                        }
                        source.start(startTime);
                        nextAudioTimeRef.current = startTime + buffer.duration;
                    }
                });

                systemAudioUnlistenRef.current = unlisten;
                usingNativeCaptureRef.current = true;
                nativeSuccess = true;

                audioChunksRef.current = [];
                await initializeNativeSession();

            } catch (e) {
                console.warn(`[useAudioRecorder] Native capture failed for ${inputSource}, fallback to Web API:`, e);
            }

            if (!nativeSuccess) {
                if (inputSource === 'desktop') {
                    if (!navigator.mediaDevices || !navigator.mediaDevices.getDisplayMedia) {
                        throw new Error(t('live.mic_error') + ': Display media not supported');
                    }

                    stream = await navigator.mediaDevices.getDisplayMedia({
                        video: { width: 1, height: 1, frameRate: 1 },
                        audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }
                    });

                    const audioTracks = stream.getAudioTracks();
                    if (audioTracks.length === 0) throw new Error('No audio track found in display media');
                    stream.getVideoTracks().forEach(track => track.stop());
                    stream = new MediaStream([audioTracks[0]]);
                } else {
                    // Microphone Fallback
                    if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                        throw new Error('Media devices API not supported');
                    }

                    const constraints: MediaStreamConstraints = {
                        audio: config.microphoneId && config.microphoneId !== 'default'
                            ? { deviceId: { exact: config.microphoneId } }
                            : true
                    };

                    stream = await navigator.mediaDevices.getUserMedia(constraints);
                }
            }

            if (!usingNativeCaptureRef.current && stream) {
                activeStreamRef.current = stream;
                await initializeAudioSession(stream);

                if (config.muteDuringRecording && inputSource === 'microphone') {
                    invoke('set_system_audio_mute', { mute: true })
                        .catch(err => console.error('Failed to mute system audio:', err));
                }
            }

            startFileRecording();
            return true;

        } catch (error) {
            console.error('Failed to start capture:', error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            await alert(`${t('live.mic_error')} (${errorMessage})`, { variant: 'error' });
            return false;
        } finally {
            setIsInitializing(false);
        }
    }, [config, inputSource, t, alert, startFileRecording, initializeNativeSession, initializeAudioSession]); // Added deps


    // Stop Recording
    const stopRecording = useCallback(async () => {
        console.log('[useAudioRecorder] Stopping recording session...');

        // Stop Native Capture
        if (usingNativeCaptureRef.current) {
            if (systemAudioUnlistenRef.current) {
                systemAudioUnlistenRef.current();
                systemAudioUnlistenRef.current = null;
            }
            try {
                if (inputSource === 'desktop') {
                    await invoke('stop_system_audio_capture');
                } else {
                    await invoke('stop_microphone_capture');
                }
            } catch (e) { console.error(e); }
        } else if (audioContextRef.current && audioContextRef.current.state === 'running') {
            try {
                await audioContextRef.current.suspend();
            } catch (e) { console.error('Failed to suspend audio context:', e); }
        }

        // Soft stop service
        await transcriptionService.softStop();

        // Finalize Native Recording
        if (usingNativeCaptureRef.current) {
            const chunks = audioChunksRef.current;
            if (chunks.length > 0) {
                const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
                const fullBuffer = new Int16Array(totalLength);
                let offset = 0;
                for (const chunk of chunks) {
                    fullBuffer.set(chunk, offset);
                    offset += chunk.length;
                }

                const blob = encodeWAV(fullBuffer, 16000, 1, 16);
                const url = URL.createObjectURL(blob);
                useTranscriptStore.getState().setAudioUrl(url);

                const segments = useTranscriptStore.getState().segments;
                const duration = (Date.now() - startTimeRef.current) / 1000;

                if (segments.length > 0) {
                    const newItem = await historyService.saveRecording(blob, segments, duration);
                    if (newItem) {
                        useHistoryStore.getState().addItem(newItem);
                        useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                    }
                }
            }
            audioChunksRef.current = [];
            usingNativeCaptureRef.current = false;
        }

        stopFileRecording();

        if (audioContextRef.current) {
            activeStreamRef.current?.getTracks().forEach(t => t.stop());
            activeStreamRef.current = null;

            if (audioContextRef.current.state !== 'closed') {
                await audioContextRef.current.close();
            }
            audioContextRef.current = null;
            analyserRef.current = null;
        }

        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => console.error('Failed to unmute system audio:', err));
        }
    }, [config.muteDuringRecording, stopFileRecording]);

    const pauseRecording = useCallback(() => {
        setIsPaused(true);
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'recording') {
            mediaRecorderRef.current.pause();
        }
        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => console.error('Failed to unmute system audio on pause:', err));
        }
    }, [setIsPaused, config.muteDuringRecording]);

    const resumeRecording = useCallback(() => {
        setIsPaused(false);
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'paused') {
            mediaRecorderRef.current.resume();
        }
        if (config.muteDuringRecording && inputSource === 'microphone') {
            invoke('set_system_audio_mute', { mute: true })
                .catch(err => console.error('Failed to mute system audio on resume:', err));
        }
        if (audioContextRef.current && audioContextRef.current.state === 'suspended') {
            audioContextRef.current.resume();
        }
    }, [setIsPaused, config.muteDuringRecording, inputSource]);

    // Cleanup
    useEffect(() => {
        return () => {
            if (audioContextRef.current) {
                audioContextRef.current.close();
            }
            transcriptionService.terminate().catch(e => console.error('Error stopping transcription service:', e));
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
        analyserRef
    };
}
