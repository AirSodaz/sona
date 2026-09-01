import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Volume2, SlidersHorizontal, FileAudio } from 'lucide-react';
import { MicIcon } from '../Icons';
import { Dropdown } from '../Dropdown';
import { Switch } from '../Switch';
import { useAudioConfig, useSetConfig, useVoiceTypingConfig } from '../../stores/configStore';
import { useAudioVisualizer } from '../../hooks/useAudioVisualizer';
import {
    listMicrophoneDeviceOptions,
    listSystemAudioDeviceOptions,
} from '../../services/audioDeviceService';
import {
    startMicrophoneCapture,
    startSystemAudioCapture,
    stopMicrophoneCapture,
    stopSystemAudioCapture,
} from '../../services/tauri/audio';
import { TauriEvent } from '../../services/tauri/events';
import { useTranscriptRuntimeStore } from '../../stores/transcriptRuntimeStore';
import { useVoiceTypingRuntimeStatus } from '../../stores/voiceTypingRuntimeStore';
import { SettingsTabContainer, SettingsSection, SettingsItem, SettingsPageHeader, SettingsLocationCard } from './SettingsLayout';
import { logger } from '../../utils/logger';
import { listen, type UnlistenFn } from '../../services/tauri/platform/events';
import { remove } from '../../services/tauri/platform/fs';
import { openDialog } from '../../services/tauri/platform/dialog';
import { storageOpenPath } from '../../services/tauri/storage';
import { getRuntimeEnvironmentStatus, getPathStatuses } from '../../services/tauri/app';
import type { RuntimeEnvironmentStatus } from '../../types/runtime';
import './SettingsShared.css';

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
    const voiceTypingConfig = useVoiceTypingConfig();
    const updateConfig = useSetConfig();
    const [devices, setDevices] = useState<{ label: string; value: string }[]>([]);
    const [systemDevices, setSystemDevices] = useState<{ label: string; value: string }[]>([]);
    const [areMicrophoneDevicesLoaded, setAreMicrophoneDevicesLoaded] = useState(false);
    const [areSystemDevicesLoaded, setAreSystemDevicesLoaded] = useState(false);

    const microphoneId = config.microphoneId || 'default';
    const microphoneBoost = config.microphoneBoost ?? 1.0;
    const systemAudioDeviceId = config.systemAudioDeviceId || 'default';
    const muteDuringRecording = config.muteDuringRecording || false;
    const keepMicrophoneActive = config.keepMicrophoneActive ?? false;
    const voiceTypingEnabled = voiceTypingConfig.voiceTypingEnabled ?? false;
    const voiceTypingRuntime = useVoiceTypingRuntimeStatus();

    const ffmpegPath = config.ffmpegPath || '';
    const isCustomFfmpeg = Boolean(ffmpegPath.trim());
    const [runtimeEnv, setRuntimeEnv] = useState<RuntimeEnvironmentStatus | null>(null);
    const [isFfmpegValid, setIsFfmpegValid] = useState<boolean | null>(null);
    const [isFfmpegBusy, setIsFfmpegBusy] = useState(false);

    const displayFfmpegPath = isCustomFfmpeg ? ffmpegPath : (runtimeEnv?.ffmpegPath || '');

    useEffect(() => {
        let isMounted = true;
        const checkFfmpegStatus = async () => {
            try {
                const env = await getRuntimeEnvironmentStatus();
                if (!isMounted) return;
                setRuntimeEnv(env);

                if (isCustomFfmpeg) {
                    const statuses = await getPathStatuses([ffmpegPath.trim()]);
                    if (!isMounted) return;
                    setIsFfmpegValid(statuses[0]?.kind === 'file');
                } else {
                    setIsFfmpegValid(env.ffmpegExists);
                }
            } catch (err) {
                logger.warn('Failed to check FFmpeg runtime status:', err);
                if (isMounted) {
                    setIsFfmpegValid(false);
                }
            }
        };

        void checkFfmpegStatus();

        return () => {
            isMounted = false;
        };
    }, [ffmpegPath, isCustomFfmpeg]);

    const handleBrowseFfmpeg = async () => {
        try {
            setIsFfmpegBusy(true);
            const selected = await openDialog({
                multiple: false,
                directory: false,
                filters: [
                    {
                        name: 'FFmpeg Executable',
                        extensions: ['exe', '*'],
                    },
                ],
            });
            if (selected && typeof selected === 'string') {
                updateConfig({ ffmpegPath: selected });
            }
        } catch (err) {
            logger.error('Failed to select FFmpeg path:', err);
        } finally {
            setIsFfmpegBusy(false);
        }
    };

    const handleResetFfmpeg = () => {
        updateConfig({ ffmpegPath: '' });
    };

    const handleOpenFfmpegFolder = async () => {
        if (!displayFfmpegPath) return;
        try {
            await storageOpenPath(displayFfmpegPath);
        } catch (err) {
            logger.error('Failed to open FFmpeg folder:', err);
        }
    };

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

    const isRecording = useTranscriptRuntimeStore((state) => state.isRecording);
    const isCaptionMode = useTranscriptRuntimeStore((state) => state.isCaptionMode);

    // We only control the system capture if it's not already running for recording/captioning
    const isActiveSession = isRecording || isCaptionMode;
    const canReusePersistentVoiceTypingMic =
        keepMicrophoneActive && voiceTypingEnabled && voiceTypingRuntime.warmup === 'ready';
    const arePreviewDependenciesReady = areMicrophoneDevicesLoaded && areSystemDevicesLoaded;
    const shouldRunActiveEffects = isOpen && isActiveTab;

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

    type PreviewCaptureKind = 'microphone' | 'system';

    const stopPreviewCapture = useCallback(async (
        kind: PreviewCaptureKind,
        instanceId: 'test_mic' | 'test_system'
    ) => {
        const path = kind === 'microphone'
            ? await stopMicrophoneCapture(instanceId)
            : await stopSystemAudioCapture(instanceId);
        if (path) {
            await remove(path).catch(logger.error);
        }
    }, []);

    const clearQueuedSystemPreviewFrame = useCallback(() => {
        if (systemPreviewFrameRef.current === null) {
            return;
        }

        cancelAnimationFrame(systemPreviewFrameRef.current);
        systemPreviewFrameRef.current = null;
    }, []);

    const waitForNextSystemPreviewFrame = useCallback(() => {
        return new Promise<void>((resolve) => {
            systemPreviewFrameRef.current = requestAnimationFrame(() => {
                systemPreviewFrameRef.current = null;
                resolve();
            });
        });
    }, []);

    // Enumerate devices
    useEffect(() => {
        if (!shouldRunActiveEffects) {
            return;
        }

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
    }, [shouldRunActiveEffects, t]);

    // Enumerate system audio devices
    useEffect(() => {
        if (!shouldRunActiveEffects) {
            return;
        }

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
    }, [shouldRunActiveEffects, t]);

    const startMicrophonePreview = useCallback(async (deviceId: string, isCurrentRequest: () => boolean) => {
        if (!isCurrentRequest()) {
            return;
        }

        usingNativeMicRef.current = false;
        startedMicCaptureRef.current = false;

        try {
            let captureStarted = false;

            if (!isActiveSession && !canReusePersistentVoiceTypingMic) {
                await startMicrophoneCapture({
                    deviceName: deviceId === 'default' ? null : deviceId,
                    instanceId: 'test_mic',
                });
                captureStarted = true;
            }

            if (!isCurrentRequest()) {
                if (captureStarted) {
                    await stopPreviewCapture('microphone', 'test_mic');
                }
                return;
            }

            const unlisten = await listen<number>(TauriEvent.audio.microphonePeak, (event) => {
                if (!isCurrentRequest()) return;
                micTargetPeakRef.current = Math.min(
                    1,
                    (Math.abs(event.payload) / 32767) * microphoneBoost,
                );
            });

            if (!isCurrentRequest()) {
                unlisten();
                if (captureStarted) {
                    await stopPreviewCapture('microphone', 'test_mic');
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
    }, [
        canReusePersistentVoiceTypingMic,
        isActiveSession,
        microphoneBoost,
        startMicWaveAnimation,
        stopMicWaveAnimation,
        stopPreviewCapture,
    ]);

    const startSystemPreview = useCallback(async (deviceId: string, isCurrentRequest: () => boolean) => {
        if (!isCurrentRequest()) {
            return;
        }

        startedSystemCaptureRef.current = false;

        try {
            let captureStarted = false;

            if (!isActiveSession) {
                await startSystemAudioCapture({
                    deviceName: deviceId === 'default' ? null : deviceId,
                    instanceId: 'test_system',
                });
                captureStarted = true;
            }

            if (!isCurrentRequest()) {
                if (captureStarted) {
                    await stopPreviewCapture('system', 'test_system');
                }
                return;
            }

            const unlisten = await listen<number>(TauriEvent.audio.systemPeak, (event) => {
                if (!isCurrentRequest()) return;
                systemTargetPeakRef.current = Math.min(1, Math.abs(event.payload) / 32767);
            });

            if (!isCurrentRequest()) {
                unlisten();
                if (captureStarted) {
                    await stopPreviewCapture('system', 'test_system');
                }
                return;
            }

            systemUnlistenRef.current = unlisten;
            startedSystemCaptureRef.current = captureStarted;
            startSystemWaveAnimation();
        } catch (err) {
            logger.error('Error starting system visualizer:', err);
        }
    }, [isActiveSession, startSystemWaveAnimation, stopPreviewCapture]);

    const stopMicrophonePreview = useCallback(() => {
        stopMicWaveAnimation();

        if (nativeUnlistenRef.current) {
            nativeUnlistenRef.current();
            nativeUnlistenRef.current = null;
        }
        if (usingNativeMicRef.current && startedMicCaptureRef.current) {
            void stopPreviewCapture('microphone', 'test_mic').catch(logger.error);
        }
        usingNativeMicRef.current = false;
        startedMicCaptureRef.current = false;
    }, [stopMicWaveAnimation, stopPreviewCapture]);

    const stopSystemPreview = useCallback(() => {
        clearQueuedSystemPreviewFrame();
        stopSystemWaveAnimation();

        if (systemUnlistenRef.current) {
            systemUnlistenRef.current();
            systemUnlistenRef.current = null;
        }

        if (startedSystemCaptureRef.current) {
            void stopPreviewCapture('system', 'test_system').catch(logger.error);
        }
        startedSystemCaptureRef.current = false;
    }, [clearQueuedSystemPreviewFrame, stopPreviewCapture, stopSystemWaveAnimation]);

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

        if (shouldRunActiveEffects && arePreviewDependenciesReady) {
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
        canReusePersistentVoiceTypingMic,
        isActiveSession,
        isActiveTab,
        microphoneId,
        shouldRunActiveEffects,
        startMicrophonePreview,
        startSystemPreview,
        stopMicrophonePreview,
        stopSystemPreview,
        systemAudioDeviceId,
        waitForNextSystemPreviewFrame,
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

                <SettingsItem
                    title={t('settings.keep_microphone_active')}
                    hint={t('settings.keep_microphone_active_hint')}
                >
                    <Switch
                        checked={keepMicrophoneActive}
                        aria-label={t('settings.keep_microphone_active')}
                        onChange={(enabled) => updateConfig({ keepMicrophoneActive: enabled })}
                    />
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

            <SettingsSection
                title={t('settings.ffmpeg_title', '音频解码工具 (FFmpeg)')}
                icon={<FileAudio size={20} />}
                description={t(
                    'settings.ffmpeg_description',
                    '用于批量导入音视频文件时进行格式解封装与音频重采样，以及提取说话人音色样本。默认使用内置 FFmpeg。'
                )}
            >
                <SettingsLocationCard
                    testId="settings-microphone-ffmpeg-card"
                    title={t('settings.ffmpeg_path_title', { defaultValue: 'FFmpeg Executable Path' })}
                    hint={t('settings.ffmpeg_path_hint', {
                        defaultValue: 'Specify a local FFmpeg executable on your system. Leave empty to use the bundled version.',
                    })}
                    path={displayFfmpegPath}
                    isCustom={isCustomFfmpeg}
                    isValid={isFfmpegValid}
                    isBusy={isFfmpegBusy}
                    changeLabel={t('common.change_path', { defaultValue: 'Change Path...' })}
                    onChangePath={handleBrowseFfmpeg}
                    openFolderLabel={t('common.open_folder', { defaultValue: 'Open Folder' })}
                    onOpenFolder={handleOpenFfmpegFolder}
                    restoreDefaultLabel={t('common.restore_default', { defaultValue: 'Restore Default' })}
                    onRestoreDefault={isCustomFfmpeg ? handleResetFfmpeg : undefined}
                    bottomHint={
                        isFfmpegValid
                            ? t('settings.ffmpeg_hint_ready', { defaultValue: 'A valid FFmpeg executable is detected and ready.' })
                            : t('settings.ffmpeg_hint_missing', { defaultValue: 'No valid FFmpeg executable found. Media decoding and batch imports may fail.' })
                    }
                    bottomHintColor={isFfmpegValid ? undefined : 'var(--color-danger, #ef4444)'}
                />
            </SettingsSection>
        </SettingsTabContainer>
    );
}

export default SettingsMicrophoneTab;
