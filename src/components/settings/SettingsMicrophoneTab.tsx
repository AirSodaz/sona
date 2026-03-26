import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { AppConfig } from '../../types/transcript';
import { useAudioVisualizer } from '../../hooks/useAudioVisualizer';
import { remove } from '@tauri-apps/plugin-fs';
import {
    listMicrophoneDeviceOptions,
    listSystemAudioDeviceOptions,
} from '../../services/audioDeviceService';

interface SettingsMicrophoneTabProps {
    config: AppConfig;
    updateConfig: (updates: Partial<AppConfig>) => void;
    isActiveTab?: boolean;
    isOpen?: boolean;
}

export function SettingsMicrophoneTab({
    config,
    updateConfig,
    isActiveTab = true,
    isOpen = true
}: SettingsMicrophoneTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [systemDevices, setSystemDevices] = useState<{ label: string; value: string }[]>([]);

    const microphoneId = config.microphoneId || 'default';
    const microphoneBoost = config.microphoneBoost ?? 1.0;
    const systemAudioDeviceId = config.systemAudioDeviceId || 'default';
    const muteDuringRecording = config.muteDuringRecording || false;

    const canvasRef = useRef<HTMLCanvasElement>(null);
    const systemCanvasRef = useRef<HTMLCanvasElement>(null);
    const nativeUnlistenRef = useRef<UnlistenFn | null>(null);
    const systemUnlistenRef = useRef<UnlistenFn | null>(null);
    const usingNativeMicRef = useRef<boolean>(false);
    const startedMicCaptureRef = useRef<boolean>(false);
    const startedSystemCaptureRef = useRef<boolean>(false);
    const micTargetPeakRef = useRef(0);
    const systemTargetPeakRef = useRef(0);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);

    // We only control the system capture if it's not already running for recording/captioning
    const isActiveSession = isRecording || isCaptionMode;

    const { startVisualizer: startMicWaveAnimation, stopVisualizer: stopMicWaveAnimation } = useAudioVisualizer({
        canvasRef,
        peakLevelRef: micTargetPeakRef,
        isPaused: false
    });

    const { startVisualizer: startSystemWaveAnimation, stopVisualizer: stopSystemWaveAnimation } = useAudioVisualizer({
        canvasRef: systemCanvasRef,
        peakLevelRef: systemTargetPeakRef,
        isPaused: false
    });

    // Enumerate devices
    useEffect(() => {
        let isMounted = true;

        async function getDevices() {
            try {
                const options = await listMicrophoneDeviceOptions(t('settings.mic_auto'));
                if (isMounted) {
                    setDevices(options);
                }
            } catch (err) {
                console.error('Error enumerating devices:', err);
            }
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
                const devs = await listSystemAudioDeviceOptions(t('settings.mic_auto'));
                if (isMounted) {
                    setSystemDevices(devs);
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
                startSystemWaveAnimation();
            } catch (err) {
                console.error('Error starting system visualizer:', err);
            }
        }

        if (isOpen && isActiveTab) {
            startSystemVisualizer();
        }

        return () => {
            isMounted = false;
            stopSystemWaveAnimation();

            if (systemUnlistenRef.current) {
                systemUnlistenRef.current();
                systemUnlistenRef.current = null;
            }

            if (startedSystemCaptureRef.current) {
                invoke<string>('stop_system_audio_capture', { instanceId: 'test_system' })
                    .then((path) => {
                        if (path) {
                            remove(path).catch(console.error);
                        }
                    })
                    .catch(console.error);
                startedSystemCaptureRef.current = false;
            }
        };
    }, [systemAudioDeviceId, isActiveSession, isOpen, isActiveTab]);

    // Sync Microphone Boost to Rust backend
    useEffect(() => {
        invoke('set_microphone_boost', { boost: microphoneBoost }).catch(console.error);
    }, [microphoneBoost]);

    // Mic Visualizer Logic
    useEffect(() => {
        let isMounted = true;

        if (isOpen && isActiveTab) {
            startVisualizer(microphoneId, () => isMounted);
        }

        return () => {
            isMounted = false;
            stopVisualizer();
        };
    }, [microphoneId, isActiveSession, isOpen, isActiveTab]);

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

            startMicWaveAnimation();
        } catch (err) {
            console.warn('Native microphone visualizer failed:', err);
            stopMicWaveAnimation();
        }
    }

    function stopVisualizer() {
        stopMicWaveAnimation();

        if (nativeUnlistenRef.current) {
            nativeUnlistenRef.current();
            nativeUnlistenRef.current = null;
        }
        if (usingNativeMicRef.current && startedMicCaptureRef.current) {
            invoke<string>('stop_microphone_capture')
                .then((path) => {
                    if (path) {
                        remove(path).catch(console.error);
                    }
                })
                .catch(console.error);
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
                            className="visualizer-canvas"
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
                            className="visualizer-canvas"
                            style={{ display: 'block', width: '100%', height: '100%' }}
                        />
                    </div>
                </div>
                <div className="settings-hint">
                    {t('settings.system_audio_hint', { defaultValue: 'Select the system audio device for capture.' })}
                </div>
            </div>

            <div className="settings-item">
                <label htmlFor="settings-mic-boost" className="settings-label">
                    {t('settings.microphone_boost', { defaultValue: 'Microphone Boost' })}
                </label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <input
                        id="settings-mic-boost"
                        type="number"
                        min="1.0"
                        max="5.0"
                        step="0.1"
                        value={microphoneBoost}
                        onChange={(e) => {
                            let val = parseFloat(e.target.value);
                            if (isNaN(val)) return;
                            val = Math.max(1.0, Math.min(5.0, val));
                            updateConfig({ microphoneBoost: val });
                        }}
                        className="sona-input"
                        style={{ width: '80px' }}
                    />
                    <span className="settings-hint" style={{ marginTop: 0 }}>x</span>
                </div>
                <div className="settings-hint">
                    {t('settings.microphone_boost_hint', { defaultValue: 'Amplify microphone volume (1.0 to 5.0). Useful for quiet microphones.' })}
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
