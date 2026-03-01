import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { AppConfig } from '../../types/transcript';

interface AudioDevice {
    name: string;
}

interface SettingsMicrophoneTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
}

export function SettingsMicrophoneTab({
    config,
    updateConfig
}: SettingsMicrophoneTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [systemDevices, setSystemDevices] = useState<{ label: string; value: string }[]>([]);

    const microphoneId = config.microphoneId || 'default';
    const systemAudioDeviceId = config.systemAudioDeviceId || 'default';
    const muteDuringRecording = config.muteDuringRecording || false;

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const systemCanvasRef = useRef<HTMLCanvasElement>(null);
    const animationRef = useRef<number>(0);
    const systemAnimationRef = useRef<number>(0);
    const nativeUnlistenRef = useRef<UnlistenFn | null>(null);
    const systemUnlistenRef = useRef<UnlistenFn | null>(null);
    const usingNativeMicRef = useRef<boolean>(false);
    const startedMicCaptureRef = useRef<boolean>(false);
    const startedSystemCaptureRef = useRef<boolean>(false);
    const micTargetPeakRef = useRef(0);
    const micAmplitudeRef = useRef(0);
    const micPhaseRef = useRef(0);
    const systemTargetPeakRef = useRef(0);
    const systemAmplitudeRef = useRef(0);
    const systemPhaseRef = useRef(0);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);

    // We only control the system capture if it's not already running for recording/captioning
    const isActiveSession = isRecording || isCaptionMode;

    // Enumerate devices
    useEffect(() => {
        let isMounted = true;

        async function getDevices() {
            // Attempt Native first
            try {
                const devs = await invoke<AudioDevice[]>('get_microphone_devices');
                if (isMounted && devs && devs.length > 0) {
                    const options = [
                        { label: t('settings.mic_auto'), value: 'default' },
                        ...devs.map(d => ({
                            label: d.name,
                            value: d.name
                        }))
                    ];
                    setDevices(options);
                    return; // Successfully loaded native devices
                }
            } catch (err) {
                console.warn('Native get_microphone_devices failed, falling back to Web API:', err);
            }

            // Fallback to Web API
            try {
                const devs = await navigator.mediaDevices.enumerateDevices();
                const audioInputs = devs.filter(d => d.kind === 'audioinput');

                const hasLabels = audioInputs.some(d => d.label.length > 0);
                if (!hasLabels) {
                    try {
                        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
                        stream.getTracks().forEach(t => t.stop());
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

    const startWaveAnimation = (
        canvasRef: React.RefObject<HTMLCanvasElement | null>,
        frameRef: React.MutableRefObject<number>,
        targetPeakRef: React.MutableRefObject<number>,
        amplitudeRef: React.MutableRefObject<number>,
        phaseRef: React.MutableRefObject<number>
    ) => {
        const canvas = canvasRef.current;
        if (!canvas) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const drawLoop = () => {
            frameRef.current = requestAnimationFrame(drawLoop);
            amplitudeRef.current += (targetPeakRef.current - amplitudeRef.current) * 0.08;
            phaseRef.current += 0.06;

            const { width, height } = canvas;
            const centerY = height / 2;
            const maxWaveHeight = height * 0.42;
            const amplitudePx = Math.max(0.02, amplitudeRef.current) * maxWaveHeight;

            ctx.clearRect(0, 0, width, height);
            ctx.lineWidth = 2;
            ctx.strokeStyle = '#4b5563';
            ctx.beginPath();

            const cycles = 2.3;
            for (let x = 0; x <= width; x += 2) {
                const t = x / width;
                const y =
                    centerY +
                    Math.sin((t * cycles * Math.PI * 2) + phaseRef.current) * amplitudePx +
                    Math.sin((t * (cycles * 0.55) * Math.PI * 2) + phaseRef.current * 1.45) * (amplitudePx * 0.35);
                if (x === 0) {
                    ctx.moveTo(x, y);
                } else {
                    ctx.lineTo(x, y);
                }
            }
            ctx.stroke();
        };

        drawLoop();
    };

    const stopWaveAnimation = (
        canvasRef: React.RefObject<HTMLCanvasElement | null>,
        frameRef: React.MutableRefObject<number>,
        amplitudeRef: React.MutableRefObject<number>
    ) => {
        if (frameRef.current) {
            cancelAnimationFrame(frameRef.current);
            frameRef.current = 0;
        }
        amplitudeRef.current = 0;
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (canvas && ctx) {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
        }
    };

    // System Audio Visualizer Logic
    useEffect(() => {
        let isMounted = true;

        async function startSystemVisualizer() {
            try {
                if (!isActiveSession) {
                    await invoke('start_system_audio_capture', {
                        deviceName: systemAudioDeviceId === 'default' ? null : systemAudioDeviceId,
                        instanceId: 'test_system'
                    });
                    startedSystemCaptureRef.current = true;
                } else {
                    startedSystemCaptureRef.current = false;
                }

                const unlisten = await listen<number>('system-audio', (event) => {
                    if (!isMounted) return;
                    systemTargetPeakRef.current = Math.min(1, Math.abs(event.payload) / 32767);
                });

                systemUnlistenRef.current = unlisten;
                startWaveAnimation(
                    systemCanvasRef,
                    systemAnimationRef,
                    systemTargetPeakRef,
                    systemAmplitudeRef,
                    systemPhaseRef
                );
            } catch (err) {
                console.error('Error starting system visualizer:', err);
            }
        }

        startSystemVisualizer();

        return () => {
            isMounted = false;
            stopWaveAnimation(systemCanvasRef, systemAnimationRef, systemAmplitudeRef);

            if (systemUnlistenRef.current) {
                systemUnlistenRef.current();
                systemUnlistenRef.current = null;
            }

            if (startedSystemCaptureRef.current) {
                invoke('stop_system_audio_capture').catch(console.error);
                startedSystemCaptureRef.current = false;
            }
        };
    }, [systemAudioDeviceId, isActiveSession]);

    // Mic Visualizer Logic
    useEffect(() => {
        let isMounted = true;
        startVisualizer(microphoneId, () => isMounted);

        return () => {
            isMounted = false;
            stopVisualizer();
        };
    }, [microphoneId, isActiveSession]);

    async function startVisualizer(deviceId: string, checkMounted: () => boolean) {
        stopVisualizer();
        try {
            if (!isActiveSession) {
                await invoke('start_microphone_capture', {
                    deviceName: deviceId === 'default' ? null : deviceId,
                    instanceId: 'test_mic'
                });
                startedMicCaptureRef.current = true;
            } else {
                startedMicCaptureRef.current = false;
            }

            const unlisten = await listen<number>('microphone-audio', (event) => {
                if (!checkMounted()) return;
                micTargetPeakRef.current = Math.min(1, Math.abs(event.payload) / 32767);
            });

            nativeUnlistenRef.current = unlisten;
            usingNativeMicRef.current = true;

            startWaveAnimation(
                canvasRef,
                animationRef,
                micTargetPeakRef,
                micAmplitudeRef,
                micPhaseRef
            );
        } catch (err) {
            console.warn('Native microphone visualizer failed:', err);
            stopWaveAnimation(canvasRef, animationRef, micAmplitudeRef);
        }
    }

    function stopVisualizer() {
        stopWaveAnimation(canvasRef, animationRef, micAmplitudeRef);

        if (nativeUnlistenRef.current) {
            nativeUnlistenRef.current();
            nativeUnlistenRef.current = null;
        }
        if (usingNativeMicRef.current && startedMicCaptureRef.current) {
            invoke('stop_microphone_capture').catch(console.error);
        }
        usingNativeMicRef.current = false;
        startedMicCaptureRef.current = false;
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
                            onChange={(val) => updateConfig({ microphoneId: val })}
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
                            onChange={(val) => updateConfig({ systemAudioDeviceId: val })}
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
                        onChange={(enabled) => updateConfig({ muteDuringRecording: enabled })}
                    />
                </div>
            </div>

        </div>
    );
}
