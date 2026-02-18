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
    // Track the active media stream to share between Capture (Transcription) and Recording (File)
    const activeStreamRef = useRef<MediaStream | null>(null);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const setIsRecording = useTranscriptStore((state) => state.setIsRecording);
    const setIsPaused = useTranscriptStore((state) => state.setIsPaused);

    const isRecordingRef = useRef(false); // Use ref to track recording state for closure
    const isPausedRef = useRef(false);

    // Sync refs with store state
    useEffect(() => {
        isRecordingRef.current = isRecording;
    }, [isRecording]);

    useEffect(() => {
        isPausedRef.current = isPaused;
    }, [isPaused]);

    const mimeTypeRef = useRef<string>('');
    const [isInitializing, setIsInitializing] = useState(false);
    const [inputSource, setInputSource] = useState<'microphone' | 'desktop'>('microphone');
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);
    const setIsCaptionMode = useTranscriptStore((state) => state.setIsCaptionMode);
    const isCaptionModeRef = useRef(false);

    // Sync caption mode ref
    useEffect(() => {
        isCaptionModeRef.current = isCaptionMode;
    }, [isCaptionMode]);

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

    // Start recording

    // Start capture (audio context + visualizer + transcription)
    const startCapture = useCallback(async (forceSource?: 'microphone' | 'desktop', isCaption: boolean = false) => {
        const config = useTranscriptStore.getState().config;
        if (!config.offlineModelPath) {
            await alert(t('batch.no_model_error'), { variant: 'error' });
            return false;
        }

        setIsInitializing(true);

        const effectiveSource = forceSource || inputSource;

        try {
            let stream: MediaStream;

            if (effectiveSource === 'desktop') {
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
            } else {
                if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
                    throw new Error('Media devices API not supported');
                }
                const micStream = await navigator.mediaDevices.getUserMedia({ audio: true });
                stream = micStream;
            }

            activeStreamRef.current = stream;
            await initializeAudioSession(stream);

            // Use the explicit flag if provided, otherwise fallback to store
            // We check store state as a fallback because sometimes we just start capture (e.g. for recording) 
            // and we want to know if caption should also be open.
            // But if called from handleCaptionToggle(true), we pass isCaption=true.
            if (isCaption || useTranscriptStore.getState().isCaptionMode) {
                captionWindowService.open().catch(console.error);
            }

            return true;

        } catch (error) {
            console.error('Failed to start capture:', error);
            const errorMessage = error instanceof Error ? error.message : String(error);
            await alert(`${t('live.mic_error')} (${errorMessage})`, { variant: 'error' });
            return false;
        } finally {
            setIsInitializing(false);
        }
    }, [inputSource, t, alert]);

    // Initialize AudioContext, Visualizer, and Transcription
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
            // Always send audio if we are "capturing" (which means either recording OR captioning)
            // We'll filter what to do with the text in the callback
            transcriptionService.sendAudioInt16(e.data);
        };

        source.connect(processor);
        processor.connect(audioContextRef.current.destination);

        // Prepare proper config
        const config = useTranscriptStore.getState().config;
        transcriptionService.setModelPath(config.offlineModelPath);
        transcriptionService.setLanguage(language);
        transcriptionService.setEnableITN(config.enableITN ?? false);

        // ITN Setup
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

        await transcriptionService.start(
            (segment) => {
                // Logic for handling segments based on active modes

                // 1. Caption Mode: Send to window
                if (isCaptionModeRef.current) {
                    captionWindowService.sendSegments([segment]).catch(console.error);
                }

                // 2. Recording Mode: Save to store (Main Window)
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
            },
            (error) => {
                console.error('Transcription error:', error);
            }
        );

        // Also setup MediaRecorder if we are recording (or if we need it ready?)
        // Actually, we should separate MediaRecorder start. 
        // We need to keep the stream reference to start recording later if needed.
        // Let's store stream in a ref
        // (We need a new ref for the stream)
        // For now, let's assume this initializes "Capture" which allows Captioning.
        // Recording needs to piggyback on this stream.

        // We'll assign it to a ref to be used by startFileRecording
        (window as any).currentStream = stream; // Temporary hack to pass stream, or use a ref

        drawVisualizer();
    }



    // Start file recording (MediaRecorder)
    const startFileRecording = useCallback(async () => {
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

            // Wait for sidecar to finalize?
            // If we are still captioning, we shouldn't stop the sidecar.
            // But valid file recording usually expects the "Final" segment.
            // We'll do a soft stop ONLY if we are also stopping capture.
            // If we are just stopping recording but keeping caption, we just save what we have.

            // Actually, softStop sends __RESET__ which might interrupt captioning flow?
            // If we are captioning, we might just want to save current state.

            // For now, let's assume we don't softStop if caption is active, 
            // implying the last segment might remain "partial" in the saved file transcript 
            // if silence didn't trigger final.

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








    const stopCapture = useCallback(async () => {
        console.log('[LiveRecord] Stopping capture...');

        if (animationRef.current) {
            window.cancelAnimationFrame(animationRef.current);
            animationRef.current = 0;
        }

        if (audioContextRef.current) {
            activeStreamRef.current?.getTracks().forEach(t => t.stop());
            activeStreamRef.current = null;

            if (audioContextRef.current.state !== 'closed') {
                await audioContextRef.current.suspend();
            }
        }

        await transcriptionService.softStop();
        captionWindowService.close().catch(e => console.error('Failed to close caption window:', e));

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
    }, [setIsPaused]);

    const resumeRecording = useCallback(() => {
        setIsPaused(false);
        drawVisualizer();

        if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'paused') {
            mediaRecorderRef.current.resume();
        }
    }, [setIsPaused, drawVisualizer]);

    const stopRecording = useCallback(async () => {
        stopFileRecording();
        if (!isCaptionModeRef.current) {
            await stopCapture();
        }
    }, [stopFileRecording, stopCapture]);

    const handleToggleRecording = useCallback(async () => {
        if (isRecordingRef.current) {
            await stopRecording();
        } else {
            if (activeStreamRef.current) {
                startFileRecording();
            } else {
                const success = await startCapture();
                if (success) {
                    startFileRecording();
                }
            }
        }
    }, [startCapture, stopRecording, startFileRecording]);

    const handleTogglePause = useCallback(() => {
        if (isPausedRef.current) {
            resumeRecording();
        } else {
            pauseRecording();
        }
    }, [pauseRecording, resumeRecording]);

    const handleCaptionToggle = useCallback(async (checked: boolean) => {
        setIsCaptionMode(checked);

        if (checked) {
            // Always try to open the caption window
            captionWindowService.open().catch(console.error);

            // If not recording, we need to start capture solely for captioning
            if (!isRecordingRef.current) {
                await startCapture('desktop', true);
            }
        } else {
            captionWindowService.close().catch(e => console.error(e));
            // Only stop capture if we are NOT recording
            if (!isRecordingRef.current) {
                await stopCapture();
            }
        }
    }, [setIsCaptionMode, startCapture, stopCapture]);

    function getRecordingStatusText(): string {
        if (isRecording) {
            if (isCaptionMode) {
                return isPaused ? t('live.recording_paused') : t('live.caption_active');
            }
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
                        <button
                            className="control-button start"
                            onClick={handleToggleRecording}
                            disabled={isInitializing}
                            aria-label={t('live.start_recording')}
                            data-tooltip={isInitializing ? 'Initializing...' : t('live.start_recording')}
                            data-tooltip-pos="bottom"
                            style={isInitializing ? { opacity: 0.7, cursor: 'wait' } : {}}
                        >
                            <div className="control-button-inner" />
                        </button>
                    ) : (
                        <>
                            <button
                                className="control-button pause"
                                onClick={handleTogglePause}
                                disabled={isInitializing}
                                aria-label={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip={isPaused ? t('live.resume') : t('live.pause')}
                                data-tooltip-pos="bottom"
                            >
                                {isPaused ? <Play size={24} fill="currentColor" aria-hidden="true" /> : <Pause size={24} fill="currentColor" aria-hidden="true" />}
                            </button>

                            <button
                                className="control-button stop"
                                onClick={handleToggleRecording}
                                disabled={isInitializing}
                                aria-label={t('live.stop')}
                                data-tooltip={t('live.stop')}
                                data-tooltip-pos="bottom"
                            >
                                <Square size={28} fill="white" color="white" aria-hidden="true" />
                            </button>
                        </>
                    )}
                </div>

                {!isRecording && !isCaptionMode && (
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
