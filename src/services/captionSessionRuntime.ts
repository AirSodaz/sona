import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { remove } from '@tauri-apps/plugin-fs';
import { captionWindowService } from './captionWindowService';
import {
  captionTranscriptionService,
  type TranscriptionService,
} from './transcriptionService';
import {
  startSystemAudioCapture,
  stopSystemAudioCapture,
} from './tauri/audio';
import { TauriEvent } from './tauri/events';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';

type CaptionWindowOpenOptions = Parameters<typeof captionWindowService.open>[0];
type CaptionWindowStyleOptions = Parameters<typeof captionWindowService.updateStyle>[0];

interface CaptionNativeCaptureResult {
  started: boolean;
  unlisten: UnlistenFn | null;
}

interface WebAudioPipeline {
  processor: AudioWorkletNode;
}

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

async function tryStartNativeCaptionCapture(config: AppConfig): Promise<CaptionNativeCaptureResult> {
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
): WebAudioPipeline {
  const source = audioContext.createMediaStreamSource(stream);
  const processor = new AudioWorkletNode(audioContext, 'audio-processor');

  processor.port.onmessage = (event) => {
    void captionService.sendAudioInt16(event.data).catch(logger.error);
  };

  source.connect(processor);
  processor.connect(audioContext.destination);

  return {
    processor,
  };
}

class CaptionSessionRuntime {
  private audioContext: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private processor: AudioWorkletNode | null = null;
  private activeService: TranscriptionService = captionTranscriptionService;
  private usingNativeCapture = false;
  private systemAudioUnlisten: UnlistenFn | null = null;

  isRunning(): boolean {
    return Boolean(
      (this.stream && this.audioContext?.state === 'running')
      || this.usingNativeCapture,
    );
  }

  async start(
    config: AppConfig,
    isActive: () => boolean,
    onStreamEnded: () => void,
  ): Promise<void> {
    if (this.isRunning()) {
      return;
    }

    logger.info('[CaptionSession] Starting caption session...');

    if (!isActive()) {
      return;
    }

    const captionService = captionTranscriptionService;
    this.activeService = captionService;

    if (!this.stream && !this.usingNativeCapture) {
      const nativeCapture = await tryStartNativeCaptionCapture(config);
      if (nativeCapture.started) {
        this.systemAudioUnlisten = nativeCapture.unlisten;
        this.usingNativeCapture = true;
      } else {
        this.stream = await requestDisplayMediaFallback(isActive, onStreamEnded);
      }
    }

    if (!isActive()) {
      return;
    }

    if (!this.usingNativeCapture) {
      this.audioContext = await resolveCaptionAudioContext(
        this.audioContext,
        isActive,
      );

      if (!isActive()) {
        return;
      }
    }

    logger.info('[CaptionSession] Opening caption window...');
    await captionWindowService.open(buildCaptionWindowOptions(config));

    if (!isActive()) {
      return;
    }

    await startCaptionRecognizer(captionService);

    if (!isActive()) {
      await captionService.stop();
      return;
    }

    if (
      !this.usingNativeCapture
      && !this.processor
      && this.audioContext
      && this.stream
    ) {
      const pipeline = connectWebAudioPipeline(
        this.audioContext,
        this.stream,
        captionService,
      );
      this.processor = pipeline.processor;
    }
  }

  async stop(): Promise<void> {
    logger.info('[CaptionSession] Stopping session...');

    await closeCaptionWindow();

    if (this.usingNativeCapture) {
      await stopNativeCaptureAndDiscardFile();
      this.usingNativeCapture = false;
    }

    clearNativePeakListener(this.systemAudioUnlisten);
    this.systemAudioUnlisten = null;

    await closeAudioContext(this.audioContext);
    this.audioContext = null;

    if (this.stream) {
      stopStreamTracks(this.stream);
      this.stream = null;
    }

    await this.activeService.stop();
    this.activeService = captionTranscriptionService;

    this.processor = null;
  }

  async restartRecognizer(): Promise<void> {
    const captionService = captionTranscriptionService;
    this.activeService = captionService;
    await captionService.start(sendCaptionSegments, logCaptionServiceError);
  }

  async updateStyle(config: AppConfig): Promise<void> {
    await captionWindowService.updateStyle(buildCaptionWindowStyle({
      width: config.captionWindowWidth,
      fontSize: config.captionFontSize,
      color: config.captionFontColor,
      backgroundOpacity: config.captionBackgroundOpacity,
    }));
  }

  resetForTesting(): void {
    this.audioContext = null;
    this.stream = null;
    this.processor = null;
    this.activeService = captionTranscriptionService;
    this.usingNativeCapture = false;
    this.systemAudioUnlisten = null;
  }
}

export const captionSessionRuntime = new CaptionSessionRuntime();
