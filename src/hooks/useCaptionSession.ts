import { useEffect, useRef, useState, useCallback } from 'react';
import { TranscriptionService } from '../services/transcriptionService';
import { captionWindowService } from '../services/captionWindowService';
import { modelService } from '../services/modelService';
import { AppConfig } from '../types/transcript';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export function useCaptionSession(config: AppConfig, isCaptionMode: boolean) {
    const [isInitializing, setIsInitializing] = useState(false);

    // Refs to hold instances across renders
    // We instantiate the service lazily or once.
    const serviceRef = useRef<TranscriptionService>(new TranscriptionService());
    const audioContextRef = useRef<AudioContext | null>(null);
    const streamRef = useRef<MediaStream | null>(null);
    const processorRef = useRef<AudioWorkletNode | null>(null);
    const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);

    // Native capture refs
    const usingNativeCaptureRef = useRef(false);
    const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);

    // Track active state to handle race conditions
    const activeRef = useRef(isCaptionMode);

    useEffect(() => {
        activeRef.current = isCaptionMode;
    }, [isCaptionMode]);

    // Helper to update service config from AppConfig
    const updateServiceConfig = useCallback(async (service: TranscriptionService, cfg: AppConfig) => {
        service.setModelPath(cfg.offlineModelPath);
        service.setLanguage(cfg.language);
        service.setEnableITN(cfg.enableITN ?? false);
        service.setPunctuationModelPath(cfg.punctuationModelPath || '');
        service.setCtcModelPath(cfg.ctcModelPath || '');
        service.setVadModelPath(cfg.vadModelPath || '');
        service.setVadBufferSize(cfg.vadBufferSize || 5);

        // ITN Setup
        const enabledITNModels = new Set(cfg.enabledITNModels || []);
        const itnRulesOrder = cfg.itnRulesOrder || ['itn-zh-number'];
        if (enabledITNModels.size > 0) {
            try {
                const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
                service.setITNModelPaths(paths);
            } catch (e) {
                console.warn('[CaptionSession] Failed to setup ITN paths:', e);
                service.setITNModelPaths([]);
            }
        } else {
            service.setITNModelPaths([]);
        }
    }, []); // Stable callback

    const stopCaptionSession = useCallback(async () => {
        console.log('[CaptionSession] Stopping session...');

        // Close Window
        try {
            await captionWindowService.close();
        } catch (e) { console.error(e); }

        // Stop Native Capture
        if (usingNativeCaptureRef.current) {
            try {
                await invoke('stop_system_audio_capture');
            } catch (e) { console.error(e); }
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
            } catch (e) { console.error(e); }
            audioContextRef.current = null;
        }

        // Stop Stream (Web API fallback)
        if (streamRef.current) {
            streamRef.current.getTracks().forEach(t => t.stop());
            streamRef.current = null;
        }

        // Stop Service
        if (serviceRef.current) {
            await serviceRef.current.stop();
        }

        processorRef.current = null;
        sourceRef.current = null;
        setIsInitializing(false);
    }, []);

    const startCaptionSession = useCallback(async () => {
        if (!config.offlineModelPath) {
            console.warn('[CaptionSession] No offline model path configured.');
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
            console.log('[CaptionSession] Starting caption session...');

            if (!activeRef.current) return;

            // 1. Get Audio Source (Try Native -> Fallback to Web API)
            if (!streamRef.current && !usingNativeCaptureRef.current) {
                let nativeSuccess = false;
                try {
                    console.log('[CaptionSession] Attempting native system audio capture...');
                    await invoke('start_system_audio_capture');
                    const unlisten = await listen<number[]>('system-audio', (event) => {
                        const samples = new Int16Array(event.payload);
                        serviceRef.current.sendAudioInt16(samples);
                    });
                    systemAudioUnlistenRef.current = unlisten;
                    usingNativeCaptureRef.current = true;
                    nativeSuccess = true;
                    console.log('[CaptionSession] Native capture started.');
                } catch (e) {
                    console.warn('[CaptionSession] Native capture failed, falling back to Web API:', e);
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
                            console.log('[CaptionSession] Stream ended by user.');
                            stopCaptionSession();
                        };
                    } catch (err) {
                        if (!activeRef.current) return;
                        console.error('[CaptionSession] Failed to get display media:', err);
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

            // 3. Configure Service
            const service = serviceRef.current;
            await updateServiceConfig(service, config);

            if (!activeRef.current) return;

            // 4. Start Service
            await service.start(
                (segment) => {
                    captionWindowService.sendSegments([segment]).catch(console.error);
                },
                (error) => {
                    console.error('[CaptionSession] Service error:', error);
                }
            );

            if (!activeRef.current) {
                await service.stop();
                return;
            }

            // 5. Connect Audio Pipeline (ONLY for Web API fallback)
            if (!usingNativeCaptureRef.current && !processorRef.current && audioContextRef.current && streamRef.current) {
                const source = audioContextRef.current.createMediaStreamSource(streamRef.current);
                sourceRef.current = source;

                const processor = new AudioWorkletNode(audioContextRef.current, 'audio-processor');
                processorRef.current = processor;

                processor.port.onmessage = (e) => {
                    service.sendAudioInt16(e.data);
                };

                source.connect(processor);
                processor.connect(audioContextRef.current.destination);
            }

            if (!activeRef.current) return;

            // 6. Open Window
            await captionWindowService.open({
                alwaysOnTop: config.alwaysOnTop ?? true,
                lockWindow: config.lockWindow ?? false,
                width: config.captionWindowWidth,
                fontSize: config.captionFontSize,
                color: config.captionFontColor
            });

        } catch (error) {
            console.error('[CaptionSession] Error starting session:', error);
            stopCaptionSession();
        } finally {
            setIsInitializing(false);
        }
    }, [config, updateServiceConfig, stopCaptionSession]);


    // Effect: Manage Session based on Mode
    useEffect(() => {
        if (isCaptionMode) {
            // Start
            startCaptionSession();
        } else {
            // Stop
            stopCaptionSession();
        }
    }, [isCaptionMode, startCaptionSession, stopCaptionSession]);


    // Effect: Handle Service Config Changes while Active (Restart Service)
    useEffect(() => {
        const update = async () => {
            if (isCaptionMode && serviceRef.current && !isInitializing) {
                // Update service config and restart sidecar only
                await updateServiceConfig(serviceRef.current, config);

                // Triggers internal restart check
                await serviceRef.current.start(
                    (segment) => captionWindowService.sendSegments([segment]).catch(console.error),
                    (error) => console.error('[CaptionSession] Service error:', error)
                );
            }
        };
        update();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [
        config.offlineModelPath,
        config.language,
        config.enableITN,
        config.punctuationModelPath,
        config.ctcModelPath,
        config.vadModelPath,
        config.vadBufferSize,
        // Use JSON stringify for deep comparison of arrays/sets
        JSON.stringify(config.enabledITNModels),
        JSON.stringify(config.itnRulesOrder),
        isCaptionMode,
        isInitializing,
        updateServiceConfig
    ]);

    // Effect: Handle Style Changes (No Restart)
    useEffect(() => {
        if (isCaptionMode && !isInitializing) {
            captionWindowService.updateStyle({
                width: config.captionWindowWidth,
                fontSize: config.captionFontSize,
                color: config.captionFontColor
            }).catch(console.error);
        }
    }, [config.captionWindowWidth, config.captionFontSize, config.captionFontColor, isCaptionMode, isInitializing]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            stopCaptionSession();
        };
    }, []); // Empty deps means unmount only

    return {
        isInitializing
    };
}
