import { useEffect, useRef, useState, useCallback } from 'react';
import { captionTranscriptionService, TranscriptionService } from '../services/transcriptionService';
import { captionWindowService } from '../services/captionWindowService';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { remove } from '@tauri-apps/plugin-fs';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';

export function useCaptionSession(config: AppConfig, isCaptionMode: boolean) {
    const [isInitializing, setIsInitializing] = useState(false);

    // Refs to hold instances across renders
    const audioContextRef = useRef<AudioContext | null>(null);
    const streamRef = useRef<MediaStream | null>(null);
    const processorRef = useRef<AudioWorkletNode | null>(null);
    const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
    const activeServiceRef = useRef<TranscriptionService>(captionTranscriptionService);

    // Native capture refs
    const usingNativeCaptureRef = useRef(false);
    const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);

    // Track active state to handle race conditions
    const activeRef = useRef(isCaptionMode);

    useEffect(() => {
        activeRef.current = isCaptionMode;
    }, [isCaptionMode]);

    // Configuration is now updated globally via useTranscriptionServiceSync,
    // so we don't need updateServiceConfig here anymore.

    const stopCaptionSession = useCallback(async () => {
        logger.info('[CaptionSession] Stopping session...');

        // Close Window
        try {
            await captionWindowService.close();
        } catch (e) { logger.error('Error:', e); }

        // Stop Native Capture
        if (usingNativeCaptureRef.current) {
            try {
                const savedWavPath = await invoke<string>('stop_system_audio_capture', { instanceId: 'caption' });
                if (savedWavPath) {
                    logger.info('[CaptionSession] Deleting auto-saved native capture file:', savedWavPath);
                    try {
                        await remove(savedWavPath);
                    } catch (err) {
                        logger.error('[CaptionSession] Failed to delete native capture file:', err);
                    }
                }
            } catch (e) { logger.error('Error:', e); }
            usingNativeCaptureRef.current = false;
        }

        if (systemAudioUnlistenRef.current) {
            systemAudioUnlistenRef.current();
            systemAudioUnlistenRef.current = null;
        }

        // Stop Audio Context (Web API fallback)
        if (audioContextRef.current) {
            try {
                await audioContextRef.current.close();
            } catch (e) { logger.error('Error:', e); }
            audioContextRef.current = null;
        }

        // Stop Stream (Web API fallback)
        if (streamRef.current) {
            streamRef.current.getTracks().forEach(t => t.stop());
            streamRef.current = null;
        }

        // Stop Service
        await activeServiceRef.current.stop();
        activeServiceRef.current = captionTranscriptionService;

        processorRef.current = null;
        sourceRef.current = null;
        setIsInitializing(false);
    }, []);

    const startCaptionSession = useCallback(async () => {
        if (!config.streamingModelPath) {
            logger.warn('Cannot start caption: streaming model path is not set.');
            return;
        }

        if (
            (streamRef.current && audioContextRef.current && audioContextRef.current.state === 'running') ||
            (usingNativeCaptureRef.current)
        ) {
            // Already running
            return;
        }

        try {
            setIsInitializing(true);
            logger.info('[CaptionSession] Starting caption session...');

            if (!activeRef.current) return;

            const captionService = captionTranscriptionService;
            activeServiceRef.current = captionService;

            // 1. Get Audio Source (Try Native -> Fallback to Web API)
            if (!streamRef.current && !usingNativeCaptureRef.current) {
                let nativeSuccess = false;
                try {
                    logger.info('[CaptionSession] Attempting native system audio capture...');
                    await invoke('start_system_audio_capture', {
                        deviceName: config.systemAudioDeviceId === 'default' ? null : config.systemAudioDeviceId,
                        instanceId: 'caption'
                    });
                    const unlisten = await listen<number>('system-audio', () => {
                        // The Rust backend now feeds itself directly.
                    });
                    systemAudioUnlistenRef.current = unlisten;
                    usingNativeCaptureRef.current = true;
                    nativeSuccess = true;
                    logger.info('[CaptionSession] Native capture started.');
                } catch (e) {
                    logger.warn('[CaptionSession] Native capture failed, falling back to Web API:', e);
                }

                if (!nativeSuccess) {
                    // Fallback to Web API
                    let stream: MediaStream;
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

                        // Check active again after async
                        if (!activeRef.current) {
                            stream.getTracks().forEach(t => t.stop());
                            return;
                        }

                        const audioTracks = stream.getAudioTracks();
                        if (audioTracks.length === 0) {
                            throw new Error('No audio track selected in screen share.');
                        }

                        stream.getVideoTracks().forEach(t => t.stop());
                        stream = new MediaStream([audioTracks[0]]);
                        streamRef.current = stream;

                        stream.getAudioTracks()[0].onended = () => {
                            logger.info('[CaptionSession] Stream ended by user.');
                            stopCaptionSession();
                        };
                    } catch (err) {
                        if (!activeRef.current) return;
                        logger.error('[CaptionSession] Failed to get display media:', err);
                        throw err;
                    }
                }
            }

            if (!activeRef.current) return;

            // 2. Initialize Audio Context (ONLY for Web API fallback)
            if (!usingNativeCaptureRef.current) {
                if (!audioContextRef.current || audioContextRef.current.state === 'closed') {
                    const audioContext = new AudioContext({ sampleRate: 16000 });
                    audioContextRef.current = audioContext;
                    try {
                        await audioContext.audioWorklet.addModule('/audio-processor.js');
                    } catch (e) {
                        if (!activeRef.current) {
                            await audioContext.close();
                            audioContextRef.current = null;
                            return;
                        }
                        throw e;
                    }
                } else if (audioContextRef.current.state === 'suspended') {
                    await audioContextRef.current.resume();
                }

                if (!activeRef.current) {
                    if (audioContextRef.current) {
                        await audioContextRef.current.close();
                        audioContextRef.current = null;
                    }
                    return;
                }
            }

            // 3. Open Window early so the floating subtitle surface is visible
            // even if recognizer startup or the first VAD result is delayed.
            logger.info('[CaptionSession] Opening caption window...');
            await captionWindowService.open({
                alwaysOnTop: config.alwaysOnTop ?? true,
                lockWindow: config.lockWindow ?? false,
                width: config.captionWindowWidth,
                fontSize: config.captionFontSize,
                color: config.captionFontColor,
                backgroundOpacity: config.captionBackgroundOpacity
            });

            if (!activeRef.current) return;

            // 4. Start Service (Configuration is already handled globally)
            logger.info('[CaptionSession] Starting caption recognizer...');
            await captionService.start(
                (update: TranscriptUpdate | TranscriptSegment) => {
                    const normalizedUpdate = normalizeTranscriptUpdate(update);
                    captionWindowService.sendSegments(normalizedUpdate.upsertSegments).catch(logger.error);
                },
                (error: string) => {
                    logger.error('[CaptionSession] Service error:', error);
                }
            );
            logger.info('[CaptionSession] Caption recognizer started.');

            if (!activeRef.current) {
                await captionService.stop();
                return;
            }

            // 5. Connect Audio Pipeline (ONLY for Web API fallback)
            if (!usingNativeCaptureRef.current && !processorRef.current && audioContextRef.current && streamRef.current) {
                const source = audioContextRef.current.createMediaStreamSource(streamRef.current);
                sourceRef.current = source;

                const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');
                processorRef.current = processor;

                processor.port.onmessage = (e) => {
                    captionService.sendAudioInt16(e.data).catch(logger.error);
                };

                source.connect(processor);
                processor.connect(audioContextRef.current.destination);
            }

            if (!activeRef.current) return;

        } catch (error) {
            logger.error('[CaptionSession] Error starting session:', error);
            stopCaptionSession();
        } finally {
            setIsInitializing(false);
        }
    }, [config, stopCaptionSession]);


    // Effect: Manage Session based on Mode
    useEffect(() => {
        queueMicrotask(() => {
            if (isCaptionMode) {
                void startCaptionSession();
                return;
            }

            void stopCaptionSession();
        });
    }, [isCaptionMode, startCaptionSession, stopCaptionSession]);


    // Effect: Handle Service Config Changes while Active (Restart Service)
    useEffect(() => {
        const update = async () => {
            // Configuration updates are handled globally in useTranscriptionServiceSync.
            // When config changes, it updates the global transcriptionService.
            // We just need to restart the stream if we are in caption mode.
            if (isCaptionMode && !isInitializing) {
                const captionService = captionTranscriptionService;
                activeServiceRef.current = captionService;
                await captionService.start(
                    (update: TranscriptUpdate | TranscriptSegment) => {
                        const normalizedUpdate = normalizeTranscriptUpdate(update);
                        return captionWindowService.sendSegments(normalizedUpdate.upsertSegments).catch(logger.error);
                    },
                    (error: string) => logger.error('[CaptionSession] Service error:', error)
                );
            }
        };
        update();
    }, [
        config.streamingModelPath,
        config.language,
        config.enableITN,
        config.punctuationModelPath,
        config.vadModelPath,
        config.vadBufferSize,
        isCaptionMode,
        isInitializing,
    ]);

    // Effect: Handle Style Changes (No Restart)
    useEffect(() => {
        if (isCaptionMode && !isInitializing) {
            captionWindowService.updateStyle({
                width: config.captionWindowWidth,
                fontSize: config.captionFontSize,
                color: config.captionFontColor,
                backgroundOpacity: config.captionBackgroundOpacity
            }).catch(logger.error);
        }
    }, [config.captionWindowWidth, config.captionFontSize, config.captionFontColor, config.captionBackgroundOpacity, isCaptionMode, isInitializing]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            void stopCaptionSession();
        };
    }, [stopCaptionSession]);

    return {
        isInitializing
    };
}
