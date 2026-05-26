import { useCallback, useEffect, useRef, useState } from 'react';
import { captionSessionRuntime } from '../services/captionSessionRuntime';
import { isAsrRequestConfigured, resolveAsrTranscriptionRequest } from '../services/asrConfigService';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import type { AppConfig } from '../types/config';
import { logger } from '../utils/logger';

interface CaptionSessionState {
  isInitializing: boolean;
}

export function useCaptionSession(
  config: AppConfig,
  isCaptionMode: boolean,
): CaptionSessionState {
  const [isInitializing, setIsInitializing] = useState(false);
  const activeRef = useRef(isCaptionMode);

  useEffect(() => {
    activeRef.current = isCaptionMode;
  }, [isCaptionMode]);

  const stopCaptionSession = useCallback(async function stopCaptionSession(): Promise<void> {
    await captionSessionRuntime.stop();
    setIsInitializing(false);
  }, []);

  const startCaptionSession = useCallback(async function startCaptionSession(): Promise<void> {
    if (!isAsrRequestConfigured(resolveAsrTranscriptionRequest(getEffectiveConfigSnapshot(), 'caption'))) {
      logger.warn('Cannot start caption: ASR is not configured.');
      return;
    }

    try {
      setIsInitializing(true);
      await captionSessionRuntime.start(
        config,
        () => activeRef.current,
        () => {
          logger.info('[CaptionSession] Stream ended by user.');
          void stopCaptionSession();
        },
      );
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

      await captionSessionRuntime.restartRecognizer();
    }

    void restartCaptionRecognizer();
  }, [
    config.streamingModelPath,
    config.asr,
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

    void captionSessionRuntime.updateStyle(config).catch(logger.error);
  }, [
    config,
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
