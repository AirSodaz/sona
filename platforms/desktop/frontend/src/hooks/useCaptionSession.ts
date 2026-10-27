import { useCallback, useEffect, useRef, useState } from 'react';
import {
  isAsrRequestConfigured,
  resolveAsrTranscriptionRequest,
} from '../services/asrConfigService';
import { captionSessionRuntime } from '../services/captionSessionRuntime';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';

interface CaptionSessionState {
  isInitializing: boolean;
}

export function useCaptionSession(config: AppConfig, isCaptionMode: boolean): CaptionSessionState {
  const [isInitializing, setIsInitializing] = useState(false);
  const activeRef = useRef(isCaptionMode);

  const previousConfigRef = useRef<string | null>(null);
  useEffect(() => {
    activeRef.current = isCaptionMode;
  }, [isCaptionMode]);

  const stopCaptionSession = useCallback(async function stopCaptionSession(): Promise<void> {
    try {
      await captionSessionRuntime.stop();
    } catch (error) {
      logger.warn('[CaptionSession] Error stopping session:', error);
    } finally {
      setIsInitializing(false);
      previousConfigRef.current = null;
    }
  }, []);

  const startCaptionSession = useCallback(
    async function startCaptionSession(): Promise<void> {
      if (
        !isAsrRequestConfigured(
          resolveAsrTranscriptionRequest(getEffectiveConfigSnapshot(), 'caption')
        )
      ) {
        logger.warn('Cannot start caption: ASR is not configured.');
        return;
      }

      try {
        setIsInitializing(true);
        previousConfigRef.current = JSON.stringify([
          config.streamingModelPath,
          config.asr,
          config.language,
          config.enableITN,
          config.livePunctuationModelPath,
          config.liveVadModelPath,
          config.liveVadBufferSize,
          config.realtimeTranslationEnabled,
          config.realtimeTranslationSourceLanguage,
          config.realtimeTranslationTargetLanguage,
          config.realtimeTranslationEngine,
          config.realtimeTranslationModelId,
        ]);
        await captionSessionRuntime.start(
          config,
          () => activeRef.current,
          () => {
            logger.info('[CaptionSession] Stream ended by user.');
            void stopCaptionSession();
          }
        );
      } catch (error) {
        logger.error('[CaptionSession] Error starting session:', error);
        void stopCaptionSession();
      } finally {
        setIsInitializing(false);
      }
    },
    [config, stopCaptionSession]
  );

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
    const currentConfigKey = JSON.stringify([
      config.streamingModelPath,
      config.asr,
      config.language,
      config.enableITN,
      config.livePunctuationModelPath,
      config.liveVadModelPath,
      config.liveVadBufferSize,
      config.realtimeTranslationEnabled,
      config.realtimeTranslationSourceLanguage,
      config.realtimeTranslationTargetLanguage,
      config.realtimeTranslationEngine,
      config.realtimeTranslationModelId,
    ]);

    if (!isCaptionMode || isInitializing || !captionSessionRuntime.isRunning()) {
      return;
    }

    if (previousConfigRef.current === null) {
      previousConfigRef.current = currentConfigKey;
      return;
    }

    if (previousConfigRef.current === currentConfigKey) {
      return;
    }

    previousConfigRef.current = currentConfigKey;

    async function restartCaptionRecognizer(): Promise<void> {
      try {
        await captionSessionRuntime.restartRecognizer();
      } catch (error) {
        logger.warn('[CaptionSession] Failed to restart recognizer on config change:', error);
      }
    }

    void restartCaptionRecognizer();
  }, [
    config.streamingModelPath,
    config.asr,
    config.language,
    config.enableITN,
    config.livePunctuationModelPath,
    config.liveVadModelPath,
    config.liveVadBufferSize,
    config.realtimeTranslationEnabled,
    config.realtimeTranslationSourceLanguage,
    config.realtimeTranslationTargetLanguage,
    config.realtimeTranslationEngine,
    config.realtimeTranslationModelId,
    isCaptionMode,
    isInitializing,
  ]);
  useEffect(() => {
    if (!isCaptionMode || isInitializing) {
      return;
    }

    void captionSessionRuntime.updateStyle(config).catch(logger.error);
  }, [
    config,
    config.captionWindowWidth,
    config.captionFontSize,
    config.captionFontColor,
    config.captionBackgroundColor,
    config.captionBackgroundOpacity,
    config.captionTranslationFontSize,
    config.captionTranslationFontColor,
    config.captionDisplayMode,
    config.captionBilingualOrder,
    config.captionDisplayPriority,
    config.captionTextStroke,
    config.captionStrokeWidth,
    config.captionStrokeColor,
    config.captionTextShadow,
    config.captionTextAlign,
    config.captionBackgroundBlur,
    config.captionBackgroundBlurRadius,
    config.captionDisappearDelay,
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
