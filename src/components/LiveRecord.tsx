import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { modelService } from '../services/modelService';
import { Pause, Play, Square, Mic, Monitor } from 'lucide-react';
import { historyService } from '../services/historyService';
import { useHistoryStore } from '../stores/historyStore';
import { splitByPunctuation } from '../utils/segmentUtils';
import { RecordingTimer } from './RecordingTimer';
import { Dropdown } from './Dropdown';
import { TranscriptionOptions } from './TranscriptionOptions';
import { Switch } from './Switch';
import { captionWindowService } from '../services/captionWindowService';
import { useCaptionSession } from '../hooks/useCaptionSession';
import { encodeWAV } from '../utils/wavUtils';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/** Props for the LiveRecord component. */
interface LiveRecordProps {
    className?: string;
}

/**
 * Determines the supported audio MIME type for the current browser.
 *
 * Checks a list of common types and returns the first supported one.
 *
 * @return The supported MIME type string, or empty string if none found.
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

function getSourceIcon(source: 'microphone' | 'desktop' | 'file'): React.ReactElement {
    switch (source) {
        case 'microphone': return <Mic size={18} aria-hidden="true" />;
        case 'desktop': return <Monitor size={18} aria-hidden="true" />;
        default: return <Mic size={18} aria-hidden="true" />;
    }
}

/**
 * Component for handling real-time audio recording and visualization.
 *
 * Supports recording from microphone or system audio (desktop).
 * Includes a visualizer and timer.
 *
 * @param props Component props.
 * @return The rendered LiveRecord component.
 */
export function LiveRecord({ className = '' }: LiveRecordProps): React.ReactElement {
    const { alert } = useDialogStore();
    const { t } = useTranslation();

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const animationRef = useRef<number>(0);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const audioContextRef = useRef<AudioContext | null>(null);
    const activeStreamRef = useRef<MediaStream | null>(null); // Managed stream for recording and visualization
    const nextStartTimeRef = useRef<number>(0);

    // Native capture refs
    const usingNativeCaptureRef = useRef(false);
    const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);
    const audioChunksRef = useRef<Int16Array[]>([]);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const setIsRecording = useTranscriptStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptStore((state) => state.setIsPaused);

    const isRecordingRef = useRef(false);
    const isPausedRef = useRef(false);

    // Sync refs with store state
    useEffect(() => {
        isRecordingRef.current = isRecording;
    }, [isRecording]);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    const mimeTypeRef = useRef<string>('');
    const [isRecordingInitializing, setIsRecordingInitializing] = useState(false);
    const [inputSource, setInputSource] = useState<'microphone' | 'desktop'>('microphone');

    // Caption Mode
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);

    // Initialize dedicated caption session hook
    useCaptionSession(config, isCaptionMode);

    // Use config directly
    const enableTimeline = config.enableTimeline ?? true;
    const language = config.language;
    const lockWindow = config.lockWindow ?? false;
    const alwaysOnTop = config.alwaysOnTop ?? true;
    const enableTimelineRef = useRef(true);

    const setEnableTimeline = useCallback((val: boolean) => setConfig({ enableTimeline: val }), [setConfig]);
    const setLanguage = useCallback((val: string) => setConfig({ language: val }), [setConfig]);

    // Sync ref
    useEffect(() => {
        enableTimelineRef.current = enableTimeline;
    }, [enableTimeline]);

    // Sync window settings (Optional: could move to hook, but UI is here)
    useEffect(() => {
        captionWindowService.setClickThrough(lockWindow).catch(console.error);
        captionWindowService.setAlwaysOnTop(alwaysOnTop).catch(console.error);
    }, [lockWindow, alwaysOnTop]);

    const audioRef = useRef<HTMLAudioElement | null>(null);
    const startTimeRef = useRef<number>(0);

    const upsertSegmentAndSetActive = useTranscriptStore((state) => state.upsertSegmentAndSetActive);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);

    // Draw visualizer
    const drawVisualizer = useCallback(() => {
        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
        }

        const canvas = canvasRef.current;
        const analyser = analyserRef.current;
        if (!canvas || !analyser) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const bufferLength = analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        // Cache for gradients: 256 possible byte values
        const gradients = new Array<CanvasGradient | undefined>(256);
        let cachedHeight = canvas.height;

        const draw = () => {
            // Optimization: Stop the loop if paused
            if (isPausedRef.current) {
                return;
            }

            animationRef.current = window.requestAnimationFrame(draw);

            // Invalidate cache if height changes (e.g. resize)
            if (canvas.height !== cachedHeight) {
                gradients.fill(undefined);
                cachedHeight = canvas.height;
            }

            analyser.getByteFrequencyData(dataArray);

            ctx.clearRect(0, 0, canvas.width, canvas.height); // Use CSS background

            const barWidth = (canvas.width / bufferLength) * 2.5;
            let x = 0;

            for (let i = 0; i < bufferLength; i++) {
                const value = dataArray[i];
                const barHeight = (value / 255) * canvas.height * 0.8;

                if (!gradients[value]) {
                    const gradient = ctx.createLinearGradient(0, canvas.height, 0, canvas.height - barHeight);
                    gradient.addColorStop(0, '#37352f'); // Notion Black
                    gradient.addColorStop(1, '#787774'); // Notion Gray
                    gradients[value] = gradient;
                }

                ctx.fillStyle = gradients[value]!;
                ctx.fillRect(x, canvas.height - barHeight, barWidth - 1, barHeight);

                x += barWidth;
            }
        };

        draw();
    }, []);

    // Helper to configure service (shared logic)
    const configureService = async () => {
        const config = useTranscriptStore.getState().config;
        transcriptionService.setModelPath(config.offlineModelPath);
        transcriptionService.setLanguage(language);
        transcriptionService.setEnableITN(config.enableITN ?? false);

        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];
        if (enabledITNModels.size > 0) {
            try {
                const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                transcriptionService.setITNModelPaths(paths);
            } catch (e) {
                console.warn('[LiveRecord] Failed to setup ITN paths:', e);
            }
        }

        transcriptionService.setPunctuationModelPath(config.punctuationModelPath || '');
        transcriptionService.setCtcModelPath(config.ctcModelPath || '');
        transcriptionService.setVadModelPath(config.vadModelPath || '');
        transcriptionService.setVadBufferSize(config.vadBufferSize || 5);
    };

    // Callback for service segments
    const onSegment = (segment: any) => {
        if (isRecordingRef.current) {
            if (enableTimelineRef.current && segment.isFinal) {
                const parts = splitByPunctuation([segment]);
                if (parts.length > 0) {
                    useTranscriptStore.getState().deleteSegment(segment.id);
                    parts.forEach(part => useTranscriptStore.getState().upsertSegment(part));
                    useTranscriptStore.getState().setActiveSegmentId(parts[parts.length - 1].id);
                } else {
                    upsertSegmentAndSetActive(segment);
                }
            } else {
                upsertSegmentAndSetActive(segment);
            }
        }
    };

    // Initialize Native Session (No AudioContext)
    async function initializeNativeSession(): Promise<void> {
        await configureService();
        await transcriptionService.start(
            onSegment,
            (error) => { console.error('Transcription error:', error); }
        );
    }

    // Ensure AudioContext and Analyser are ready for visualization
    const ensureAudioContext = useCallback(async () => {
        if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        } else if (audioContextRef.current.state === 'suspended') {
            await audioContextRef.current.resume();
        }

        if (!analyserRef.current && audioContextRef.current) {
            analyserRef.current = audioContextRef.current.createAnalyser();
            analyserRef.current.fftSize = 256;
        }
    }, []);

    // Start Recording Capture (Audio Context + Visualizer + Main Transcription)
    const startRecordingSession = useCallback(async () => {
        const config = useTranscriptStore.getState().config;
        if (!config.offlineModelPath) {
            await alert(t('batch.no_model_error'), { variant: 'error' });
            return false;
        }

        setIsRecordingInitializing(true);

        try {
            let stream: MediaStream | undefined;

            if (inputSource === 'desktop') {
                // Try Native Capture first
                let nativeSuccess = false;
                try {
                    console.log('[LiveRecord] Attempting native system audio capture...');
                    await invoke('start_system_audio_capture');

                    // Initialize visualization context for native capture
                    await ensureAudioContext();
                    nextStartTimeRef.current = audioContextRef.current?.currentTime || 0;
                    drawVisualizer();

                    const unlisten = await listen<number[]>('system-audio', (event) => {
                        const samples = new Int16Array(event.payload);
                        transcriptionService.sendAudioInt16(samples);

                        // Feed visualizer
                        if (audioContextRef.current && analyserRef.current && samples.length > 0) {
                            const ctx = audioContextRef.current;
                            // Convert Int16 to Float32
                            const float32 = new Float32Array(samples.length);
                            for (let i = 0; i < samples.length; i++) {
                                float32[i] = samples[i] / 32768.0;
                            }

                            const buffer = ctx.createBuffer(1, samples.length, 16000);
                            buffer.copyToChannel(float32, 0);

                            const source = ctx.createBufferSource();
                            source.buffer = buffer;
                            source.connect(analyserRef.current);

                            // Schedule playback
                            const now = ctx.currentTime;
                            // Reset if we fell behind too much (e.g. paused tab)
                            if (nextStartTimeRef.current < now) {
                                nextStartTimeRef.current = now;
                            }

                            source.start(nextStartTimeRef.current);
                            nextStartTimeRef.current += buffer.duration;
                        }

                        // If recording and not paused, accumulate samples
                        if (isRecordingRef.current && !isPausedRef.current) {
                             audioChunksRef.current.push(samples);
                        }
                    });

                    systemAudioUnlistenRef.current = unlisten;
                    usingNativeCaptureRef.current = true;
                    nativeSuccess = true;
                    console.log('[LiveRecord] Native capture started.');

                    // Clear chunks when starting new session
                    audioChunksRef.current = [];

                    // Initialize service without AudioContext
                    await initializeNativeSession();

                } catch (e) {
                    console.warn('[LiveRecord] Native capture failed, fallback to Web API:', e);
                }

                if (!nativeSuccess) {
                    // Fallback to Web API (getDisplayMedia)
                    if (!navigator.mediaDevices || !navigator.mediaDevices.getDisplayMedia) {
                        throw new Error(t('live.mic_error') + ': Display media not supported');
                    }

                    try {
                        stream = await navigator.mediaDevices.getDisplayMedia({
                            video: {
                                width: 1,
                                height: 1,
                                frameRate: 1,
                            },
                            audio: {
                                echoCancellation: false,
                                noiseSuppression: false,
                                autoGainControl: false,
                            }
                        });

                        const audioTracks = stream.getAudioTracks();
                        if (audioTracks.length === 0) {
                            throw new Error('No audio track found in display media');
                        }
                        stream.getVideoTracks().forEach(track => track.stop());
                        stream = new MediaStream([audioTracks[0]]);
                    } catch (err) {
                        console.error('Error getting display media:', err);
                        throw err;
                    }
                }
            } else {
                // Microphone
                if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                    throw new Error('Media devices API not supported');
                }

                const constraints: MediaStreamConstraints = {
                    audio: config.microphoneId && config.microphoneId !== 'default'
                        ? { deviceId: { exact: config.microphoneId } }
                        : true
                };

                const micStream = await navigator.mediaDevices.getUserMedia(constraints);
                stream = micStream;
            }

            if (!usingNativeCaptureRef.current && stream) {
                activeStreamRef.current = stream;
                await initializeAudioSession(stream);

                // Mute system audio if configured and using microphone
                if (config.muteDuringRecording && inputSource === 'microphone') {
                    invoke('set_system_audio_mute', { mute: true })
                        .catch(err => console.error('Failed to mute system audio:', err));
                }
            }

            return true;

        } catch (error) {
            console.error('Failed to start capture:', error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            await alert(`${t('live.mic_error')} (${errorMessage})`, { variant: 'error' });
            return false;
        } finally {
            setIsRecordingInitializing(false);
        }
    }, [inputSource, t, alert, language]); // Added language to deps

    // Initialize AudioContext, Visualizer, and Transcription for Recording
    async function initializeAudioSession(stream: MediaStream): Promise<void> {
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
            // Send audio to the main transcription service
            transcriptionService.sendAudioInt16(e.data);
        };

        source.connect(processor);
        processor.connect(audioContextRef.current.destination);

        await configureService();

        await transcriptionService.start(
            onSegment,
            (error) => {
                console.error('Transcription error:', error);
            }
        );

        drawVisualizer();
    }

    // Start file recording (MediaRecorder)
    const startFileRecording = useCallback(async () => {
        if (usingNativeCaptureRef.current) {
            // For native capture, we rely on the system-audio event listener to accumulate data
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

            if (segments.length > 0 || duration > 1.0) {
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
    }, [setIsRecording, setIsPaused, clearSegments]);

    const stopRecordingSession = useCallback(async () => {
        console.log('[LiveRecord] Stopping recording session...');

        // Native Cleanup
        if (usingNativeCaptureRef.current) {
            try {
                await invoke('stop_system_audio_capture');
            } catch (e) { console.error(e); }

            // If using native capture, finish the file recording logic here manually
            // since we don't have a MediaRecorder onstop event.
            // Construct the full buffer
            const chunks = audioChunksRef.current;
            if (chunks.length > 0) {
                 const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
                 const fullBuffer = new Int16Array(totalLength);
                 let offset = 0;
                 for (const chunk of chunks) {
                     fullBuffer.set(chunk, offset);
                     offset += chunk.length;
                 }

                 // Encode to WAV
                 // 16kHz, 1 channel, 16 bit
                 const blob = encodeWAV(fullBuffer, 16000, 1, 16);
                 const url = URL.createObjectURL(blob);
                 useTranscriptStore.getState().setAudioUrl(url);

                 const segments = useTranscriptStore.getState().segments;
                 const duration = (Date.now() - startTimeRef.current) / 1000;

                 if (segments.length > 0 || duration > 1.0) {
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

        if (systemAudioUnlistenRef.current) {
            systemAudioUnlistenRef.current();
            systemAudioUnlistenRef.current = null;
        }

        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
            animationRef.current = 0;
        }

        if (audioContextRef.current) {
            activeStreamRef.current?.getTracks().forEach(t => t.stop());
            activeStreamRef.current = null;

            if (audioContextRef.current.state !== 'closed') {
                await audioContextRef.current.close();
            }
            audioContextRef.current = null;
        }

        // Soft stop the recording service
        await transcriptionService.softStop();

        // Unmute system audio if configured
        const config = useTranscriptStore.getState().config;
        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => console.error('Failed to unmute system audio:', err));
        }

        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
    }, []);

    const stopFileRecording = useCallback(() => {
        if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
            mediaRecorderRef.current.stop();
        }
        setIsRecording(false);
        setIsPaused(false);

        if (audioRef.current) {
            audioRef.current.pause();
            audioRef.current = null;
        }
    }, [setIsRecording, setIsPaused]);

    const pauseRecording = useCallback(() => {
        setIsPaused(true);
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'recording') {
            mediaRecorderRef.current.pause();
        }
        // Unmute when paused so user can hear
        const config = useTranscriptStore.getState().config;
        if (config.muteDuringRecording) {
            invoke('set_system_audio_mute', { mute: false })
                .catch(err => console.error('Failed to unmute system audio on pause:', err));
        }
    }, [setIsPaused]);

    const resumeRecording = useCallback(() => {
        setIsPaused(false);
        drawVisualizer();

        if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'paused') {
            mediaRecorderRef.current.resume();
        }
        // Mute again when resuming
        const config = useTranscriptStore.getState().config;
        if (config.muteDuringRecording && inputSource === 'microphone') {
            invoke('set_system_audio_mute', { mute: true })
                .catch(err => console.error('Failed to mute system audio on resume:', err));
        }
    }, [setIsPaused, drawVisualizer, inputSource]);

    const stopRecording = useCallback(async () => {
        stopFileRecording();
        await stopRecordingSession();
    }, [stopFileRecording, stopRecordingSession]);

    const handleToggleRecording = useCallback(async () => {
        if (isRecordingRef.current) {
            await stopRecording();
        } else {
            // Start recording
            // We do NOT check input source logic here anymore; startRecordingSession handles it
            const success = await startRecordingSession();
            if (success) {
                startFileRecording();
            }
        }
    }, [startRecordingSession, stopRecording, startFileRecording]);

    const handleTogglePause = useCallback(() => {
        if (isPausedRef.current) {
            resumeRecording();
        } else {
            pauseRecording();
        }
    }, [pauseRecording, resumeRecording]);

    const handleCaptionToggle = useCallback((checked: boolean) => {
        setIsCaptionMode(checked);
    }, [setIsCaptionMode]);

    function getRecordingStatusText(): string {
        if (isRecording) {
            // If recording, show recording status
            return isPaused ? t('live.recording_paused') : t('live.recording_active');
        }
        return t('live.start_hint');
    }

    // Keyboard shortcuts
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            // Ctrl+Space to toggle recording
            if (e.ctrlKey && e.code === 'Space') {
                e.preventDefault();
                handleToggleRecording();
            }
            // Space to toggle pause/resume (only when recording)
            else if (e.code === 'Space' && isRecordingRef.current) {
                e.preventDefault();
                handleTogglePause();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [handleToggleRecording, handleTogglePause]);

    // Monitor config changes and prepare transcription service (Recording Service)
    useEffect(() => {
        const prepareService = async () => {
            if (config.offlineModelPath) {
                console.log('[LiveRecord] Config loaded, preparing recording transcription service:', config.offlineModelPath);
                transcriptionService.setModelPath(config.offlineModelPath);
                transcriptionService.setVadModelPath(config.vadModelPath || '');
                transcriptionService.setPunctuationModelPath(config.punctuationModelPath || '');
                transcriptionService.setCtcModelPath(config.ctcModelPath || '');
                transcriptionService.setLanguage(config.language);

                // ITN Setup
                transcriptionService.setEnableITN(config.enableITN ?? false);
                const enabledITNModels = new Set(config.enabledITNModels || []);
                const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

                if (enabledITNModels.size > 0) {
                    try {
                        const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                        transcriptionService.setITNModelPaths(paths);
                    } catch (e) {
                        console.warn('[LiveRecord] Failed to setup ITN paths:', e);
                        transcriptionService.setITNModelPaths([]);
                    }
                } else {
                    transcriptionService.setITNModelPaths([]);
                }

                // Pre-spawn sidecar
                await transcriptionService.prepare();
            }
        };

        prepareService().catch(e => console.warn('Failed to prepare transcription service:', e));
    }, [
        config.offlineModelPath,
        config.vadModelPath,
        config.punctuationModelPath,
        config.ctcModelPath,
        config.enableITN,
        config.enabledITNModels,
        config.itnRulesOrder,
        config.language
    ]);


    // Init and cleanup
    useEffect(() => {
        // Pre-initialize AudioContext
        try {
            const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
            if (AudioContextClass) {
                audioContextRef.current = new AudioContext({ sampleRate: 16000 });
                audioContextRef.current.suspend();
            }
        } catch (e) {
            console.warn('Failed to pre-initialize AudioContext', e);
        }

        return () => {
            if (animationRef.current && typeof window.cancelAnimationFrame === 'function') {
                window.cancelAnimationFrame(animationRef.current);
            }
            if (audioContextRef.current) {
                audioContextRef.current.close();
            }
            if (audioRef.current) {
                audioRef.current.pause();
                audioRef.current = null;
            }
            // Stop transcription service when component unmounts
            transcriptionService.terminate().catch(e => console.error('Error stopping transcription service:', e));
        };
    }, []);

    return (
        <div className={`live-record-container ${className}`}>
            <div className="live-record-main-content">
                <div className="visualizer-wrapper">
                    <canvas
                        ref={canvasRef}
                        width={600}
                        height={120}
                        className="visualizer-canvas"
                        role="img"
                        aria-label={t('live.visualizer_label')}
                    />
                </div>

                <RecordingTimer isRecording={isRecording} isPaused={isPaused} />

                <div className="record-controls">
                    {!isRecording ? (
                        <button
                            className="control-button start"
                            onClick={handleToggleRecording}
                            disabled={isRecordingInitializing}
                            aria-label={t('live.start_recording')}
                            data-tooltip={isRecordingInitializing ? 'Initializing...' : t('live.start_recording')}
                            data-tooltip-pos="bottom"
                            style={isRecordingInitializing ? { opacity: 0.7, cursor: 'wait' } : {}}
                        >
                            <div className="control-button-inner" />
                        </button>
                    ) : (
                        <>
                            <button
                                className="control-button pause"
                                onClick={handleTogglePause}
                                disabled={isRecordingInitializing}
                                aria-label={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip-pos="bottom"
                            >
                                {isPaused ? <Play size={24} fill="currentColor" aria-hidden="true" /> : <Pause size={24} fill="currentColor" aria-hidden="true" />}
                            </button>

                            <button
                                className="control-button stop"
                                onClick={handleToggleRecording}
                                disabled={isRecordingInitializing}
                                aria-label={t('live.stop')}
                                data-tooltip={t('live.stop')}
                                data-tooltip-pos="bottom"
                            >
                                <Square size={28} fill="white" color="white" aria-hidden="true" />
                            </button>
                        </>
                    )}
                </div>

                {!isRecording && (
                    <div className="input-source-selector">
                        <div className="source-select-wrapper">
                            {getSourceIcon(inputSource)}
                            <Dropdown
                                value={inputSource}
                                onChange={(value) => setInputSource(value as 'microphone' | 'desktop')}
                                aria-label={t('live.source_select')}
                                options={[
                                    { value: 'microphone', label: t('live.source_microphone') },
                                    { value: 'desktop', label: t('live.source_desktop') }
                                ]}
                                style={{ minWidth: '180px' }}
                            />
                        </div>
                    </div>
                )}

                <p className="recording-status-text" aria-live="polite">
                    {getRecordingStatusText()}
                </p>
            </div>

            <div className="live-caption-toggle">
                <Switch
                    checked={isCaptionMode}
                    onChange={handleCaptionToggle}
                    label={t('live.caption_mode')}
                    disabled={false}
                />
                <span className="live-caption-hint">{t('live.caption_mode_hint')}</span>
            </div>

            <TranscriptionOptions
                enableTimeline={enableTimeline}
                setEnableTimeline={setEnableTimeline}
                language={language}
                setLanguage={setLanguage}
                disabled={isRecording}
            />
        </div>
    );
}
