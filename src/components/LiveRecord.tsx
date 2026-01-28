import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { useDialogStore } from '../stores/dialogStore';
import { transcriptionService } from '../services/transcriptionService';

import { Pause, Play, Square, Mic, Monitor, FileAudio } from 'lucide-react';

interface LiveRecordProps {
    className?: string;
    onOpenSettings?: () => void;
}

const getSupportedMimeType = () => {
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
};

const ModelSetupWarning: React.FC<{ onSetup: () => void }> = ({ onSetup }) => {
    const { t } = useTranslation();
    return (
        <div className="model-warning-overlay">
            <div className="model-warning-icon-wrapper">
                <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
            </div>
            <h3 className="model-warning-title">{t('settings.model_required_title')}</h3>
            <p className="model-warning-desc">
                {t('settings.model_required_desc')}
            </p>
            <button
                onClick={onSetup}
                className="btn-primary-large"
            >
                {t('settings.go_to_settings')}
            </button>
        </div>
    );
};

export const LiveRecord: React.FC<LiveRecordProps> = ({ className = '', onOpenSettings }) => {
    const { alert } = useDialogStore();
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const animationRef = useRef<number>(0);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const audioContextRef = useRef<AudioContext | null>(null);

    const [isRecording, setIsRecording] = useState(false);
    const [isPaused, setIsPaused] = useState(false);
    const [recordingTime, setRecordingTime] = useState(0);
    const isRecordingRef = useRef(false); // Use ref to track recording state for closure
    const isPausedRef = useRef(false);
    const mimeTypeRef = useRef<string>('');
    const [inputSource, setInputSource] = useState<'microphone' | 'desktop' | 'file'>('microphone');
    const [isModelReady, setIsModelReady] = useState(false);
    const [isModelLoading, setIsModelLoading] = useState(false);
    const [missingConfig, setMissingConfig] = useState(false);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const audioRef = useRef<HTMLAudioElement | null>(null);

    const config = useTranscriptStore((state) => state.config);
    // const setMode = useTranscriptStore((state) => state.setMode); // Removed as we use onOpenSettings prop now
    const upsertSegment = useTranscriptStore((state) => state.upsertSegment);
    const clearSegments = useTranscriptStore((state) => state.clearSegments);
    const { t } = useTranslation();

    // Format recording time
    const formatTime = (seconds: number) => {
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    };

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
    const startRecording = async () => {
        if (!isModelReady) return;

        if (inputSource === 'file') {
            fileInputRef.current?.click();
            return;
        }

        try {
            let stream: MediaStream;

            if (inputSource === 'desktop') {
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
                    // Fallback or re-throw? Re-throw mostly.
                    throw err;
                }
            } else {
                // Default microphone
                stream = await navigator.mediaDevices.getUserMedia({ audio: true });
            }

            await startRecordingWithStream(stream);

        } catch (error) {
            console.error('Failed to start recording:', error);
            alert(t('live.mic_error'), { variant: 'error' });
        }
    };

    const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;

        try {
            // Create audio element for playback
            const url = URL.createObjectURL(file);
            const audio = new Audio(url);
            audioRef.current = audio;

            // Wait for metadata to load to get duration etc if needed, but here we just need to play
            await audio.play(); // User interaction likely covered by the file input change event

            // Create AudioContext
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
            const source = audioContextRef.current.createMediaElementSource(audio);

            // Connect to speakers so user can hear it
            source.connect(audioContextRef.current.destination);

            // Connect to a destination node to get a stream for processing/recording
            const destination = audioContextRef.current.createMediaStreamDestination();
            source.connect(destination);

            const stream = destination.stream;

            // Auto-stop when audio ends
            audio.onended = () => {
                stopRecording();
            };

            await startRecordingWithStream(stream, true);

        } catch (error) {
            console.error('Failed to start file simulation:', error);
            alert(t('live.mic_error'), { variant: 'error' }); // Reuse error or add new one? Using generic for now
        } finally {
            // Reset input
            if (fileInputRef.current) fileInputRef.current.value = '';
        }
    }


    const startRecordingWithStream = async (stream: MediaStream, isFileSimulation = false) => {
        // Start new session to ensure isolation from previous recordings
        transcriptionService.startSession();

        // Set up audio context and analyser if not already created (File mode creates it earlier)
        if (!audioContextRef.current) {
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
        }

        // If reusing context (file mode), we might need to be careful. 
        // Actually in file mode we create context above. In mic/desktop we create here.

        // Source
        let source: MediaStreamAudioSourceNode;
        // Validating if source can be created from stream in existing context
        // If context was created for file, it already has the source connected to destination/stream.
        // But we need 'source' variable for Analyser connection below.

        if (isFileSimulation) {
            // For file simulation, the stream comes from destination, which is already a node.
            // We need to connect the stream to analyser.
            source = audioContextRef.current.createMediaStreamSource(stream);
        } else {
            source = audioContextRef.current.createMediaStreamSource(stream);
        }


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

        // We don't call start() here anymore, it's called on mount.
        // But we need to ensure the service knows we are "recording" for the purposes of handling data?
        // Actually, the service just processes what we send it.
        // And we only send data when `isRecordingRef.current` is true (in the processor callback).
        // So we just need to make sure the sidecar is running.

        // However, if for some reason the sidecar stopped (error?), we might need to restart it.
        // But start() is idempotent-ish now.
        // Let's just ensure it's "ready" check passed.


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

            // DON'T stop the service, just force the current segment to end
            // transformationService.stop(); 
            await transcriptionService.forceEndSegment();
        };

        mediaRecorderRef.current.start();
        setIsRecording(true);
        setIsPaused(false);
        isRecordingRef.current = true;
        isPausedRef.current = false;
        clearSegments();

        // Start visualizer
        drawVisualizer();
    };

    // Pause recording
    const pauseRecording = () => {
        if (mediaRecorderRef.current && isRecording && !isPaused) {
            mediaRecorderRef.current.pause();
            if (audioRef.current) audioRef.current.pause();
            setIsPaused(true);
            isPausedRef.current = true;
        }
    };

    // Resume recording
    const resumeRecording = () => {
        if (mediaRecorderRef.current && isRecording && isPaused) {
            mediaRecorderRef.current.resume();
            if (audioRef.current) audioRef.current.play();
            setIsPaused(false);
            isPausedRef.current = false;

            // Prevent double loops if resume happens quickly
            if (animationRef.current) {
                window.cancelAnimationFrame(animationRef.current);
            }

            // Restart visualizer loop
            drawVisualizer();
        }
    };

    // Stop recording
    const stopRecording = () => {
        if (audioRef.current) {
            audioRef.current.pause();
            audioRef.current = null;
        }

        if (mediaRecorderRef.current && isRecording) {
            mediaRecorderRef.current.stop();
            mediaRecorderRef.current.stream.getTracks().forEach(track => track.stop());
        }

        if (animationRef.current && typeof window.cancelAnimationFrame === 'function') {
            window.cancelAnimationFrame(animationRef.current);
        }

        if (audioContextRef.current && audioContextRef.current.state !== 'closed') {
            audioContextRef.current.close().catch(e => console.error('Error closing AudioContext:', e));
        }

        // Clear visualizer
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }

        setIsRecording(false);
        setIsPaused(false);
        isRecordingRef.current = false;
        isPausedRef.current = false;

        // We DO NOT stop the transcription service here, 
        // because we want to keep the model loaded for the next recording session.
        // The service will be stopped when the component unmounts.
    };


    // Better Timer Effect
    useEffect(() => {
        let interval: number | undefined;

        if (isRecording) {
            interval = window.setInterval(() => {
                if (!isPausedRef.current) {
                    setRecordingTime(t => t + 1);
                }
            }, 1000);
        } else {
            setRecordingTime(0);
        }

        return () => {
            if (interval) clearInterval(interval);
        };
    }, [isRecording]); // Only dependency is isRecording, uses ref for pause check

    // Initialize Model on Mount
    useEffect(() => {
        const initModel = async () => {
            if (!config.recognitionModelPath || !config.vadModelPath) {
                setMissingConfig(true);
                return;
            }

            setMissingConfig(false);
            setIsModelLoading(true);
            setIsModelReady(false);

            transcriptionService.setModelPath(config.recognitionModelPath);
            transcriptionService.setEnableITN(!!config.enableITN);
            transcriptionService.setVadModelPath(config.vadModelPath);
            transcriptionService.setPunctuationModelPath(config.punctuationModelPath || '');

            await transcriptionService.start(
                (segment) => {
                    upsertSegment(segment);
                },
                (error) => {
                    console.error('Transcription error:', error);
                    alert(t('live.error_transcription', { error }), { variant: 'error' });
                    setIsModelLoading(false);
                    // If error occurs, maybe we consider it not ready?
                },
                () => {
                    console.log('[LiveRecord] Model Ready');
                    setIsModelLoading(false);
                    setIsModelReady(true);
                }
            );
        };

        initModel();

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
            transcriptionService.stop();
        };
    }, [config.recognitionModelPath, config.vadModelPath, config.enableITN, config.punctuationModelPath]);
    // Re-run if config changes (user goes to settings and comes back)

    if (missingConfig) {
        return <ModelSetupWarning onSetup={() => onOpenSettings && onOpenSettings()} />;
    }

    return (
        <div className={`live-record-container relative h-full w-full ${className}`}>
            <input
                type="file"
                ref={fileInputRef}
                style={{ display: 'none' }}
                accept="audio/*,video/*"
                onChange={handleFileSelect}
            />
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

            <div
                className="recording-timer"
                style={{
                    visibility: isRecording ? 'visible' : 'hidden'
                }}
            >
                {formatTime(recordingTime)}
            </div>

            <div className="record-controls">
                {!isRecording ? (
                    <button
                        className={`control-button start ${!isModelReady ? 'opacity-50 cursor-not-allowed' : ''}`}
                        onClick={startRecording}
                        disabled={!isModelReady}
                        aria-label={t('live.start_recording')}
                        data-tooltip={isModelLoading ? t('live.loading_model', 'Loading Model...') : t('live.start_recording')}
                        data-tooltip-pos="bottom"
                    >
                        {isModelLoading ? (
                            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-white" />
                        ) : (
                            <div className="control-button-inner" />
                        )}
                    </button>
                ) : (
                    <>
                        <button
                            className="control-button pause"
                            onClick={isPaused ? resumeRecording : pauseRecording}
                            aria-label={isPaused ? t('live.resume') : t('live.pause')}
                            data-tooltip={isPaused ? t('live.resume') : t('live.pause')}
                            data-tooltip-pos="bottom"
                        >
                            {isPaused ? <Play size={24} fill="currentColor" /> : <Pause size={24} fill="currentColor" />}
                        </button>

                        <button
                            className="control-button stop"
                            onClick={stopRecording}
                            aria-label={t('live.stop')}
                            data-tooltip={t('live.stop')}
                            data-tooltip-pos="bottom"
                        >
                            <Square size={28} fill="white" color="white" />
                        </button>
                    </>
                )}
            </div>

            {!isRecording && (
                <div className="input-source-selector">
                    <div className="source-select-wrapper">
                        {inputSource === 'microphone' ? <Mic size={18} /> : (inputSource === 'desktop' ? <Monitor size={18} /> : <FileAudio size={18} />)}
                        <select
                            value={inputSource}
                            onChange={(e) => setInputSource(e.target.value as 'microphone' | 'desktop' | 'file')}
                            aria-label={t('live.source_select')}
                            className="source-select"
                        >
                            <option value="microphone">{t('live.source_microphone')}</option>
                            <option value="desktop">{t('live.source_desktop')}</option>
                            <option value="file">{t('live.source_file')}</option>
                        </select>
                    </div>
                </div>
            )}

            <p className="recording-status-text" aria-live="polite">
                {isRecording
                    ? (isPaused ? t('live.recording_paused') : t('live.recording_active'))
                    : t('live.start_hint')
                }
            </p>
        </div>
    );
};

export default LiveRecord;
