import { useEffect, useRef, useState, useCallback } from 'react';
import { TranscriptionService } from '../services/transcriptionService';
import { captionWindowService } from '../services/captionWindowService';
import { modelService } from '../services/modelService';
import { AppConfig } from '../types/transcript';

export function useCaptionSession(config: AppConfig, isCaptionMode: boolean) {
    const [isInitializing, setIsInitializing] = useState(false);

    // Refs to hold instances across renders
    // We instantiate the service lazily or once.
    const serviceRef = useRef<TranscriptionService>(new TranscriptionService());
    const audioContextRef = useRef<AudioContext | null>(null);
    const streamRef = useRef<MediaStream | null>(null);
    const processorRef = useRef<AudioWorkletNode | null>(null);
    const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);

    // Track active state to handle race conditions
    const activeRef = useRef(isCaptionMode);

    useEffect(() => {
        activeRef.current = isCaptionMode;
    }, [isCaptionMode]);

    // Helper to update service config from AppConfig
    const updateServiceConfig = useCallback(async (service: TranscriptionService) => {
        service.setModelPath(config.offlineModelPath);
        service.setLanguage(config.language);
        service.setEnableITN(config.enableITN ?? false);
        service.setPunctuationModelPath(config.punctuationModelPath || '');
        service.setCtcModelPath(config.ctcModelPath || '');
        service.setVadModelPath(config.vadModelPath || '');
        service.setVadBufferSize(config.vadBufferSize || 5);

        // ITN Setup
        const enabledITNModels = new Set(config.enabledITNModels || []);
        const itnRulesOrder = config.itnRulesOrder || ['itn-zh-number'];
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
    }, [config]);

    const stopCaptionSession = useCallback(async () => {
        console.log('[CaptionSession] Stopping session...');

        // Close Window
        try {
            await captionWindowService.close();
        } catch (e) { console.error(e); }

        // Stop Audio Context
        if (audioContextRef.current) {
            try {
                await audioContextRef.current.close();
            } catch (e) { console.error(e); }
            audioContextRef.current = null;
        }

        // Stop Stream
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

        if (streamRef.current && audioContextRef.current && audioContextRef.current.state === 'running') {
             // Already running
             return;
        }

        try {
            setIsInitializing(true);
            console.log('[CaptionSession] Starting caption session...');

            if (!activeRef.current) return;

            // 1. Get Desktop Stream
            if (!streamRef.current) {
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
                        // Note: We cannot easily update external state (isCaptionMode) here without a setter.
                        // Ideally we'd accept setIsCaptionMode as a prop.
                    };
                } catch (err) {
                    if (!activeRef.current) return;
                    console.error('[CaptionSession] Failed to get display media:', err);
                    throw err;
                }
            }

            if (!activeRef.current) return;

            // 2. Initialize Audio Context
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

            // 3. Configure Service
            const service = serviceRef.current;
            await updateServiceConfig(service);

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

            // 5. Connect Audio Pipeline
            if (!processorRef.current && audioContextRef.current && streamRef.current) {
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
                lockWindow: config.lockWindow ?? false
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


    // Effect: Handle Config Changes while Active
    useEffect(() => {
        const update = async () => {
            if (isCaptionMode && serviceRef.current && !isInitializing) {
                // Update service config and restart sidecar only
                await updateServiceConfig(serviceRef.current);

                // Triggers internal restart check
                await serviceRef.current.start(
                    (segment) => captionWindowService.sendSegments([segment]).catch(console.error),
                    (error) => console.error('[CaptionSession] Service error:', error)
                );
            }
        };
        update();
    }, [config, isCaptionMode, isInitializing, updateServiceConfig]);

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
