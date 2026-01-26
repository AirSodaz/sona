import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranscriptStore } from '../stores/transcriptStore';
import { transcriptionService } from '../services/transcriptionService';
import { Pause, Play, Square } from 'lucide-react';

interface LiveRecordProps {
    className?: string;
}

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

        const draw = () => {
            animationRef.current = requestAnimationFrame(draw);

            // If paused, keep existing frame (freeze)
            if (isPausedRef.current) {
                return;
            }

            analyser.getByteFrequencyData(dataArray);

            ctx.clearRect(0, 0, canvas.width, canvas.height); // Use CSS background

            const barWidth = (canvas.width / bufferLength) * 2.5;
            let x = 0;

            for (let i = 0; i < bufferLength; i++) {
                const barHeight = (dataArray[i] / 255) * canvas.height * 0.8;

                // Create gradient - Warm Black/Gray for Notion look
                const gradient = ctx.createLinearGradient(0, canvas.height, 0, canvas.height - barHeight);
                gradient.addColorStop(0, '#37352f'); // Notion Black
                gradient.addColorStop(1, '#787774'); // Notion Gray

                ctx.fillStyle = gradient;
                // Rounded tops would require more complex drawing (arc), simple rect is fine for now
                ctx.fillRect(x, canvas.height - barHeight, barWidth - 1, barHeight);

                x += barWidth;
            }
        };

        draw();
    }, []);

    // Start recording
    const startRecording = async () => {
        try {
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

            // Set up audio context and analyser
            // Set up audio context and analyser with specific sample rate
            audioContextRef.current = new AudioContext({ sampleRate: 16000 });
            const source = audioContextRef.current.createMediaStreamSource(stream);

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
            processor.connect(audioContextRef.current.destination);

            // Start transcription service
            const config = useTranscriptStore.getState().config;
            console.log('[LiveRecord] Starting transcription with model path:', config.modelPath);
            transcriptionService.setModelPath(config.modelPath);
            transcriptionService.setEnableITN(!!config.enableITN);

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
            mediaRecorderRef.current = new MediaRecorder(stream);
            const chunks: Blob[] = [];

            mediaRecorderRef.current.ondataavailable = (e) => {
                chunks.push(e.data);
            };

            mediaRecorderRef.current.onstop = () => {
                const blob = new Blob(chunks, { type: 'audio/webm' });
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

        } catch (error) {
            console.error('Failed to start recording:', error);
            alert(t('live.mic_error'));
        }
    };

    // Pause recording
    const pauseRecording = () => {
        if (mediaRecorderRef.current && isRecording && !isPaused) {
            mediaRecorderRef.current.pause();
            setIsPaused(true);
            isPausedRef.current = true;
        }
    };

    // Resume recording
    const resumeRecording = () => {
        if (mediaRecorderRef.current && isRecording && isPaused) {
            mediaRecorderRef.current.resume();
            setIsPaused(false);
            isPausedRef.current = false;
        }
    };

    // Stop recording
    const stopRecording = () => {
        if (mediaRecorderRef.current && isRecording) {
            mediaRecorderRef.current.stop();
            mediaRecorderRef.current.stream.getTracks().forEach(track => track.stop());
        }

        if (animationRef.current) {
            cancelAnimationFrame(animationRef.current);
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

    // Recording timer
    useEffect(() => {
        let interval: number | undefined;

        if (isRecording && !isPaused) {
            // Don't reset time here if it's just a resume, but logic above handles resets on start
            // If we paused, isRecording is still true.
            // If we start new, setRecordingTime(0) is called? No, it was inside this effect.
            // We need to be careful not to reset time on resume.
            // The previous logic was: if (isRecording) { setRecordingTime(0); ... }
            // This would reset time every time isRecording changed to true.
            // But now we have isPaused changing.
        }

        // Refactored Timer Logic
        if (isRecording) {
            if (recordingTime === 0 && !isPaused) {
                // Initial start
            }

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
    }, [isRecording, isPaused]); // Re-run when pause state changes? No, better to use ref inside interval or just careful dependency

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
            if (animationRef.current) {
                cancelAnimationFrame(animationRef.current);
            }
            if (audioContextRef.current) {
                audioContextRef.current.close();
            }
        };
    }, []);

    return (
        <div className={`live-record ${className}`} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 24, padding: 32 }}>
            <div className="visualizer-container" style={{ width: '100%', height: 120, borderRadius: 12 }}>
                <canvas
                    ref={canvasRef}
                    width={600}
                    height={120}
                    style={{ width: '100%', height: '100%', borderRadius: 12 }}
                />
            </div>

            <div style={{
                fontFamily: 'var(--font-mono)',
                fontSize: '1.5rem',
                color: 'var(--color-recording)',
                visibility: isRecording ? 'visible' : 'hidden'
            }}>
                {formatTime(recordingTime)}
            </div>

            <div className="controls" style={{ display: 'flex', gap: 24, alignItems: 'center' }}>
                {!isRecording ? (
                    <button
                        className="control-button start"
                        onClick={startRecording}
                        style={{
                            width: 64, height: 64, borderRadius: '50%',
                            backgroundColor: '#ef4444', border: 'none', cursor: 'pointer',
                            display: 'flex', alignItems: 'center', justifyContent: 'center',
                            boxShadow: '0 4px 12px rgba(239, 68, 68, 0.3)',
                            transition: 'all 0.2s ease'
                        }}
                    >
                        <div style={{ width: 24, height: 24, backgroundColor: 'white', borderRadius: '50%' }} />
                    </button>
                ) : (
                    <>
                        <button
                            className="control-button pause"
                            onClick={isPaused ? resumeRecording : pauseRecording}
                            style={{
                                width: 56, height: 56, borderRadius: '50%',
                                backgroundColor: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)',
                                cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center',
                                color: 'var(--color-text)', transition: 'all 0.2s ease'
                            }}
                            title={isPaused ? t('live.resume') : t('live.pause')}
                        >
                            {isPaused ? <Play size={24} fill="currentColor" /> : <Pause size={24} fill="currentColor" />}
                        </button>

                        <button
                            className="control-button stop"
                            onClick={stopRecording}
                            style={{
                                width: 64, height: 64, borderRadius: '50%',
                                backgroundColor: '#ef4444', border: 'none', cursor: 'pointer',
                                display: 'flex', alignItems: 'center', justifyContent: 'center',
                                boxShadow: '0 4px 12px rgba(239, 68, 68, 0.3)',
                                transition: 'all 0.2s ease'
                            }}
                            title={t('live.stop')}
                        >
                            <Square size={28} fill="white" color="white" />
                        </button>
                    </>
                )}
            </div>

            <p style={{ color: 'var(--color-text-muted)', fontSize: '0.875rem' }}>
                {isRecording
                    ? (isPaused ? t('live.recording_paused') : t('live.recording_active'))
                    : t('live.start_hint')
                }
            </p>
        </div>
    );
};

export default LiveRecord;
