import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { remove } from '@tauri-apps/plugin-fs';
import { useCallback, useEffect, useRef, useState } from 'react';
import { captionWindowService } from '../services/captionWindowService';
import {
  captionTranscriptionService,
  type TranscriptionService,
} from '../services/transcriptionService';
import {
  startSystemAudioCapture,
  stopSystemAudioCapture,
} from '../services/tauri/audio';
import { TauriEvent } from '../services/tauri/events';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';

interface CaptionSessionState {
  isInitializing: boolean;
}

type CaptionWindowOpenOptions = Parameters<typeof captionWindowService.open>[0];
type CaptionWindowStyleOptions = Parameters<typeof captionWindowService.updateStyle>[0];

function getCaptionDeviceName(config: AppConfig): string | null {
  if (config.systemAudioDeviceId && config.systemAudioDeviceId !== 'default') {
    return config.systemAudioDeviceId;
  }

  return null;
}

function buildCaptionWindowOptions(config: AppConfig): CaptionWindowOpenOptions {
  return {
    alwaysOnTop: config.alwaysOnTop ?? true,
    lockWindow: config.lockWindow ?? false,
    width: config.captionWindowWidth,
    fontSize: config.captionFontSize,
    color: config.captionFontColor,
    backgroundOpacity: config.captionBackgroundOpacity,
  };
}

function buildCaptionWindowStyle({
  width,
  fontSize,
  color,
  backgroundOpacity,
}: {
  width?: number;
  fontSize?: number;
  color?: string;
  backgroundOpacity?: number;
}): CaptionWindowStyleOptions {
  return {
    width,
    fontSize,
    color,
    backgroundOpacity,
  };
}

function buildDisplayMediaOptions(): DisplayMediaStreamOptions {
  return {
    video: {
      width: 1,
      height: 1,
      frameRate: 1,
    },
    audio: {
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
    },
  };
}

function isCaptionSessionRunning(
  stream: MediaStream | null,
  audioContext: AudioContext | null,
  usingNativeCapture: boolean,
): boolean {
  return Boolean(
    (stream && audioContext?.state === 'running')
    || usingNativeCapture,
  );
}

function stopStreamTracks(stream: MediaStream): void {
  stream.getTracks().forEach((track) => track.stop());
}

function clearNativePeakListener(unlisten: UnlistenFn | null): void {
  if (unlisten) {
    unlisten();
  }
}

async function closeCaptionWindow(): Promise<void> {
  try {
    await captionWindowService.close();
  } catch (error) {
    logger.error('Error:', error);
  }
}

async function deleteNativeCaptureFile(savedWavPath: string): Promise<void> {
  logger.info('[CaptionSession] Deleting auto-saved native capture file:', savedWavPath);

  try {
    await remove(savedWavPath);
  } catch (error) {
    logger.error('[CaptionSession] Failed to delete native capture file:', error);
  }
}

async function stopNativeCaptureAndDiscardFile(): Promise<void> {
  try {
    const savedWavPath = await stopSystemAudioCapture('caption');
    if (savedWavPath) {
      await deleteNativeCaptureFile(savedWavPath);
    }
  } catch (error) {
    logger.error('Error:', error);
  }
}

async function closeAudioContext(audioContext: AudioContext | null): Promise<void> {
  if (!audioContext) {
    return;
  }

  try {
    await audioContext.close();
  } catch (error) {
    logger.error('Error:', error);
  }
}

async function tryStartNativeCaptionCapture(config: AppConfig): Promise<{
  started: boolean;
  unlisten: UnlistenFn | null;
}> {
  try {
    logger.info('[CaptionSession] Attempting native system audio capture...');
    await startSystemAudioCapture({
      deviceName: getCaptionDeviceName(config),
      instanceId: 'caption',
    });
    const unlisten = await listen<number>(TauriEvent.audio.systemPeak, () => {
      // The Rust backend feeds the recognizer directly for native caption capture.
    });

    logger.info('[CaptionSession] Native capture started.');
    return {
      started: true,
      unlisten,
    };
  } catch (error) {
    logger.warn('[CaptionSession] Native capture failed, falling back to Web API:', error);
    return {
      started: false,
      unlisten: null,
    };
  }
}

async function requestDisplayMediaFallback(
  isActive: () => boolean,
  onStreamEnded: () => void,
): Promise<MediaStream | null> {
  try {
    const stream = await navigator.mediaDevices.getDisplayMedia(buildDisplayMediaOptions());

    if (!isActive()) {
      stopStreamTracks(stream);
      return null;
    }

    const audioTracks = stream.getAudioTracks();
    if (audioTracks.length === 0) {
      throw new Error('No audio track selected in screen share.');
    }

    stream.getVideoTracks().forEach((track) => track.stop());
    const audioOnlyStream = new MediaStream([audioTracks[0]]);
    audioOnlyStream.getAudioTracks()[0].onended = onStreamEnded;

    return audioOnlyStream;
  } catch (error) {
    if (!isActive()) {
      return null;
    }

    logger.error('[CaptionSession] Failed to get display media:', error);
    throw error;
  }
}

async function resolveCaptionAudioContext(
  existingAudioContext: AudioContext | null,
  isActive: () => boolean,
): Promise<AudioContext | null> {
  let audioContext = existingAudioContext;

  if (!audioContext || audioContext.state === 'closed') {
    audioContext = new AudioContext({ sampleRate: 16000 });

    try {
      await audioContext.audioWorklet.addModule('/audio-processor.js');
    } catch (error) {
      if (!isActive()) {
        await audioContext.close();
        return null;
      }

      throw error;
    }
  } else if (audioContext.state === 'suspended') {
    await audioContext.resume();
  }

  if (!isActive()) {
    await audioContext.close();
    return null;
  }

  return audioContext;
}

function sendCaptionSegments(update: TranscriptUpdate | TranscriptSegment): void {
  const normalizedUpdate = normalizeTranscriptUpdate(update);
  void captionWindowService.sendSegments(normalizedUpdate.upsertSegments).catch(logger.error);
}

function logCaptionServiceError(error: string): void {
  logger.error('[CaptionSession] Service error:', error);
}

async function startCaptionRecognizer(captionService: TranscriptionService): Promise<void> {
  logger.info('[CaptionSession] Starting caption recognizer...');
  await captionService.start(sendCaptionSegments, logCaptionServiceError);
  logger.info('[CaptionSession] Caption recognizer started.');
}

function connectWebAudioPipeline(
  audioContext: AudioContext,
  stream: MediaStream,
  captionService: TranscriptionService,
): {
  processor: AudioWorkletNode;
  source: MediaStreamAudioSourceNode;
} {
  const source = audioContext.createMediaStreamSource(stream);
  const processor = new AudioWorkletNode(audioContext, 'audio-processor');

  processor.port.onmessage = (event) => {
    void captionService.sendAudioInt16(event.data).catch(logger.error);
  };

  source.connect(processor);
  processor.connect(audioContext.destination);

  return {
    processor,
    source,
  };
}

export function useCaptionSession(
  config: AppConfig,
  isCaptionMode: boolean,
): CaptionSessionState {
  const [isInitializing, setIsInitializing] = useState(false);

  const audioContextRef = useRef<AudioContext | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const processorRef = useRef<AudioWorkletNode | null>(null);
  const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const activeServiceRef = useRef<TranscriptionService>(captionTranscriptionService);
  const usingNativeCaptureRef = useRef(false);
  const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);
  const activeRef = useRef(isCaptionMode);

  useEffect(() => {
    activeRef.current = isCaptionMode;
  }, [isCaptionMode]);

  const stopCaptionSession = useCallback(async function stopCaptionSession(): Promise<void> {
    logger.info('[CaptionSession] Stopping session...');

    await closeCaptionWindow();

    if (usingNativeCaptureRef.current) {
      await stopNativeCaptureAndDiscardFile();
      usingNativeCaptureRef.current = false;
    }

    clearNativePeakListener(systemAudioUnlistenRef.current);
    systemAudioUnlistenRef.current = null;

    await closeAudioContext(audioContextRef.current);
    audioContextRef.current = null;

    if (streamRef.current) {
      stopStreamTracks(streamRef.current);
      streamRef.current = null;
    }

    await activeServiceRef.current.stop();
    activeServiceRef.current = captionTranscriptionService;

    processorRef.current = null;
    sourceRef.current = null;
    setIsInitializing(false);
  }, []);

  const startCaptionSession = useCallback(async function startCaptionSession(): Promise<void> {
    if (!config.streamingModelPath) {
      logger.warn('Cannot start caption: streaming model path is not set.');
      return;
    }

    if (isCaptionSessionRunning(
      streamRef.current,
      audioContextRef.current,
      usingNativeCaptureRef.current,
    )) {
      return;
    }

    try {
      setIsInitializing(true);
      logger.info('[CaptionSession] Starting caption session...');

      if (!activeRef.current) {
        return;
      }

      const captionService = captionTranscriptionService;
      activeServiceRef.current = captionService;

      if (!streamRef.current && !usingNativeCaptureRef.current) {
        const nativeCapture = await tryStartNativeCaptionCapture(config);
        if (nativeCapture.started) {
          systemAudioUnlistenRef.current = nativeCapture.unlisten;
          usingNativeCaptureRef.current = true;
        } else {
          const stream = await requestDisplayMediaFallback(
            () => activeRef.current,
            () => {
              logger.info('[CaptionSession] Stream ended by user.');
              void stopCaptionSession();
            },
          );
          streamRef.current = stream;
        }
      }

      if (!activeRef.current) {
        return;
      }

      if (!usingNativeCaptureRef.current) {
        audioContextRef.current = await resolveCaptionAudioContext(
          audioContextRef.current,
          () => activeRef.current,
        );

        if (!activeRef.current) {
          return;
        }
      }

      logger.info('[CaptionSession] Opening caption window...');
      await captionWindowService.open(buildCaptionWindowOptions(config));

      if (!activeRef.current) {
        return;
      }

      await startCaptionRecognizer(captionService);

      if (!activeRef.current) {
        await captionService.stop();
        return;
      }

      if (
        !usingNativeCaptureRef.current
        && !processorRef.current
        && audioContextRef.current
        && streamRef.current
      ) {
        const pipeline = connectWebAudioPipeline(
          audioContextRef.current,
          streamRef.current,
          captionService,
        );
        sourceRef.current = pipeline.source;
        processorRef.current = pipeline.processor;
      }

      if (!activeRef.current) {
        return;
      }
    } catch (error) {
      logger.error('[CaptionSession] Error starting session:', error);
      void stopCaptionSession();
    } finally {
      setIsInitializing(false);
    }
  }, [config, stopCaptionSession]);

  useEffect(() => {
    queueMicrotask(() => {
      if (isCaptionMode) {
        void startCaptionSession();
        return;
      }

      void stopCaptionSession();
    });
  }, [isCaptionMode, startCaptionSession, stopCaptionSession]);

  useEffect(() => {
    async function restartCaptionRecognizer(): Promise<void> {
      if (!isCaptionMode || isInitializing) {
        return;
      }

      const captionService = captionTranscriptionService;
      activeServiceRef.current = captionService;
      await captionService.start(sendCaptionSegments, logCaptionServiceError);
    }

    void restartCaptionRecognizer();
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

  useEffect(() => {
    if (!isCaptionMode || isInitializing) {
      return;
    }

    void captionWindowService.updateStyle(buildCaptionWindowStyle({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
      color: config.captionFontColor,
      backgroundOpacity: config.captionBackgroundOpacity,
    })).catch(logger.error);
  }, [
    config.captionWindowWidth,
    config.captionFontSize,
    config.captionFontColor,
    config.captionBackgroundOpacity,
    isCaptionMode,
    isInitializing,
  ]);

  useEffect(() => {
    return () => {
      void stopCaptionSession();
    };
  }, [stopCaptionSession]);

  return {
    isInitializing,
  };
}
