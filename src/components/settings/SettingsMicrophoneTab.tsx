import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Volume2, SlidersHorizontal } from 'lucide-react';
import { MicIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { useAudioConfig, useSetConfig } from '../../stores/configStore';
import { useAudioVisualizer } from '../../hooks/useAudioVisualizer';
import { remove } from '@tauri-apps/plugin-fs';
import {
    listMicrophoneDeviceOptions,
    listSystemAudioDeviceOptions,
} from '../../services/audioDeviceService';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader } from './SettingsLayout';
import { logger } from '../../utils/logger';

interface SettingsMicrophoneTabProps {
    isActiveTab?: boolean;
    isOpen?: boolean;
}

export function SettingsMicrophoneTab({
    isActiveTab = true,
    isOpen = true
}: SettingsMicrophoneTabProps): React.JSX.Element {
    const { t } = useTranslation();
    const config = useAudioConfig();
    const updateConfig = useSetConfig();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [systemDevices, setSystemDevices] = useState<{ label: string; value: string }[]>([]);
    const [areMicrophoneDevicesLoaded, setAreMicrophoneDevicesLoaded] = useState(false);
    const [areSystemDevicesLoaded, setAreSystemDevicesLoaded] = useState(false);

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
    const micPreviewRequestIdRef = useRef(0);
    const systemPreviewRequestIdRef = useRef(0);
    const systemPreviewFrameRef = useRef<number | null>(null);
    const micTargetPeakRef = useRef(0);
    const systemTargetPeakRef = useRef(0);

    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isCaptionMode = useTranscriptStore((state) => state.isCaptionMode);

    // We only control the system capture if it's not already running for recording/captioning
    const isActiveSession = isRecording || isCaptionMode;
    const arePreviewDependenciesReady = areMicrophoneDevicesLoaded && areSystemDevicesLoaded;

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

    async function stopPreviewCapture(command: 'stop_microphone_capture' | 'stop_system_audio_capture', instanceId: 'test_mic' | 'test_system') {
        const path = await invoke<string>(command, { instanceId });
        if (path) {
            await remove(path).catch(logger.error);
        }
    }

    function clearQueuedSystemPreviewFrame() {
        if (systemPreviewFrameRef.current === null) {
            return;
        }

        cancelAnimationFrame(systemPreviewFrameRef.current);
        systemPreviewFrameRef.current = null;
    }

    function waitForNextSystemPreviewFrame() {
        return new Promise<void>((resolve) => {
            systemPreviewFrameRef.current = requestAnimationFrame(() => {
                systemPreviewFrameRef.current = null;
                resolve();
            });
        });
    }

    // Enumerate devices
    useEffect(() => {
        let isMounted = true;
        queueMicrotask(() => {
            if (isMounted) {
                setAreMicrophoneDevicesLoaded(false);
            }
        });

        async function getDevices() {
            try {
                const options = await listMicrophoneDeviceOptions(t('settings.mic_auto'));
                if (isMounted) {
                    setDevices(options);
                }
            } catch (err) {
                logger.error('Error enumerating devices:', err);
            } finally {
                if (isMounted) {
                    setAreMicrophoneDevicesLoaded(true);
                }
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
        queueMicrotask(() => {
            if (isMounted) {
                setAreSystemDevicesLoaded(false);
            }
        });

        async function getSystemDevices() {
            try {
                const devs = await listSystemAudioDeviceOptions(t('settings.mic_auto'));
                if (isMounted) {
                    setSystemDevices(devs);
                }
            } catch (err) {
                logger.error('Error getting system audio devices:', err);
            } finally {
                if (isMounted) {
                    setAreSystemDevicesLoaded(true);
                }
            }
        }

        getSystemDevices();

        return () => {
            isMounted = false;
        };
    }, [t]);

    // Sync Microphone Boost to Rust backend
    useEffect(() => {
        invoke('set_microphone_boost', { boost: microphoneBoost }).catch(logger.error);
    }, [microphoneBoost]);

    async function startMicrophonePreview(deviceId: string, isCurrentRequest: () => boolean) {
        if (!isCurrentRequest()) {
            return;
        }

        usingNativeMicRef.current = false;
        startedMicCaptureRef.current = false;

        try {
            let captureStarted = false;

            if (!isActiveSession) {
                await invoke('start_microphone_capture', {
                    deviceName: deviceId === 'default' ? null : deviceId,
                    instanceId: 'test_mic'
                });
                captureStarted = true;
            }

            if (!isCurrentRequest()) {
                if (captureStarted) {
                    await stopPreviewCapture('stop_microphone_capture', 'test_mic');
                }
                return;
            }

            const unlisten = await listen<number>('microphone-audio', (event) => {
                if (!isCurrentRequest()) return;
                micTargetPeakRef.current = Math.min(1, Math.abs(event.payload) / 32767);
            });

            if (!isCurrentRequest()) {
                unlisten();
                if (captureStarted) {
                    await stopPreviewCapture('stop_microphone_capture', 'test_mic');
                }
                return;
            }

            nativeUnlistenRef.current = unlisten;
            usingNativeMicRef.current = true;
            startedMicCaptureRef.current = captureStarted;

            startMicWaveAnimation();
        } catch (err) {
            logger.warn('Native microphone visualizer failed:', err);
            stopMicWaveAnimation();
        }
    }

    async function startSystemPreview(deviceId: string, isCurrentRequest: () => boolean) {
        if (!isCurrentRequest()) {
            return;
        }

        startedSystemCaptureRef.current = false;

        try {
            let captureStarted = false;

            if (!isActiveSession) {
                await invoke('start_system_audio_capture', {
                    deviceName: deviceId === 'default' ? null : deviceId,
                    instanceId: 'test_system'
                });
                captureStarted = true;
            }

            if (!isCurrentRequest()) {
                if (captureStarted) {
                    await stopPreviewCapture('stop_system_audio_capture', 'test_system');
                }
                return;
            }

            const unlisten = await listen<number>('system-audio', (event) => {
                if (!isCurrentRequest()) return;
                systemTargetPeakRef.current = Math.min(1, Math.abs(event.payload) / 32767);
            });

            if (!isCurrentRequest()) {
                unlisten();
                if (captureStarted) {
                    await stopPreviewCapture('stop_system_audio_capture', 'test_system');
                }
                return;
            }

            systemUnlistenRef.current = unlisten;
            startedSystemCaptureRef.current = captureStarted;
            startSystemWaveAnimation();
        } catch (err) {
            logger.error('Error starting system visualizer:', err);
        }
    }

    function stopMicrophonePreview() {
        stopMicWaveAnimation();

        if (nativeUnlistenRef.current) {
            nativeUnlistenRef.current();
            nativeUnlistenRef.current = null;
        }
        if (usingNativeMicRef.current && startedMicCaptureRef.current) {
            void stopPreviewCapture('stop_microphone_capture', 'test_mic').catch(logger.error);
        }
        usingNativeMicRef.current = false;
        startedMicCaptureRef.current = false;
    }

    function stopSystemPreview() {
        clearQueuedSystemPreviewFrame();
        stopSystemWaveAnimation();

        if (systemUnlistenRef.current) {
            systemUnlistenRef.current();
            systemUnlistenRef.current = null;
        }

        if (startedSystemCaptureRef.current) {
            void stopPreviewCapture('stop_system_audio_capture', 'test_system').catch(logger.error);
        }
        startedSystemCaptureRef.current = false;
    }

    useEffect(() => {
        let isMounted = true;
        const micRequestId = ++micPreviewRequestIdRef.current;
        const systemRequestId = ++systemPreviewRequestIdRef.current;

        const isCurrentMicRequest = () => isMounted && micRequestId === micPreviewRequestIdRef.current;
        const isCurrentSystemRequest = () => isMounted && systemRequestId === systemPreviewRequestIdRef.current;

        async function startPreviews() {
            await startMicrophonePreview(microphoneId, isCurrentMicRequest);

            if (!isCurrentMicRequest() || !isCurrentSystemRequest()) {
                return;
            }

            await waitForNextSystemPreviewFrame();

            if (!isCurrentMicRequest() || !isCurrentSystemRequest()) {
                return;
            }

            await startSystemPreview(systemAudioDeviceId, isCurrentSystemRequest);
        }

        if (isOpen && isActiveTab && arePreviewDependenciesReady) {
            void startPreviews();
        }

        return () => {
            isMounted = false;
            micPreviewRequestIdRef.current += 1;
            systemPreviewRequestIdRef.current += 1;
            stopMicrophonePreview();
            stopSystemPreview();
        };
    }, [
        arePreviewDependenciesReady,
        isActiveSession,
        isActiveTab,
        isOpen,
        microphoneId,
        systemAudioDeviceId,
    ]);

    return (
        <SettingsTabContainer id="settings-panel-microphone" ariaLabelledby="settings-tab-microphone">
            <SettingsPageHeader 
                icon={<MicIcon width={28} height={28} />}
                title={t('settings.input_device')} 
                description={t('settings.microphone_description')} 
            />
            <SettingsSection
                title={t('settings.microphone_title')}
                icon={<SlidersHorizontal size={20} />}
            >
                <SettingsItem
                    title={t('settings.microphone_selection')}
                    hint={t('settings.mic_auto_hint')}
                    layout="vertical"
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%', flexWrap: 'wrap' }}>
                        <div style={{ flex: '1 1 320px', minWidth: 0, maxWidth: 520 }}>
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
                            borderRadius: 'var(--radius-sm)',
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
                </SettingsItem>

                <SettingsItem
                    title={t('settings.microphone_boost')}
                    hint={t('settings.microphone_boost_hint')}
                >
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
                            className="settings-input"
                            style={{ width: '80px', textAlign: 'center' }}
                        />
                        <span className="settings-hint" style={{ marginTop: 0 }}>x</span>
                    </div>
                </SettingsItem>
            </SettingsSection>

            <SettingsSection
                title={t('settings.system_audio_title')}
                icon={<Volume2 size={20} />}
                description={t('settings.system_audio_description')}
            >
                <SettingsItem
                    title={t('settings.system_audio_selection')}
                    hint={t('settings.system_audio_hint')}
                    layout="vertical"
                >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%', flexWrap: 'wrap' }}>
                        <div style={{ flex: '1 1 320px', minWidth: 0, maxWidth: 520 }}>
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
                            borderRadius: 'var(--radius-sm)',
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
                </SettingsItem>

                <SettingsItem
                    title={t('settings.mute_during_recording')}
                    hint={t('settings.mute_during_recording_hint')}
                >
                    <Switch
                        checked={muteDuringRecording}
                        onChange={(enabled) => updateConfig({ muteDuringRecording: enabled })}
                    />
                </SettingsItem>
            </SettingsSection>
        </SettingsTabContainer>
    );
}
