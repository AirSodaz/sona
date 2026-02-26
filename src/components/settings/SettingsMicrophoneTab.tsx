import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../../stores/transcriptStore';

interface AudioDevice {
    name: string;
}

interface SettingsMicrophoneTabProps {
    microphoneId: string;
    setMicrophoneId: (id: string) => void;
    systemAudioDeviceId: string;
    setSystemAudioDeviceId: (id: string) => void;
    muteDuringRecording: boolean;
    setMuteDuringRecording: (enabled: boolean) => void;
}

export function SettingsMicrophoneTab({
    microphoneId,
    setMicrophoneId,
    systemAudioDeviceId,
    setSystemAudioDeviceId,
    muteDuringRecording,
    setMuteDuringRecording
}: SettingsMicrophoneTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [systemDevices, setSystemDevices] = useState<{ label: string; value: string }[]>([]);

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const audioContextRef = useRef<AudioContext | null>(null);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
    const animationRef = useRef<number>(0);
    const streamRef = useRef<MediaStream | null>(null);

    // System Audio Visualizer Refs
    const systemCanvasRef = useRef<HTMLCanvasElement>(null);
    const systemAudioContextRef = useRef<AudioContext | null>(null);
    const systemAnalyserRef = useRef<AnalyserNode | null>(null);
    const systemAnimationRef = useRef<number>(0);
    const systemUnlistenRef = useRef<UnlistenFn | null>(null);
    const nextAudioTimeRef = useRef<number>(0);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);

    // We only control the system capture if it's not already running for recording/captioning
    const isActiveSession = isRecording || isCaptionMode;

    // Enumerate devices
    useEffect(() => {
        let isMounted = true;

        async function getDevices() {
            try {
                // Check if we have permission first, otherwise labels might be empty
                // We'll try to get a temporary stream to trigger permission prompt if needed
                // or just rely on existing permissions.
                // If labels are empty, we might need to prompt.

                const devs = await navigator.mediaDevices.enumerateDevices();
                const audioInputs = devs.filter(d => d.kind === 'audioinput');

                // If no labels, we might need to request permission
                const hasLabels = audioInputs.some(d => d.label.length > 0);
                if (!hasLabels) {
                    try {
                        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
                        stream.getTracks().forEach(t => t.stop());
                        // Try again
                        const newDevs = await navigator.mediaDevices.enumerateDevices();
                         const newAudioInputs = newDevs.filter(d => d.kind === 'audioinput');
                         if (isMounted) {
                             formatAndSetDevices(newAudioInputs);
                         }
                    } catch (err) {
                        console.warn('Microphone permission denied or error', err);
                    }
                } else {
                     if (isMounted) {
                         formatAndSetDevices(audioInputs);
                     }
                }

            } catch (err) {
                console.error('Error enumerating devices:', err);
            }
        }

        function formatAndSetDevices(inputs: MediaDeviceInfo[]) {
            const options = [
                { label: t('settings.mic_auto'), value: 'default' },
                ...inputs.map(d => ({
                    label: d.label || `Microphone ${d.deviceId.slice(0, 5)}...`,
                    value: d.deviceId
                }))
            ];
            // Remove duplicates (sometimes default and deviceId are same physical device, but we want distinct options in UI?
            // Usually 'default' is separate.
            // But if we have multiple entries with same deviceId, filter them.
            // enumerateDevices returns unique deviceIds usually.

            // Deduplicate by value just in case
            const uniqueOptions = options.filter((opt, index, self) =>
                index === self.findIndex((t) => (
                    t.value === opt.value
                ))
            );

            setDevices(uniqueOptions);
        }

        getDevices();

        return () => {
            isMounted = false;
        };
    }, [t]);

    // Enumerate system audio devices
    useEffect(() => {
        let isMounted = true;

        async function getSystemDevices() {
            try {
                const devs = await invoke<AudioDevice[]>('get_system_audio_devices');
                if (isMounted) {
                    const options = [
                        { label: t('settings.mic_auto'), value: 'default' },
                        ...devs.map(d => ({
                            label: d.name,
                            value: d.name
                        }))
                    ];
                    setSystemDevices(options);
                }
            } catch (err) {
                console.error('Error getting system audio devices:', err);
            }
        }

        getSystemDevices();

        return () => {
            isMounted = false;
        };
    }, [t]);

    // System Audio Visualizer Logic
    useEffect(() => {
        let isMounted = true;

        async function startSystemVisualizer() {
            try {
                // Initialize Audio Context
                if (!systemAudioContextRef.current || systemAudioContextRef.current.state === 'closed') {
                    systemAudioContextRef.current = new AudioContext({ sampleRate: 16000 });
                } else if (systemAudioContextRef.current.state === 'suspended') {
                    await systemAudioContextRef.current.resume();
                }

                // Initialize Analyser
                if (!systemAnalyserRef.current && systemAudioContextRef.current) {
                    systemAnalyserRef.current = systemAudioContextRef.current.createAnalyser();
                    systemAnalyserRef.current.fftSize = 2048;
                    // Connect to mute gain to keep graph active
                    const gainNode = systemAudioContextRef.current.createGain();
                    gainNode.gain.value = 0;
                    systemAnalyserRef.current.connect(gainNode);
                    gainNode.connect(systemAudioContextRef.current.destination);
                }

                if (systemAudioContextRef.current) {
                    nextAudioTimeRef.current = systemAudioContextRef.current.currentTime;
                }

                // If no active session, we start the capture
                if (!isActiveSession) {
                    await invoke('start_system_audio_capture', {
                        deviceName: systemAudioDeviceId === 'default' ? null : systemAudioDeviceId
                    });
                }

                const unlisten = await listen<number[]>('system-audio', (event) => {
                    if (!isMounted || !systemAudioContextRef.current || !systemAnalyserRef.current) return;

                    const samples = new Int16Array(event.payload);
                    const float32Data = new Float32Array(samples.length);
                    for (let i = 0; i < samples.length; i++) {
                        const float = samples[i] < 0 ? samples[i] / 0x8000 : samples[i] / 0x7FFF;
                        float32Data[i] = float;
                    }

                    const buffer = systemAudioContextRef.current.createBuffer(1, samples.length, 16000);
                    buffer.copyToChannel(float32Data, 0);

                    const source = systemAudioContextRef.current.createBufferSource();
                    source.buffer = buffer;
                    source.connect(systemAnalyserRef.current);

                    const currentTime = systemAudioContextRef.current.currentTime;
                    let startTime = nextAudioTimeRef.current;
                    if (startTime < currentTime) {
                        startTime = currentTime;
                    }
                    source.start(startTime);
                    nextAudioTimeRef.current = startTime + buffer.duration;
                });

                systemUnlistenRef.current = unlisten;

                drawSystem();
            } catch (err) {
                console.error('Error starting system visualizer:', err);
            }
        }

        function drawSystem() {
            const canvas = systemCanvasRef.current;
            const analyser = systemAnalyserRef.current;

            if (!canvas || !analyser) return;

            const ctx = canvas.getContext('2d');
            if (!ctx) return;

            const dataArray = new Uint8Array(analyser.fftSize);

            const barCount = 20;
            const barGap = 2;
            const totalGap = (barCount - 1) * barGap;
            const barWidth = (canvas.width - totalGap) / barCount;

            const drawLoop = () => {
                systemAnimationRef.current = requestAnimationFrame(drawLoop);

                analyser.getByteTimeDomainData(dataArray);

                let sum = 0;
                for (let i = 0; i < dataArray.length; i++) {
                    const value = (dataArray[i] - 128) / 128.0;
                    sum += value * value;
                }
                const rms = Math.sqrt(sum / dataArray.length);
                const displayVolume = Math.min(1.0, rms * 5.0);

                ctx.clearRect(0, 0, canvas.width, canvas.height);

                const activeBars = Math.ceil(displayVolume * barCount);

                for (let i = 0; i < barCount; i++) {
                    const x = i * (barWidth + barGap);

                    let color = '#e5e7eb';

                    if (i < activeBars) {
                        const percent = i / barCount;
                        if (percent < 0.6) {
                            color = '#22c55e';
                        } else if (percent < 0.8) {
                            color = '#eab308';
                        } else {
                            color = '#ef4444';
                        }
                    }

                    ctx.fillStyle = color;
                    ctx.fillRect(x, 0, barWidth, canvas.height);
                }
            };

            drawLoop();
        }

        startSystemVisualizer();

        return () => {
            isMounted = false;

            if (systemAnimationRef.current) {
                cancelAnimationFrame(systemAnimationRef.current);
            }

            if (systemUnlistenRef.current) {
                systemUnlistenRef.current();
            }

            if (systemAudioContextRef.current) {
                systemAudioContextRef.current.close();
                systemAudioContextRef.current = null;
                systemAnalyserRef.current = null;
            }

            // Only stop capture if we started it (i.e., no active session was running)
            // However, this check uses the initial value of isActiveSession from closure.
            // But since this effect re-runs if systemAudioDeviceId changes, we need to be careful.
            // Actually, we can just check the store state via a ref or rely on the fact that
            // stop_system_audio_capture is safe to call? No, it kills the stream for everyone.

            // To be safe: We check the store via the prop or fresh check if possible.
            // But hooks can't access updated state in cleanup easily without refs.
            // We use isActiveSession from the scope. If it was false when we mounted, we stop it.
            if (!isActiveSession) {
                invoke('stop_system_audio_capture').catch(console.error);
            }
        };
    }, [systemAudioDeviceId, isActiveSession]); // Re-run if device changes or active session state changes


    // Mic Visualizer Logic
    useEffect(() => {
        let isMounted = true;
        // Start visualization for the selected microphone
        // This runs whenever microphoneId changes or component mounts

        startVisualizer(microphoneId, () => isMounted);

        return () => {
            isMounted = false;
            stopVisualizer();
        };
    }, [microphoneId]);

    async function startVisualizer(deviceId: string, checkMounted: () => boolean) {
        stopVisualizer(); // Stop previous if any

        try {
            const constraints: MediaStreamConstraints = {
                audio: deviceId === 'default'
                    ? true
                    : { deviceId: { exact: deviceId } }
            };

            const stream = await navigator.mediaDevices.getUserMedia(constraints);

            if (!checkMounted()) {
                stream.getTracks().forEach(t => t.stop());
                return;
            }

            streamRef.current = stream;

            const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
            const audioCtx = new AudioContextClass();
            audioContextRef.current = audioCtx;

            const analyser = audioCtx.createAnalyser();
            // Use a larger FFT size for better time-domain resolution if needed,
            // but for simple volume (RMS), 2048 is standard.
            analyser.fftSize = 2048;
            analyserRef.current = analyser;

            const source = audioCtx.createMediaStreamSource(stream);
            source.connect(analyser);
            sourceRef.current = source;

            draw();
        } catch (err) {
            console.error('Error starting visualizer:', err);
        }
    }

    function stopVisualizer() {
        if (animationRef.current) {
            cancelAnimationFrame(animationRef.current);
            animationRef.current = 0;
        }
        if (sourceRef.current) {
            sourceRef.current.disconnect();
            sourceRef.current = null;
        }
        if (analyserRef.current) {
            analyserRef.current.disconnect();
            analyserRef.current = null;
        }
        if (audioContextRef.current) {
            audioContextRef.current.close();
            audioContextRef.current = null;
        }
        if (streamRef.current) {
            streamRef.current.getTracks().forEach(t => t.stop());
            streamRef.current = null;
        }
    }

    function draw() {
        const canvas = canvasRef.current;
        const analyser = analyserRef.current;

        if (!canvas || !analyser) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const dataArray = new Uint8Array(analyser.fftSize); // Use full time domain data buffer

        const barCount = 20; // Number of LED bars
        const barGap = 2;
        const totalGap = (barCount - 1) * barGap;
        const barWidth = (canvas.width - totalGap) / barCount;

        const drawLoop = () => {
            animationRef.current = requestAnimationFrame(drawLoop);

            // Get time-domain data for waveform/volume calculation
            analyser.getByteTimeDomainData(dataArray);

            // Calculate RMS
            let sum = 0;
            for (let i = 0; i < dataArray.length; i++) {
                // Convert 8-bit unsigned (0-255) to centered -1 to 1 float
                const value = (dataArray[i] - 128) / 128.0;
                sum += value * value;
            }
            const rms = Math.sqrt(sum / dataArray.length);

            // Normalize/Boost volume for display
            // Typical speech RMS is often low, so we apply some gain or log scale
            // Let's use a simple multiplier for sensitivity
            const displayVolume = Math.min(1.0, rms * 5.0);

            ctx.clearRect(0, 0, canvas.width, canvas.height);

            const activeBars = Math.ceil(displayVolume * barCount);

            for (let i = 0; i < barCount; i++) {
                const x = i * (barWidth + barGap);

                // Color logic
                let color = '#e5e7eb'; // Default gray (off)

                if (i < activeBars) {
                    // Color based on index position (0-100% of range)
                    const percent = i / barCount;
                    if (percent < 0.6) {
                        color = '#22c55e'; // Green
                    } else if (percent < 0.8) {
                        color = '#eab308'; // Yellow
                    } else {
                        color = '#ef4444'; // Red
                    }
                }

                ctx.fillStyle = color;
                ctx.fillRect(x, 0, barWidth, canvas.height);
            }
        };

        drawLoop();
    }


    return (
        <div
            className="settings-group"
            role="tabpanel"
            id="settings-panel-microphone"
            aria-labelledby="settings-tab-microphone"
            tabIndex={0}
        >
            <div className="settings-item">
                <label htmlFor="settings-mic-select" className="settings-label">
                    {t('settings.microphone_selection')}
                </label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
                    <div style={{ flex: 1, maxWidth: 300 }}>
                        <Dropdown
                            id="settings-mic-select"
                            value={microphoneId}
                            onChange={setMicrophoneId}
                            options={devices}
                        />
                    </div>

                    <div style={{
                        width: '120px',
                        height: '36px',
                        backgroundColor: 'var(--color-bg-secondary)',
                        borderRadius: '4px',
                        overflow: 'hidden',
                        border: '1px solid var(--color-border)'
                    }}>
                        <canvas
                            ref={canvasRef}
                            width={120}
                            height={36}
                            style={{ display: 'block', width: '100%', height: '100%' }}
                        />
                    </div>
                </div>
                <div className="settings-hint">
                    {t('settings.mic_auto_hint', { defaultValue: 'Select which microphone to use for recording.' })}
                </div>
            </div>

            <div className="settings-item">
                <label htmlFor="settings-system-audio-select" className="settings-label">
                    {t('settings.system_audio_selection', { defaultValue: 'System Audio Selection' })}
                </label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
                    <div style={{ flex: 1, maxWidth: 300 }}>
                        <Dropdown
                            id="settings-system-audio-select"
                            value={systemAudioDeviceId}
                            onChange={setSystemAudioDeviceId}
                            options={systemDevices}
                        />
                    </div>
                    <div style={{
                        width: '120px',
                        height: '36px',
                        backgroundColor: 'var(--color-bg-secondary)',
                        borderRadius: '4px',
                        overflow: 'hidden',
                        border: '1px solid var(--color-border)'
                    }}>
                        <canvas
                            ref={systemCanvasRef}
                            width={120}
                            height={36}
                            style={{ display: 'block', width: '100%', height: '100%' }}
                        />
                    </div>
                </div>
                <div className="settings-hint">
                    {t('settings.system_audio_hint', { defaultValue: 'Select the system audio device for capture.' })}
                </div>
            </div>

            <div className="settings-item with-divider">
                <div className="settings-item-row">
                    <div>
                        <div className="settings-label" style={{ marginBottom: 0 }}>
                            {t('settings.mute_during_recording', { defaultValue: 'Mute during recording' })}
                        </div>
                        <div className="settings-hint">
                            {t('settings.mute_during_recording_hint', { defaultValue: 'Automatically mute the system speaker during recording. This stops the microphone from recording system sounds.' })}
                        </div>
                    </div>
                    <Switch
                        checked={muteDuringRecording}
                        onChange={setMuteDuringRecording}
                    />
                </div>
            </div>

        </div>
    );
}
