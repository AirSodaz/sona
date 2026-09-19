import { Copy, History, Sparkles, X } from 'lucide-react';
import { type CSSProperties, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import { useAuxWindowTheme } from '../hooks/useAuxWindowTheme';
import { TauriEvent } from '../services/tauri/events';
import { emit, listen } from '../services/tauri/platform/events';
import { getCurrentWindow, PhysicalSize } from '../services/tauri/platform/windows';
import {
  DEFAULT_VOICE_TYPING_OVERLAY_STATE,
  VOICE_TYPING_EVENT_TEXT,
  VOICE_TYPING_WINDOW_LABEL,
  VOICE_TYPING_WINDOW_WIDTH,
} from '../services/voiceTypingWindowService';
import { useVoiceTypingHistoryStore } from '../stores/voiceTypingHistoryStore';
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

  const handleReinject = useCallback(
    async (text: string) => {
      try {
        if (typeof emit === 'function') {
          await emit(TauriEvent.auxWindow.voiceTypingReinject, { text });
        }
      } catch (error) {
        logger.warn('[VoiceTypingOverlay] Failed to emit reinject event:', error);
      }
      void handleCancel();
    },
    [handleCancel]
  );

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        void handleCancel();
        return;
      }

      if (overlayState.phase === 'recall' && event.key >= '1' && event.key <= '5') {
        const idx = Number.parseInt(event.key, 10) - 1;
        const items = useVoiceTypingHistoryStore.getState().items;
        if (items[idx]?.injectedText) {
          event.preventDefault();
          void handleReinject(items[idx].injectedText);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleCancel, handleReinject, overlayState.phase]);

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
  const isPolishing = phase === 'polishing';
  const isSelection = Boolean(overlayState.hasSelection);
  const displayText = isPolishing
    ? isSelection
      ? t('voice_typing.polishing_selection', { defaultValue: 'AI 选区重写中...' })
      : t('voice_typing.polishing', { defaultValue: 'AI 润色中...' })
    : isSegment || isError
      ? text
      : phase === 'preparing'
        ? t('common.preparing')
        : isSelection
          ? t('voice_typing.listening_selection', { defaultValue: '请口述修改指令...' })
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

  if (isPolishing) {
    containerStyle = {
      ...baseContainerStyle,
      background: 'var(--color-bg-elevated)',
      color: 'var(--color-text-primary)',
      border: '1px solid rgba(168, 85, 247, 0.6)',
      boxShadow: '0 16px 32px rgba(168, 85, 247, 0.28)',
    };
  } else if (isSegment) {
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
    if (isPolishing) {
      const polishPattern = [6, 12, 16, 12, 6];
      return polishPattern[i];
    }
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
      {overlayState.phase === 'recall' ? (
        <div
          data-testid="voice-typing-recall-drawer"
          style={{
            ...baseContainerStyle,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'stretch',
            gap: '8px',
            padding: '12px 14px',
            width: '380px',
            maxWidth: '380px',
            background: 'var(--color-bg-elevated)',
            color: 'var(--color-text-primary)',
            border: '1px solid var(--color-border-hover)',
            boxShadow:
              resolvedTheme === 'dark' ? '0 16px 32px rgba(0, 0, 0, 0.45)' : 'var(--shadow-xl)',
          }}
        >
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                fontWeight: 600,
                fontSize: '12px',
              }}
            >
              <History size={14} color="var(--color-accent-blue, #3b82f6)" />
              <span>
                {t('voice_typing.quick_recall_title', {
                  defaultValue: '历史重输 (1-5 键注入)',
                })}
              </span>
            </div>
            <button
              type="button"
              data-testid="voice-typing-recall-close-btn"
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
                border: 'none',
                background: 'transparent',
                color: 'var(--color-text-muted)',
                cursor: 'pointer',
              }}
            >
              <X size={12} />
            </button>
          </div>

          {(() => {
            const historyItems = useVoiceTypingHistoryStore.getState().items.slice(0, 5);
            if (historyItems.length === 0) {
              return (
                <div
                  data-testid="voice-typing-recall-empty"
                  style={{
                    fontSize: '12px',
                    color: 'var(--color-text-muted)',
                    textAlign: 'center',
                    padding: '12px 0',
                  }}
                >
                  {t('voice_typing.quick_recall_empty', {
                    defaultValue: '暂无历史语音输入记录',
                  })}
                </div>
              );
            }

            return (
              <div
                data-testid="voice-typing-recall-list"
                style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}
              >
                {historyItems.map((item, idx) => (
                  <div
                    key={item.id}
                    data-testid={`quick-recall-item-${idx}`}
                    onClick={() => void handleReinject(item.injectedText)}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      gap: '8px',
                      padding: '6px 8px',
                      borderRadius: 'var(--radius-sm, 6px)',
                      background: 'var(--color-bg-secondary)',
                      cursor: 'pointer',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px',
                        overflow: 'hidden',
                      }}
                    >
                      <span
                        style={{
                          fontSize: '11px',
                          fontWeight: 700,
                          padding: '1px 5px',
                          borderRadius: '3px',
                          background: 'var(--color-bg-elevated)',
                          border: '1px solid var(--color-border)',
                          color: 'var(--color-text-muted)',
                          flexShrink: 0,
                        }}
                      >
                        {idx + 1}
                      </span>
                      <span
                        style={{
                          fontSize: '12px',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {item.injectedText}
                      </span>
                    </div>
                    <button
                      type="button"
                      title={t('common.copy', { defaultValue: 'Copy' })}
                      onClick={(e) => {
                        e.stopPropagation();
                        void navigator.clipboard.writeText(item.injectedText);
                      }}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        padding: '2px',
                        border: 'none',
                        background: 'transparent',
                        color: 'var(--color-text-muted)',
                        cursor: 'pointer',
                        flexShrink: 0,
                      }}
                    >
                      <Copy size={11} />
                    </button>
                  </div>
                ))}
              </div>
            );
          })()}
        </div>
      ) : (
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
                    : isPolishing
                      ? 'linear-gradient(180deg, #c084fc 0%, #9333ea 100%)'
                      : isSegment
                        ? 'linear-gradient(180deg, #34d399 0%, #22c55e 100%)'
                        : '#4ade80',
                }}
              />
            ))}
          </div>
          {overlayState.hasSelection && (
            <span
              data-testid="voice-typing-selection-badge"
              style={{
                fontSize: '11px',
                fontWeight: 600,
                padding: '1px 6px',
                borderRadius: '4px',
                background: 'rgba(59, 130, 246, 0.15)',
                color: 'var(--color-accent-blue, #3b82f6)',
                flexShrink: 0,
              }}
            >
              {t('voice_typing.selection_badge', { defaultValue: '选区重写' })}
            </span>
          )}
          {isPolishing && <Sparkles size={14} color="#a855f7" style={{ flexShrink: 0 }} />}
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
      )}
    </div>
  );
}
