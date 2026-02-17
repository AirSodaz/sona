import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';
import { modelService } from '../services/modelService';
import { Pause, Play, Square, Mic, Monitor, MessageSquare, Lock, Unlock } from 'lucide-react';
import { historyService } from '../services/historyService';
import { useHistoryStore } from '../stores/historyStore';
import { splitByPunctuation } from '../utils/segmentUtils';
import { RecordingTimer } from './RecordingTimer';
import { Dropdown } from './Dropdown';
import { TranscriptionOptions } from './TranscriptionOptions';
import { Window, getCurrentWindow } from '@tauri-apps/api/window';

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


    const canvasRef = useRef<HTMLCanvasElement>(null);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const animationRef = useRef<number>(0);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const audioContextRef = useRef<AudioContext | null>(null);

    const [isRecording, setIsRecording] = useState(false);
    const [isPaused, setIsPaused] = useState(false);
    const isRecordingRef = useRef(false); // Use ref to track recording state for closure
    const isPausedRef = useRef(false);
    const mimeTypeRef = useRef<string>('');
    const [isInitializing, setIsInitializing] = useState(false);
    const [inputSource, setInputSource] = useState<'microphone' | 'desktop'>('microphone');

    // Live Caption Mode State
    const isLiveCaptionMode = useRef(false);
    const [isLiveCaptionActive, setIsLiveCaptionActive] = useState(false);
    const [isCaptionLocked, setIsCaptionLocked] = useState(true);
    const liveCaptionWindowRef = useRef<Window | null>(null);

    const config = useTranscriptStore((state) => state.config);
    const setConfig = useTranscriptStore((state) => state.setConfig);

    // Use config directly
    const enableTimeline = config.enableTimeline ?? true;
    const language = config.language;
    const enableTimelineRef = useRef(true);

    const setEnableTimeline = useCallback((val: boolean) => setConfig({ enableTimeline: val }), [setConfig]);
    const setLanguage = useCallback((val: string) => setConfig({ language: val }), [setConfig]);

    // Sync ref
    useEffect(() => {
        enableTimelineRef.current = enableTimeline;
    }, [enableTimeline]);

    const audioRef = useRef<HTMLAudioElement | null>(null);
    const startTimeRef = useRef<number>(0);

    const upsertSegmentAndSetActive = useTranscriptStore((state) => state.upsertSegmentAndSetActive);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);
    const setAudioFile = useTranscriptStore((state) => state.setAudioFile);
    // config declaration moved up
    const { t } = useTranslation();

    // Draw visualizer
    const drawVisualizer = useCallback(() => {
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

                // Optimization: Cache gradients based on value (0-255)
                // This prevents creating ~7680 CanvasGradient objects per second
                if (!gradients[value]) {
                    // Create gradient - Warm Black/Gray for Notion look
                    const gradient = ctx.createLinearGradient(0, canvas.height, 0, canvas.height - barHeight);
                    gradient.addColorStop(0, '#37352f'); // Notion Black
                    gradient.addColorStop(1, '#787774'); // Notion Gray
                    gradients[value] = gradient;
                }

                ctx.fillStyle = gradients[value]!;
                // Rounded tops would require more complex drawing (arc), simple rect is fine for now
                ctx.fillRect(x, canvas.height - barHeight, barWidth - 1, barHeight);

                x += barWidth;
            }
        };

        draw();
    }, []);

    // Toggle Live Caption Mode
    async function startLiveCaption() {
        if (isInitializing) return;

        isLiveCaptionMode.current = true;
        setIsLiveCaptionActive(true);
        setIsCaptionLocked(true); // Reset to locked
        setIsInitializing(true);

        try {
            // Get handle to caption window
            // In browser dev mode, Window might be undefined or getByLabel might throw
            try {
                if (Window) {
                    const win = await Window.getByLabel('live-caption');
                    if (win) {
                        liveCaptionWindowRef.current = win;
                        await win.show();
                        await win.setIgnoreCursorEvents(true); // Lock by default
                        await win.emit('caption-lock-state', { locked: true });
                    }
                }
            } catch (e) {
                console.warn('Live Caption Window not available (likely in browser mode)', e);
            }

            // Minimize main window if in Tauri
            try {
                if (getCurrentWindow) {
                    await getCurrentWindow().minimize();
                }
            } catch (e) {
                console.warn('Could not minimize window', e);
            }

            // Start recording (this will prompt for screen selection)
            // We pass control to startRecording, but we need to reset isInitializing there
            // Actually startRecording sets isInitializing=true inside.
            // So we set it to false here before calling to allow startRecording to proceed?
            // No, startRecording checks nothing about isInitializing but sets it.
            // But we set it here to prevent double clicks.
            // Let's just call startRecording.
            setIsInitializing(false); // Reset so startRecording can set it
            await startRecording();
        } catch (e) {
            console.error('Failed to start live caption:', e);
            stopRecording(); // Cleanup
        }
    }

    async function toggleCaptionLock() {
        const newLocked = !isCaptionLocked;
        setIsCaptionLocked(newLocked);

        if (liveCaptionWindowRef.current) {
            try {
                await liveCaptionWindowRef.current.setIgnoreCursorEvents(newLocked);
                await liveCaptionWindowRef.current.emit('caption-lock-state', { locked: newLocked });
            } catch (e) {
                console.error('Failed to toggle lock', e);
            }
        }
    }

    // Start recording
    async function startRecording(): Promise<void> {
        // Validation: Check if model is configured
        const config = useTranscriptStore.getState().config;
        if (!config.offlineModelPath) {
            await alert(t('batch.no_model_error'), { variant: 'error' });
            return;
        }

        // Reset player state
        setAudioFile(null);



        setIsInitializing(true);

        try {
            let stream: MediaStream;

            // In Live Caption mode, force desktop audio
            const currentSource = isLiveCaptionMode.current ? 'desktop' : inputSource;

            if (currentSource === 'desktop') {
                if (!navigator.mediaDevices || !navigator.mediaDevices.getDisplayMedia) {
                    throw new Error(t('live.mic_error') + ': Display media not supported');
                }

                try {
                    // Capture system audio
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

                    // We only need the audio track
                    const audioTracks = stream.getAudioTracks();
                    if (audioTracks.length === 0) {
                        throw new Error('No audio track found in display media');
                    }

                    // Stop video tracks immediately as we don't need them
                    stream.getVideoTracks().forEach(track => track.stop());

                    // Create a new stream with only the audio track
                    stream = new MediaStream([audioTracks[0]]);

                } catch (err) {
                    console.error('Error getting display media:', err);
                    throw err;
                }
            } else {
                // Check support
                if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                    throw new Error('Media devices API not supported in this environment');
                }

                try {
                    // Default microphone
                    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
                } catch (err: any) {
                    console.error('GetUserMedia Error:', err);
                    if (err.name === 'NotAllowedError' || err.name === 'PermissionDeniedError') {
                        throw new Error(t('live.mic_permission_denied') || 'Microphone permission denied');
                    } else if (err.name === 'NotFoundError' || err.name === 'DevicesNotFoundError') {
                        throw new Error(t('live.mic_not_found') || 'No microphone found');
                    } else if (err.name === 'NotReadableError' || err.name === 'TrackStartError') {
                        throw new Error(t('live.mic_busy') || 'Microphone is busy or not readable');
                    } else {
                        throw err;
                    }
                }
            }

            await initializeRecordingSession(stream);

        } catch (error) {
            console.error('Failed to start recording:', error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            await alert(`${t('live.mic_error')} (${errorMessage})`, { variant: 'error' });

            // Clean up if we failed
            if (isLiveCaptionMode.current) {
                isLiveCaptionMode.current = false;
                setIsLiveCaptionActive(false);
            }
        } finally {
            setIsInitializing(false);
        }
    }




    async function initializeRecordingSession(stream: MediaStream): Promise<void> {
        // Set up audio context and analyzer if not already created
        if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        } else if (audioContextRef.current.state === 'suspended') {
            await audioContextRef.current.resume();
        }

        // If reusing context (file mode), we might need to be careful. 
        // Actually in file mode we create context above. In mic/desktop we create here.

        // Source
        let source: MediaStreamAudioSourceNode;
        // Validating if source can be created from stream in existing context
        // If context was created for file, it already has the source connected to destination/stream.
        // But we need 'source' variable for Analyzer connection below.

        source = audioContextRef.current.createMediaStreamSource(stream);


        // Analyser for visualizer
        analyserRef.current = audioContextRef.current.createAnalyser();
        analyserRef.current.fftSize = 256;
        source.connect(analyserRef.current);

        // Processor for transcription streaming (16kHz requirement)
        // Use AudioWorklet for better performance (off-main-thread processing)
        try {
            await audioContextRef.current.audioWorklet.addModule('/audio-processor.js');
        } catch (err) {
            console.error('Failed to load audio worklet module:', err);
            throw new Error('Audio worklet failed to load');
        }

        const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');

        processor.port.onmessage = (e) => {
            if (!isRecordingRef.current || isPausedRef.current) return;

            // e.data is Int16Array transferred from the worklet
            transcriptionService.sendAudioInt16(e.data);
        };

        source.connect(processor);

        // Only connect processor to destination if NOT file simulation (to avoid double audio or feedback loop if we were doing pass-through)
        // Actually processor output is usually silence/empty, it's a tap.
        // But in original code: processor.connect(audioContextRef.current.destination);
        // This is required for AudioWorklet to run in some browsers/contexts (needs to be connected to destination).
        // Since we don't output audio from processor (we just send data to socket), this is fine.
        processor.connect(audioContextRef.current.destination);

        // Start transcription service
        const config = useTranscriptStore.getState().config;
        console.log('[LiveRecord] Starting transcription with model path:', config.offlineModelPath);
        transcriptionService.setModelPath(config.offlineModelPath);
        transcriptionService.setLanguage(language);

        // ITN Configuration
        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];

        transcriptionService.setEnableITN(config.enableITN ?? false);

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

        await transcriptionService.start(
            (segment) => {
                console.log('[LiveRecord] Received segment:', segment);

                // Live Caption Emission
                if (isLiveCaptionMode.current && liveCaptionWindowRef.current) {
                    liveCaptionWindowRef.current.emit('caption-update', {
                        text: segment.text,
                        isFinal: segment.isFinal
                    }).catch(e => console.warn('Failed to emit caption update', e));
                }

                // If timeline mode is enabled and segment is final, we split it
                if (enableTimelineRef.current && segment.isFinal) {
                    const parts = splitByPunctuation([segment]);

                    if (parts.length > 0) {
                         // Remove the original ID which might have been upserted as partial
                         useTranscriptStore.getState().deleteSegment(segment.id);

                         // Add all parts
                         parts.forEach(part => useTranscriptStore.getState().upsertSegment(part));

                         // Set active to the last part
                         useTranscriptStore.getState().setActiveSegmentId(parts[parts.length - 1].id);
                    } else {
                        upsertSegmentAndSetActive(segment);
                    }
                } else {
                    upsertSegmentAndSetActive(segment);
                }
            },
            (error) => {
                console.error('Transcription error:', error);
                alert(`${t('live.mic_error')} (${error})`, { variant: 'error' });
                if (isLiveCaptionMode.current) {
                    stopRecording();
                }
            }
        );

        // Set up media recorder for full file save
        const mimeType = getSupportedMimeType();
        mimeTypeRef.current = mimeType;
        console.log('[LiveRecord] Using mimeType:', mimeType);

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

            // Wait for sidecar to finalize the last segment before saving
            await transcriptionService.softStop();

            // Skip saving to history if in Live Caption mode
            if (isLiveCaptionMode.current) {
                console.log('[LiveRecord] Live Caption mode: skipping history save (ephemeral).');
                isLiveCaptionMode.current = false; // Reset for next session
                return;
            }

            // Save to History (after softStop so final segment is included)
            const segments = useTranscriptStore.getState().segments;
            const duration = (Date.now() - startTimeRef.current) / 1000;

            // Only save if we have data (segments or substantial audio)
            if (segments.length > 0 || duration > 1.0) {
                try {
                    const newItem = await historyService.saveRecording(blob, segments, duration);
                    if (newItem) {
                        useHistoryStore.getState().addItem(newItem);
                        useTranscriptStore.getState().setSourceHistoryId(newItem.id);
                    }
                } catch (err) {
                    console.error('Failed to save history:', err);
                }
            }
        };

        mediaRecorderRef.current.start();
        setIsRecording(true);
        setIsPaused(false);
        isRecordingRef.current = true;
        isPausedRef.current = false;
        startTimeRef.current = Date.now();
        clearSegments();

        // Start visualizer
        drawVisualizer();
    }


    // Pause recording
    function pauseRecording(): void {
        setIsPaused(true);
        isPausedRef.current = true;

        if (mediaRecorderRef.current && isRecordingRef.current && mediaRecorderRef.current.state === 'recording') {
            mediaRecorderRef.current.pause();
            if (audioRef.current) audioRef.current.pause();
        }
    }

    // Resume recording
    function resumeRecording(): void {
        setIsPaused(false);
        isPausedRef.current = false;

        // Prevent double loops if resume happens quickly
        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
        }
        // Restart visualizer loop
        drawVisualizer();

        if (mediaRecorderRef.current && isRecordingRef.current && mediaRecorderRef.current.state === 'paused') {
            mediaRecorderRef.current.resume();
            if (audioRef.current) audioRef.current.play();
        }
    }

    // Stop recording
    function stopRecording(): void {
        // Immediate UI Update
        setIsRecording(false);
        setIsPaused(false);
        isRecordingRef.current = false;
        isPausedRef.current = false;

        if (audioRef.current) {
            audioRef.current.pause();
            audioRef.current = null;
        }

        // Stop and Hide Live Caption Window if active
        if (isLiveCaptionMode.current) {
            if (liveCaptionWindowRef.current) {
                liveCaptionWindowRef.current.hide().catch(console.warn);
            }
            // We reset refs here or after?
            // Better to reset after to ensure onstop has access if needed, but onstop logic is synchronous in execution order regarding ref?
            // Actually onstop runs when mediaRecorder.stop() is called below.
            // We need isLiveCaptionMode.current to be true during onstop execution.
            // So we reset it AFTER mediaRecorder stop triggers.

            // But mediaRecorder.stop() is async in event dispatch.
            // However, we can just reset it after a small delay or trust that onstop runs immediately?
            // Actually onstop is an event.
            // Let's reset setIsLiveCaptionActive(false) for UI, but keep ref true until we are sure?
            // No, safely reset state for next time is key.
            // But if we reset ref now, onstop will see false and save history.
            // FIX: We can set a flag `wasLiveCaption` or simply delay resetting the ref.
            // Or easier: inside onstop, we check a local variable if we captured it? No, closure.
            // Best approach: reset `isLiveCaptionMode.current = false` inside `onstop`.
            setIsLiveCaptionActive(false);
        }

        if (mediaRecorderRef.current) {
            mediaRecorderRef.current.stop();
            mediaRecorderRef.current.stream.getTracks().forEach(track => track.stop());
        }

        if (animationRef.current && typeof window.cancelAnimationFrame === 'function') {
            window.cancelAnimationFrame(animationRef.current);
        }

        if (audioContextRef.current && audioContextRef.current.state !== 'closed') {
            audioContextRef.current.suspend().catch(e => console.error('Error suspending AudioContext:', e));
            // Don't close, just suspend to reuse or let it be closed by unmount
        }

        // Clear visualizer
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
    }

    function getRecordingStatusText(): string {
        if (isLiveCaptionActive) {
            return t('Live Caption Mode Active');
        }
        if (isRecording) {
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
                if (isRecordingRef.current) {
                    stopRecording();
                } else {
                    startRecording();
                }
            }
            // Space to toggle pause/resume (only when recording)
            else if (e.code === 'Space' && isRecordingRef.current) {
                e.preventDefault();
                if (isPausedRef.current) {
                    resumeRecording();
                } else {
                    pauseRecording();
                }
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, []);

    // Monitor config changes and prepare transcription service
    useEffect(() => {
        const prepareService = async () => {
            if (config.offlineModelPath) {
                console.log('[LiveRecord] Config loaded, preparing transcription service:', config.offlineModelPath);
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

                // Pre-spawn sidecar now that we have the model path
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
                // Suspend immediately so it doesn't consume resources or start playback until needed
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
            // Stop transcription service when component unmounts to prevent detached state
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
                        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                            {/* Standard Recording Button */}
                            <button
                                className="control-button start"
                                onClick={startRecording}
                                disabled={isInitializing}
                                aria-label={t('live.start_recording')}
                                data-tooltip={isInitializing ? 'Initializing...' : t('live.start_recording')}
                                data-tooltip-pos="bottom"
                                style={isInitializing ? { opacity: 0.7, cursor: 'wait' } : {}}
                            >
                                <div className="control-button-inner" />
                            </button>

                            {/* Live Caption Button */}
                             <button
                                className="control-button"
                                onClick={startLiveCaption}
                                disabled={isInitializing}
                                aria-label="Start Live Caption"
                                data-tooltip="Start Live Caption (Overlay)"
                                data-tooltip-pos="bottom"
                                style={{
                                    backgroundColor: '#3b82f6', // distinct blue color
                                    ...((isInitializing) ? { opacity: 0.7, cursor: 'wait' } : {})
                                }}
                            >
                                <MessageSquare size={24} fill="white" color="white" aria-hidden="true" />
                            </button>
                        </div>
                    ) : (
                        <div style={{ display: 'flex', gap: '16px', alignItems: 'center' }}>
                             {/* Pause/Resume */}
                            <button
                                className="control-button pause"
                                onClick={isPaused ? resumeRecording : pauseRecording}
                                disabled={isInitializing}
                                aria-label={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip-pos="bottom"
                            >
                                {isPaused ? <Play size={24} fill="currentColor" aria-hidden="true" /> : <Pause size={24} fill="currentColor" aria-hidden="true" />}
                            </button>

                            {/* Stop */}
                            <button
                                className="control-button stop"
                                onClick={stopRecording} // Using stopRecording directly, ref reset handled in onstop or needs fix
                                disabled={isInitializing}
                                aria-label={t('live.stop')}
                                data-tooltip={t('live.stop')}
                                data-tooltip-pos="bottom"
                            >
                                <Square size={28} fill="white" color="white" aria-hidden="true" />
                            </button>

                            {/* Unlock/Lock Toggle (Only for Live Caption) */}
                            {isLiveCaptionActive && (
                                <button
                                    className="control-button"
                                    onClick={toggleCaptionLock}
                                    aria-label={isCaptionLocked ? "Unlock Caption Window" : "Lock Caption Window"}
                                    data-tooltip={isCaptionLocked ? "Unlock Window (Make Draggable)" : "Lock Window (Click-through)"}
                                    data-tooltip-pos="bottom"
                                    style={{ backgroundColor: isCaptionLocked ? '#4b5563' : '#eab308' }} // Gray if locked, Yellow if unlocked
                                >
                                    {isCaptionLocked ?
                                        <Lock size={20} color="white" /> :
                                        <Unlock size={20} color="white" />
                                    }
                                </button>
                            )}
                        </div>
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
