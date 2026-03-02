import { useRef, useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useHistoryStore } from '../stores/historyStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { historyService } from '../services/historyService';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
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
    const audioContextRef = useRef<AudioContext | null>(null);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const activeStreamRef = useRef<MediaStream | null>(null);
    const nativeAudioUnlistenRef = useRef<UnlistenFn | null>(null);
    const usingNativeCaptureRef = useRef(false);
    const startTimeRef = useRef<number>(0);
    const mimeTypeRef = useRef<string>('');
    const peakLevelRef = useRef<number>(0);
    const activeInputSourceRef = useRef<'microphone' | 'desktop'>(inputSource);

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

        // Processor
        try {
            await audioContextRef.current.audioWorklet.addModule('/audio-processor.js');
        } catch (err) {
            console.error('Failed to load audio worklet module:', err);
            throw new Error('Audio worklet failed to load');
        }

        const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');
        processor.port.onmessage = (e) => {
            const samples = e.data as Int16Array;
            transcriptionService.sendAudioInt16(samples);
            if (!isPausedRef.current) {
                setPeakFromInt16(samples);
            }
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
            activeInputSourceRef.current = inputSource;

            if (inputSource === 'desktop') {
                // Try Native Capture first
                let nativeSuccess = false;
                try {
                    console.log('[useAudioRecorder] Attempting native system audio capture...');
                    await invoke('start_system_audio_capture', {
                        deviceName: config.systemAudioDeviceId === 'default' ? null : config.systemAudioDeviceId,
                        instanceId: 'record'
                    });

                    const unlisten = await listen<number>('system-audio', (event) => {
                        const peak = Math.abs(event.payload);
                        const sample = Math.min(32767, Math.round(peak));
                        // Do not send samples back to Rust, backend feeds itself directly.

                        if (!isPausedRef.current) {
                            peakLevelRef.current = sample / 32767;
                        }
                    });

                    nativeAudioUnlistenRef.current = unlisten;
                    usingNativeCaptureRef.current = true;
                    nativeSuccess = true;

                    await initializeNativeSession();

                } catch (e) {
                    console.warn('[useAudioRecorder] Native capture failed, fallback to Web API:', e);
                }

                if (!nativeSuccess) {
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
                }
            } else {
                // Microphone
                let nativeSuccess = false;
                try {
                    console.log('[useAudioRecorder] Attempting native microphone capture...');
                    await invoke('start_microphone_capture', {
                        deviceName: config.microphoneId === 'default' ? null : config.microphoneId,
                        instanceId: 'record'
                    });

                    const unlisten = await listen<number>('microphone-audio', (event) => {
                        const peak = Math.abs(event.payload);
                        const sample = Math.min(32767, Math.round(peak));
                        // Do not send samples back to Rust, backend feeds itself directly.

                        if (!isPausedRef.current) {
                            peakLevelRef.current = sample / 32767;
                        }
                    });

                    nativeAudioUnlistenRef.current = unlisten;
                    usingNativeCaptureRef.current = true;
                    nativeSuccess = true;

                    await initializeNativeSession();

                    if (config.muteDuringRecording) {
                        invoke('set_system_audio_mute', { mute: true })
                            .catch(err => console.error('Failed to mute system audio:', err));
                    }

                } catch (e) {
                    console.warn('[useAudioRecorder] Native microphone capture failed, fallback to Web API:', e);
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
                    savedWavPath = await invoke<string>('stop_microphone_capture');
                }
                console.log('[useAudioRecorder] Saved raw audio to:', savedWavPath);
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
            if (savedWavPath) {
                const url = convertFileSrc(savedWavPath);
                useTranscriptStore.getState().setAudioUrl(url);

                const segments = useTranscriptStore.getState().segments;
                const duration = (Date.now() - startTimeRef.current) / 1000;

                if (segments.length > 0) {
                    const newItem = await historyService.saveNativeRecording(savedWavPath, segments, duration);
                    if (newItem) {
                        useHistoryStore.getState().addItem(newItem);
                        useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                    }
                }
            }
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
        }

        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => console.error('Failed to unmute system audio:', err));
        }
    }, [config.muteDuringRecording, stopFileRecording]);

    const pauseRecording = useCallback(() => {
        setIsPaused(true);
        peakLevelRef.current = 0;
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
        peakLevelRef
    };
}
