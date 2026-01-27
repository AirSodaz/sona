import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { transcriptionService } from '../services/transcriptionService';
import { modelService } from '../services/modelService';
import { Pause, Play, Square, Mic, Monitor, FileAudio } from 'lucide-react';

interface LiveRecordProps {
    className?: string;
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

export const LiveRecord: React.FC<LiveRecordProps> = ({ className = '' }) => {
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
    const fileInputRef = useRef<HTMLInputElement>(null);
    const audioRef = useRef<HTMLAudioElement | null>(null);

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
            animationRef.current = window.requestAnimationFrame(draw);

            // If paused, keep existing frame (freeze)
            if (isPausedRef.current) {
                return;
            }

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
            alert(t('live.mic_error'));
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
            alert(t('live.mic_error')); // Reuse error or add new one? Using generic for now
        } finally {
            // Reset input
            if (fileInputRef.current) fileInputRef.current.value = '';
        }
    }


    const startRecordingWithStream = async (stream: MediaStream, isFileSimulation = false) => {
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

        // Start transcription service
        const config = useTranscriptStore.getState().config;
        console.log('[LiveRecord] Starting transcription with model path:', config.streamingModelPath);
        transcriptionService.setModelPath(config.streamingModelPath);


        // ITN Configuration
        // ITN Configuration
        // ITN Configuration
        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number', 'itn-new-heteronym', 'itn-phone'];
        // Legacy support or fallback: if order doesn't cover all enabled models, append them?
        // Actually, let's assume order covers all models or at least we check enabled.

        transcriptionService.setEnableITN(enabledITNModels.size > 0);

        if (enabledITNModels.size > 0) {
            try {
                const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                transcriptionService.setITNModelPaths(paths);
            } catch (e) {
                console.warn('[LiveRecord] Failed to setup ITN paths:', e);
            }
        }

        if (config.punctuationModelPath) {
            transcriptionService.setPunctuationModelPath(config.punctuationModelPath);
        } else {
            transcriptionService.setPunctuationModelPath('');
        }

        await transcriptionService.start(
            (segment) => {
                console.log('[LiveRecord] Received segment:', segment);
                upsertSegment(segment);
            },
            (error) => {
                console.error('Transcription error:', error);
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

        mediaRecorderRef.current.onstop = () => {
            const type = mimeTypeRef.current || mediaRecorderRef.current?.mimeType || 'audio/webm';
            const blob = new Blob(chunks, { type });
            const url = URL.createObjectURL(blob);
            useTranscriptStore.getState().setAudioUrl(url);
            transcriptionService.stop();
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

    // Cleanup on unmount
    useEffect(() => {
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
        };
    }, []);

    return (
        <div className={`live-record-container ${className}`}>
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
                        className="control-button start"
                        onClick={startRecording}
                        aria-label={t('live.start_recording')}
                        data-tooltip={t('live.start_recording')}
                        data-tooltip-pos="bottom"
                    >
                        <div className="control-button-inner" />
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

            <p className="recording-status-text">
                {isRecording
                    ? (isPaused ? t('live.recording_paused') : t('live.recording_active'))
                    : t('live.start_hint')
                }
            </p>
        </div>
    );
};

export default LiveRecord;
