import { emit, listen } from '@tauri-apps/api/event';
import { X } from 'lucide-react';
import { type CSSProperties, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import { useAuxWindowTheme } from '../hooks/useAuxWindowTheme';
import { TauriEvent } from '../services/tauri/events';
import { getCurrentWindow, PhysicalSize } from '../services/tauri/platform/windows';
import {
  DEFAULT_VOICE_TYPING_OVERLAY_STATE,
  VOICE_TYPING_EVENT_TEXT,
  VOICE_TYPING_WINDOW_LABEL,
  VOICE_TYPING_WINDOW_WIDTH,
} from '../services/voiceTypingWindowService';
import { logger } from '../utils/logger';

const WAVEFORM_WEIGHTS = [0.45, 0.75, 1.0, 0.75, 0.45] as const;
const MIN_BAR_HEIGHT = 4;
const MAX_BAR_HEIGHT = 18;

const OVERLAY_ROOT_PADDING = {
  top: 4,
  right: 4,
  bottom: 20,
  left: 4,
};

const baseContainerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  padding: '10px 14px',
  borderRadius: 'var(--radius-lg)',
  fontSize: '14px',
  maxWidth: `${VOICE_TYPING_WINDOW_WIDTH - OVERLAY_ROOT_PADDING.left - OVERLAY_ROOT_PADDING.right}px`,
  transition:
    'background 120ms ease, border-color 120ms ease, box-shadow 120ms ease, color 120ms ease',
} satisfies CSSProperties;

async function resizeVoiceTypingWindow(rootElement: HTMLDivElement | null) {
  if (!rootElement) {
    return;
  }

  const totalHeight = Math.ceil(rootElement.getBoundingClientRect().height);
  if (totalHeight <= 0) {
    return;
  }

  try {
    const currentWindow = getCurrentWindow();
    const factor = await currentWindow.scaleFactor();
    const size = await currentWindow.innerSize();
    const targetPhysicalHeight = Math.ceil(totalHeight * factor);
    const targetPhysicalWidth = Math.ceil(VOICE_TYPING_WINDOW_WIDTH * factor);

    if (
      Math.abs(size.height - targetPhysicalHeight) <= 1 &&
      Math.abs(size.width - targetPhysicalWidth) <= 1
    ) {
      return;
    }

    await currentWindow.setSize(new PhysicalSize(targetPhysicalWidth, targetPhysicalHeight));
  } catch (error) {
    logger.error('[VoiceTypingOverlay] Failed to resize window:', error);
  }
}

export function VoiceTypingOverlay() {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const resolvedTheme = useAuxWindowTheme();
  const [peakLevel, setPeakLevel] = useState<number>(0);
  const overlayState = useAuxWindowState({
    label: VOICE_TYPING_WINDOW_LABEL,
    eventName: VOICE_TYPING_EVENT_TEXT,
    defaultState: DEFAULT_VOICE_TYPING_OVERLAY_STATE,
    onStateApplied: (payload, source) => {
      void logger.info('[VoiceTypingOverlay] Applied overlay state', {
        source,
        sessionId: payload.sessionId,
        revision: payload.revision,
        phase: payload.phase,
        segmentId: payload.segmentId ?? null,
        isFinal: payload.isFinal ?? null,
        textLength: payload.text.length,
      });
    },
  });

  const handleCancel = useCallback(async () => {
    try {
      if (typeof emit === 'function') {
        await emit(TauriEvent.auxWindow.voiceTypingCancel);
      }
    } catch (error) {
      logger.warn('[VoiceTypingOverlay] Failed to emit cancel event:', error);
    }
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        void handleCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleCancel]);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;

    listen<number>(TauriEvent.audio.microphonePeak, (event) => {
      if (!isMounted) return;
      const raw = typeof event.payload === 'number' ? Math.abs(event.payload) : 0;
      const normalized = Math.min(1, Math.max(0, raw / 32767));
      setPeakLevel(normalized);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err) => {
        logger.debug('[VoiceTypingOverlay] Could not listen to microphonePeak event', err);
      });

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    const previousDocumentBackground = document.documentElement.style.background;
    const previousBodyBackground = document.body.style.background;

    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';

    return () => {
      document.documentElement.style.background = previousDocumentBackground;
      document.body.style.background = previousBodyBackground;
    };
  }, []);

  useEffect(() => {
    if (!rootRef.current) {
      return;
    }

    const observer = new ResizeObserver(() => {
      void resizeVoiceTypingWindow(rootRef.current);
    });

    observer.observe(rootRef.current);
    void resizeVoiceTypingWindow(rootRef.current);

    return () => {
      observer.disconnect();
    };
  }, []);

  const { phase, text } = overlayState;
  const isSegment = phase === 'segment' && text.trim().length > 0;
  const isError = phase === 'error';
  const displayText =
    isSegment || isError
      ? text
      : phase === 'preparing'
        ? t('common.preparing')
        : t('common.listening');

  useEffect(() => {
    if (phase !== 'segment') {
      return;
    }

    void logger.info('[VoiceTypingOverlay] Rendered segment state', {
      sessionId: overlayState.sessionId,
      revision: overlayState.revision,
      textLength: text.length,
      renderedPhase: isSegment ? 'segment' : 'listening',
    });
  }, [isSegment, overlayState.revision, overlayState.sessionId, phase, text]);

  useEffect(() => {
    if (displayText !== undefined || phase !== undefined || resolvedTheme !== undefined) {
      void resizeVoiceTypingWindow(rootRef.current);
    }
  }, [displayText, phase, resolvedTheme]);

  let containerStyle: CSSProperties;

  if (isSegment) {
    containerStyle = {
      ...baseContainerStyle,
      background: 'var(--color-bg-elevated)',
      color: 'var(--color-text-primary)',
      border: '1px solid var(--color-border-hover)',
      boxShadow: resolvedTheme === 'dark' ? '0 16px 32px rgba(0, 0, 0, 0.36)' : 'var(--shadow-xl)',
    };
  } else if (isError) {
    containerStyle = {
      ...baseContainerStyle,
      background: 'rgba(127, 29, 29, 0.92)',
      color: '#ffffff',
      border: '1px solid rgba(248, 113, 113, 0.35)',
    };
  } else {
    containerStyle = {
      ...baseContainerStyle,
      background: 'var(--color-bg-elevated)',
      color: 'var(--color-text-primary)',
      border: '1px solid var(--color-border-hover)',
      boxShadow: resolvedTheme === 'dark' ? '0 16px 32px rgba(0, 0, 0, 0.36)' : 'var(--shadow-xl)',
    };
  }

  const isSpeaking = isSegment || peakLevel > 0.05;
  const barHeights = WAVEFORM_WEIGHTS.map((weight, i) => {
    if (isError) return 4;
    if (isSpeaking) {
      const boost = Math.min(1, Math.max(peakLevel * 2.2, isSegment ? 0.35 : 0.15));
      return Math.round(MIN_BAR_HEIGHT + (MAX_BAR_HEIGHT - MIN_BAR_HEIGHT) * boost * weight);
    }
    const idlePattern = [4, 7, 10, 7, 4];
    return idlePattern[i];
  });
  return (
    <div
      data-testid="voice-typing-overlay-root"
      ref={rootRef}
      style={{
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'flex-start',
        background: 'transparent',
        overflow: 'visible',
        width: 'fit-content',
        height: 'fit-content',
        padding: `${OVERLAY_ROOT_PADDING.top}px ${OVERLAY_ROOT_PADDING.right}px ${OVERLAY_ROOT_PADDING.bottom}px ${OVERLAY_ROOT_PADDING.left}px`,
      }}
    >
      <div data-testid="voice-typing-bubble" style={containerStyle}>
        {/* 5-bar Waveform Visualizer */}
        <div
          data-testid="voice-typing-waveform"
          aria-label="audio waveform"
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '2.5px',
            height: '18px',
            flexShrink: 0,
          }}
        >
          {barHeights.map((barHeight, idx) => (
            <div
              key={idx}
              style={{
                width: '3px',
                height: `${barHeight}px`,
                borderRadius: '999px',
                background: isError
                  ? '#fca5a5'
                  : isSegment
                    ? 'linear-gradient(180deg, #34d399 0%, #22c55e 100%)'
                    : '#4ade80',
                transition: 'height 80ms cubic-bezier(0.4, 0, 0.2, 1)',
              }}
            />
          ))}
        </div>
        <span
          style={{
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            fontWeight: isSegment ? 600 : 500,
            letterSpacing: isSegment ? '0.01em' : 'normal',
          }}
        >
          {displayText}
        </span>
        {/* Quick cancel button */}
        <button
          type="button"
          data-testid="voice-typing-cancel-btn"
          aria-label={t('common.cancel', { defaultValue: 'Cancel' })}
          title={t('common.cancel', { defaultValue: 'Cancel' })}
          onClick={handleCancel}
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            width: '18px',
            height: '18px',
            padding: 0,
            marginLeft: '2px',
            border: 'none',
            background: 'transparent',
            color: isError ? '#fca5a5' : 'var(--color-text-muted)',
            borderRadius: '999px',
            cursor: 'pointer',
            opacity: 0.6,
            flexShrink: 0,
            transition: 'opacity 120ms ease, background 120ms ease',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.opacity = '1';
            e.currentTarget.style.background = 'var(--color-bg-hover)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.opacity = '0.6';
            e.currentTarget.style.background = 'transparent';
          }}
        >
          <X size={12} />
        </button>
      </div>
    </div>
  );
}
