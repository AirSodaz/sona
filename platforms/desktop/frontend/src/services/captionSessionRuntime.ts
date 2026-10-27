import { useConfigStore } from '../stores/configStore';
import type { AppConfig } from '../types/config';
import type { TranscriptSegment, TranscriptUpdate } from '../types/transcript';
import { logger } from '../utils/logger';
import { normalizeTranscriptUpdate } from '../utils/transcriptTiming';
import { captionWindowService } from './captionWindowService';
import { TauriEvent } from './tauri/events';
import { listen, type UnlistenFn } from './tauri/platform/events';
import { register, unregister } from './tauri/platform/globalShortcut';
import { captionTranscriptionService, type TranscriptionService } from './transcriptionService';

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
    backgroundColor: config.captionBackgroundColor,
    backgroundOpacity: config.captionBackgroundOpacity,
    displayMode: config.captionDisplayMode,
    bilingualOrder: config.captionBilingualOrder,
    translationColor: config.captionTranslationFontColor,
    translationFontSize: config.captionTranslationFontSize,
    displayPriority: config.captionDisplayPriority,
    textStroke: config.captionTextStroke,
    strokeWidth: config.captionStrokeWidth,
    strokeColor: config.captionStrokeColor,
    textShadow: config.captionTextShadow,
    textAlign: config.captionTextAlign,
    backgroundBlur: config.captionBackgroundBlur,
    backgroundBlurRadius: config.captionBackgroundBlurRadius,
    disappearDelay: config.captionDisappearDelay,
  };
}

function buildCaptionWindowStyle({
  width,
  fontSize,
  color,
  backgroundColor,
  backgroundOpacity,
  displayMode,
  bilingualOrder,
  translationColor,
  translationFontSize,
  displayPriority,
  textStroke,
  strokeWidth,
  strokeColor,
  textShadow,
  textAlign,
  backgroundBlur,
  backgroundBlurRadius,
  disappearDelay,
}: {
  width?: number;
  fontSize?: number;
  color?: string;
  backgroundColor?: string;
  backgroundOpacity?: number;
  displayMode?: 'bilingual' | 'translation' | 'original';
  bilingualOrder?: 'original_first' | 'translation_first';
  translationColor?: string;
  translationFontSize?: number;
  displayPriority?: 'translation_main' | 'original_main' | 'translation_only' | 'original_only';
  textStroke?: boolean;
  strokeWidth?: number;
  strokeColor?: string;
  textShadow?: boolean;
  textAlign?: 'center' | 'left';
  backgroundBlur?: boolean;
  backgroundBlurRadius?: number;
  disappearDelay?: number;
}): CaptionWindowStyleOptions {
  return {
    width,
    fontSize,
    color,
    backgroundColor,
    backgroundOpacity,
    ...(displayMode !== undefined ? { displayMode } : {}),
    ...(bilingualOrder !== undefined ? { bilingualOrder } : {}),
    ...(translationColor !== undefined ? { translationColor } : {}),
    ...(translationFontSize !== undefined ? { translationFontSize } : {}),
    ...(displayPriority !== undefined ? { displayPriority } : {}),
    ...(textStroke !== undefined ? { textStroke } : {}),
    ...(strokeWidth !== undefined ? { strokeWidth } : {}),
    ...(strokeColor !== undefined ? { strokeColor } : {}),
    ...(textShadow !== undefined ? { textShadow } : {}),
    ...(textAlign !== undefined ? { textAlign } : {}),
    ...(backgroundBlur !== undefined ? { backgroundBlur } : {}),
    ...(backgroundBlurRadius !== undefined ? { backgroundBlurRadius } : {}),
    ...(disappearDelay !== undefined ? { disappearDelay } : {}),
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
  stream.getTracks().forEach((track) => {
    track.stop();
  });
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
    logger.debug('[CaptionSession] Failed to close caption window:', error);
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

async function tryStartNativeCaptionCapture(
  config: AppConfig
): Promise<CaptionNativeCaptureResult> {
  try {
    logger.info('[CaptionSession] Attempting native system audio capture...');
    await captionTranscriptionService.startNative(sendCaptionSegments, logCaptionServiceError, {
      sourceKind: 'system',
      deviceName: getCaptionDeviceName(config),
      callbackOwner: 'caption',
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
  onStreamEnded: () => void
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

    stream.getVideoTracks().forEach((track) => {
      track.stop();
    });
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
  isActive: () => boolean
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

function connectWebAudioPipeline(
  audioContext: AudioContext,
  stream: MediaStream,
  captionService: TranscriptionService
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

  private registeredShortcut: string | null = null;
  private lastLockToggleTime = 0;
  isRunning(): boolean {
    return Boolean(
      (this.stream && this.audioContext?.state === 'running') || this.usingNativeCapture
    );
  }

  async start(
    config: AppConfig,
    isActive: () => boolean,
    onStreamEnded: () => void
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
      await this.stop();
      return;
    }

    if (!this.usingNativeCapture) {
      this.audioContext = await resolveCaptionAudioContext(this.audioContext, isActive);

      if (!isActive()) {
        await this.stop();
        return;
      }
    }

    logger.info('[CaptionSession] Opening caption window...');
    await captionWindowService.open(buildCaptionWindowOptions(config));

    if (!isActive()) {
      await this.stop();
      return;
    }

    if (!this.usingNativeCapture) {
      logger.info('[CaptionSession] Starting caption recognizer...');
      await captionService.startExternal(sendCaptionSegments, logCaptionServiceError);
      logger.info('[CaptionSession] Caption recognizer started.');
    }

    if (!isActive()) {
      await this.stop();
      return;
    }

    if (!this.usingNativeCapture && !this.processor && this.audioContext && this.stream) {
      const pipeline = connectWebAudioPipeline(this.audioContext, this.stream, captionService);
      this.processor = pipeline.processor;
    }

    const shortcut = config.captionLockShortcut || 'Alt+C';
    const normalizedShortcut = shortcut.replace(/\s+/g, '');
    if (normalizedShortcut) {
      try {
        await register(normalizedShortcut, async (event) => {
          if (event?.state && event.state !== 'Pressed') {
            return;
          }
          const now = Date.now();
          if (now - this.lastLockToggleTime < 300) {
            return;
          }
          this.lastLockToggleTime = now;

          const currentLock = useConfigStore.getState().config.lockWindow ?? false;
          const nextLock = !currentLock;
          useConfigStore.getState().setConfig({ lockWindow: nextLock });
          await captionWindowService.setClickThrough(nextLock);
        });
        this.registeredShortcut = normalizedShortcut;
      } catch (error) {
        logger.warn(
          '[CaptionSession] Failed to register global lock shortcut:',
          normalizedShortcut,
          error
        );
      }
    }
  }
  async stop(): Promise<void> {
    logger.info('[CaptionSession] Stopping session...');

    await closeCaptionWindow();

    try {
      await this.activeService.stop();
    } catch (error) {
      logger.warn('[CaptionSession] Error while stopping active service:', error);
    }
    this.usingNativeCapture = false;

    clearNativePeakListener(this.systemAudioUnlisten);
    this.systemAudioUnlisten = null;

    await closeAudioContext(this.audioContext);
    this.audioContext = null;

    if (this.stream) {
      stopStreamTracks(this.stream);
      this.stream = null;
    }

    this.activeService = captionTranscriptionService;

    this.processor = null;

    if (this.registeredShortcut) {
      try {
        await unregister(this.registeredShortcut);
      } catch (error) {
        logger.debug('[CaptionSession] Failed to unregister global lock shortcut:', error);
      }
      this.registeredShortcut = null;
    }
  }

  async restartRecognizer(): Promise<void> {
    if (!this.isRunning()) {
      logger.debug('[CaptionSession] Skipping restartRecognizer because session is not running');
      return;
    }

    const captionService = captionTranscriptionService;
    this.activeService = captionService;
    await captionService.restartStream();
  }

  async updateStyle(config: AppConfig): Promise<void> {
    await captionWindowService.updateStyle(
      buildCaptionWindowStyle({
        width: config.captionWindowWidth,
        fontSize: config.captionFontSize,
        color: config.captionFontColor,
        backgroundColor: config.captionBackgroundColor,
        backgroundOpacity: config.captionBackgroundOpacity,
        displayMode: config.captionDisplayMode,
        bilingualOrder: config.captionBilingualOrder,
        translationColor: config.captionTranslationFontColor,
        translationFontSize: config.captionTranslationFontSize,
        displayPriority: config.captionDisplayPriority,
        textStroke: config.captionTextStroke,
        strokeWidth: config.captionStrokeWidth,
        strokeColor: config.captionStrokeColor,
        textShadow: config.captionTextShadow,
        textAlign: config.captionTextAlign,
        backgroundBlur: config.captionBackgroundBlur,
        backgroundBlurRadius: config.captionBackgroundBlurRadius,
        disappearDelay: config.captionDisappearDelay,
      })
    );
  }

  resetForTesting(): void {
    this.audioContext = null;
    this.stream = null;
    this.processor = null;
    this.activeService = captionTranscriptionService;
    this.usingNativeCapture = false;
    this.systemAudioUnlisten = null;
    this.registeredShortcut = null;
    this.lastLockToggleTime = 0;
  }
}

export const captionSessionRuntime = new CaptionSessionRuntime();
