import { Copy, Lock } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import {
  CAPTION_EVENT_STATE,
  CAPTION_WINDOW_LABEL,
  type CaptionWindowStyle,
  DEFAULT_CAPTION_WINDOW_STATE,
} from '../services/captionWindowService';
import { emit, listen } from '../services/tauri/platform/events';
import { getCurrentWindow, PhysicalSize } from '../services/tauri/platform/windows';
import type { TranscriptSegment } from '../types/transcript';
import { needsWordBoundarySpace } from '../utils/cjkTypography';
import { hexToRgba } from '../utils/colorUtils';
import { logger } from '../utils/logger';

export function computeCaptionTextStyle(
  style: CaptionWindowStyle,
  _isPrimary: boolean,
  isTranslation: boolean
): React.CSSProperties {
  const strokeEnabled = Boolean(style.textStroke) && (style.strokeWidth ?? 0) > 0;
  const strokeWidth = style.strokeWidth ?? 0;
  const strokeColor = style.strokeColor ?? '#000000';
  const shadowEnabled = style.textShadow !== false;

  const shadows: string[] = [];
  if (strokeEnabled) {
    shadows.push(
      `-1px -1px 0 ${strokeColor}`,
      `1px -1px 0 ${strokeColor}`,
      `-1px 1px 0 ${strokeColor}`,
      `1px 1px 0 ${strokeColor}`
    );
  }
  if (shadowEnabled) {
    const diffuseShadow = isTranslation
      ? '0 1px 2px rgba(0, 0, 0, 0.4)'
      : '0 1px 3px rgba(0, 0, 0, 0.55)';
    shadows.push(diffuseShadow);
  }

  return {
    ...(strokeEnabled ? { WebkitTextStroke: `${strokeWidth}px ${strokeColor}` } : {}),
    ...(shadows.length > 0 ? { textShadow: shadows.join(', ') } : {}),
  };
}
/**
 * Root component for the always-on-top caption window.
 * Manages its own state by listening to Tauri events from the main window.
 */
export function CaptionWindow() {
  const captionState = useAuxWindowState({
    label: CAPTION_WINDOW_LABEL,
    eventName: CAPTION_EVENT_STATE,
    defaultState: DEFAULT_CAPTION_WINDOW_STATE,
    onStateApplied: (state, source) => {
      void logger.info('[CaptionWindow] Applied caption state', {
        source,
        revision: state.revision,
        segmentCount: state.segments.length,
        width: state.style.width,
        fontSize: state.style.fontSize,
      });
    },
  });

  const { t } = useTranslation();
  const [clearedRevision, setClearedRevision] = useState<number | null>(null);
  const [isFadingOut, setIsFadingOut] = useState(false);
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  const [translationError, setTranslationError] = useState<string | null>(null);

  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lockTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (toastTimerRef.current) {
        clearTimeout(toastTimerRef.current);
      }
      if (lockTimerRef.current) {
        clearTimeout(lockTimerRef.current);
      }
    };
  }, []);

  // Track target width to enforce it during height resizing
  const containerRef = useRef<HTMLDivElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  const displaySegments = useMemo<TranscriptSegment[]>(
    () => (clearedRevision === captionState.revision ? [] : captionState.segments),
    [captionState.revision, captionState.segments, clearedRevision]
  );

  // Set transparent background for the window
  useEffect(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';

    return () => {
      document.documentElement.style.background = '';
      document.body.style.background = '';
    };
  }, []);

  // Clear text: Fade out smoothly after disappearDelay, then clear after 300ms transition.
  useEffect(() => {
    if (fadeTimerRef.current) {
      clearTimeout(fadeTimerRef.current);
      fadeTimerRef.current = null;
    }
    if (clearTimerRef.current) {
      clearTimeout(clearTimerRef.current);
      clearTimerRef.current = null;
    }
    setIsFadingOut(false);

    if (displaySegments.length > 0) {
      const revision = captionState.revision;
      const delayMs = (captionState.style.disappearDelay ?? 4.0) * 1000;
      fadeTimerRef.current = setTimeout(() => {
        setIsFadingOut(true);
        clearTimerRef.current = setTimeout(() => {
          setClearedRevision(revision);
          setIsFadingOut(false);
        }, 300);
      }, delayMs);
    }

    return () => {
      if (fadeTimerRef.current) {
        clearTimeout(fadeTimerRef.current);
        fadeTimerRef.current = null;
      }
      if (clearTimerRef.current) {
        clearTimeout(clearTimerRef.current);
        clearTimerRef.current = null;
      }
    };
  }, [captionState.revision, captionState.style.disappearDelay, displaySegments.length]);

  // Reset error when revision increments after window was cleared or on session restart
  useEffect(() => {
    if (clearedRevision !== null && captionState.revision !== clearedRevision) {
      setClearedRevision(null);
      setTranslationError(null);
    }
  }, [captionState.revision, clearedRevision]);

  // Clear error state when new translations arrive successfully
  useEffect(() => {
    if (
      translationError &&
      displaySegments.some((seg) => Boolean(seg.translation || seg.draftTranslation))
    ) {
      setTranslationError(null);
    }
  }, [displaySegments, translationError]);

  // Listen for realtime translation model loading errors from backend
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<{ error: string; engine: string }>('realtime-translation-error', (event) => {
      void logger.warn('[CaptionWindow] Realtime translation error received:', event.payload);
      setTranslationError(event.payload.error || 'Translation model failed to load');
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Dynamic height: Use ResizeObserver to ensure window always matches content height accurately
  useEffect(() => {
    if (!rootRef.current) return;

    let resizeTimeout: ReturnType<typeof setTimeout>;

    const observer = new ResizeObserver(() => {
      // Debounce the resize to prevent spamming IPC calls and ensure final size applies
      clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(async () => {
        if (!rootRef.current) return;
        try {
          // Measure total height of the component root (using getBoundingClientRect for subpixel accuracy)
          const totalHeight = Math.ceil(rootRef.current.getBoundingClientRect().height);

          // Get current window scale factor and size to preserve width and accurately set height
          const currentWindow = getCurrentWindow();
          const factor = await currentWindow.scaleFactor();
          const size = await currentWindow.innerSize();

          const targetPhysicalHeight = Math.ceil(totalHeight * factor);
          const widthToUse = Math.ceil(captionState.style.width * factor);

          // Avoid unnecessary resize calls if the physical height/width matches
          if (
            Math.abs(size.height - targetPhysicalHeight) > 1 ||
            Math.abs(size.width - widthToUse) > 1
          ) {
            const targetSize = new PhysicalSize(widthToUse, targetPhysicalHeight);

            // Use Min/Max Size Lock trick to prevent manual dragging by the user
            await currentWindow.setMinSize(targetSize);
            await currentWindow.setMaxSize(targetSize);
            await currentWindow.setSize(targetSize);
          }
        } catch (error) {
          logger.error('[CaptionWindow] Failed to resize window:', error);
        }
      }, 50);
    });

    observer.observe(rootRef.current);

    return () => {
      observer.disconnect();
      clearTimeout(resizeTimeout);
    };
  }, [captionState.style.width]);

  const startDragging = () => {
    getCurrentWindow().startDragging();
  };

  const showToast = (message: string) => {
    clearTimeout(toastTimerRef.current ?? undefined);
    setToastMessage(message);
    toastTimerRef.current = setTimeout(() => {
      setToastMessage(null);
    }, 2000);
  };

  const effectivePriority =
    captionState.style.displayPriority ??
    (captionState.style.displayMode === 'translation'
      ? 'translation_only'
      : captionState.style.displayMode === 'original'
        ? 'original_only'
        : captionState.style.bilingualOrder === 'translation_first'
          ? 'translation_main'
          : 'original_main');

  const textAlign = captionState.style.textAlign ?? 'center';
  const translationColor = captionState.style.translationColor ?? '#6ee7b7';
  const primaryFontSize = captionState.style.fontSize;
  const secondaryFontSize =
    captionState.style.translationFontSize ??
    Math.max(14, Math.round(captionState.style.fontSize * 0.75));

  const isTranslationMain = effectivePriority === 'translation_main';
  const isOriginalMain = effectivePriority === 'original_main';
  const isTranslationOnly = effectivePriority === 'translation_only';
  const isOriginalOnly = effectivePriority === 'original_only';

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (displaySegments.length === 0) return;
    const latest = displaySegments[displaySegments.length - 1];
    let textToCopy = '';
    if (isOriginalOnly) {
      textToCopy = latest.text || '';
    } else if (isOriginalMain) {
      textToCopy = latest.text || latest.translation || latest.draftTranslation || '';
    } else {
      textToCopy = latest.translation || latest.draftTranslation || latest.text || '';
    }
    if (!textToCopy) return;
    try {
      await navigator.clipboard.writeText(textToCopy);
      showToast(t('caption.copied', { defaultValue: '已复制' }));
    } catch (err) {
      logger.warn('[CaptionWindow] Failed to copy to clipboard', err);
    }
  };

  const handleLock = async (e: React.MouseEvent) => {
    e.stopPropagation();
    showToast(t('caption.locked_click_through', { defaultValue: '已开启穿透 (Alt+C 解锁)' }));
    if (lockTimerRef.current) {
      clearTimeout(lockTimerRef.current);
    }
    lockTimerRef.current = setTimeout(async () => {
      try {
        await emit('caption:lock-toggle', true);
        const currentWindow = getCurrentWindow();
        await currentWindow.setIgnoreCursorEvents(true).catch(() => {});
      } catch (err) {
        logger.warn('[CaptionWindow] Failed to set ignore cursor events', err);
      }
    }, 1500);
  };
  return (
    <div
      className={`caption-window-body ${isFadingOut ? 'fading-out' : ''}`}
      ref={rootRef}
      style={{
        height: 'auto',
        minHeight: 'auto',
        userSelect: 'none',
        cursor: 'default',
        background: hexToRgba(
          captionState.style.backgroundColor,
          captionState.style.backgroundOpacity
        ),
        ...(captionState.style.backgroundBlur && captionState.style.backgroundOpacity > 0
          ? {
              backdropFilter: `blur(${captionState.style.backgroundBlurRadius ?? 12}px)`,
              WebkitBackdropFilter: `blur(${captionState.style.backgroundBlurRadius ?? 12}px)`,
            }
          : {}),
      }}
    >
      {/* Drag region for moving the window & hover mini toolbar */}
      <div className="caption-drag-handle" data-tauri-drag-region onMouseDown={startDragging}>
        <div className="drag-indicator" />
        <div className="caption-handle-actions" data-tauri-drag-region="false">
          <button
            type="button"
            className="caption-action-btn"
            data-tauri-drag-region="false"
            title={t('caption.copy', { defaultValue: '复制字幕' })}
            onClick={handleCopy}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <Copy size={14} />
          </button>
          <button
            type="button"
            className="caption-action-btn"
            data-tauri-drag-region="false"
            title={t('caption.lock', { defaultValue: '锁定穿透 (Alt+C)' })}
            onClick={handleLock}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <Lock size={14} />
          </button>
        </div>
      </div>

      {toastMessage && <div className="caption-toast-banner">{toastMessage}</div>}

      {translationError && (
        <div
          className="caption-error-banner"
          style={{
            padding: '4px 8px',
            marginBottom: '4px',
            backgroundColor: 'rgba(239, 68, 68, 0.85)',
            color: '#ffffff',
            borderRadius: '4px',
            fontSize: '12px',
            textAlign: 'center',
            backdropFilter: 'blur(4px)',
          }}
        >
          {translationError}
        </div>
      )}

      <div
        className="live-caption-content"
        ref={containerRef}
        style={{
          maxHeight: 'none',
          height: 'auto',
          fontSize: `${primaryFontSize}px`,
          lineHeight: 1.5,
          color: captionState.style.color,
          textAlign,
        }}
      >
        {displaySegments.length === 0
          ? null
          : displaySegments.map((segment) => {
              const hasTranslation = Boolean(segment.translation || segment.draftTranslation);

              const originalStyle: React.CSSProperties = {
                color: captionState.style.color,
                fontSize: `${isTranslationMain && hasTranslation ? secondaryFontSize : primaryFontSize}px`,
                opacity: isTranslationMain && hasTranslation ? 0.8 : 1,
                textAlign,
                ...computeCaptionTextStyle(
                  captionState.style,
                  !(isTranslationMain && hasTranslation),
                  false
                ),
              };

              const translationStyle: React.CSSProperties = {
                color: translationColor,
                fontSize: `${isOriginalMain ? secondaryFontSize : primaryFontSize}px`,
                opacity: 1,
                textAlign,
                ...computeCaptionTextStyle(captionState.style, !isOriginalMain, true),
              };

              const originalLine = (
                <div
                  className={
                    isTranslationMain && hasTranslation
                      ? 'caption-original-line caption-secondary-line'
                      : 'caption-original-line caption-primary-line'
                  }
                  style={{
                    ...originalStyle,
                    marginTop: isTranslationMain && hasTranslation ? '2px' : 0,
                  }}
                >
                  {segment.speaker && (
                    <span className="live-caption-speaker">[{segment.speaker.label}]</span>
                  )}
                  <span>{typeof segment.text === 'string' ? segment.text : ''}</span>
                </div>
              );

              const translationLine = hasTranslation ? (
                <div
                  className={
                    isOriginalMain
                      ? 'caption-translation-line caption-secondary-line'
                      : 'caption-translation-line caption-primary-line'
                  }
                  style={{
                    ...translationStyle,
                    marginTop: isOriginalMain ? '2px' : 0,
                  }}
                >
                  {isTranslationOnly && segment.speaker && (
                    <span className="live-caption-speaker">[{segment.speaker.label}]</span>
                  )}
                  <span>{segment.translation ?? ''}</span>
                  {segment.draftTranslation && (
                    <>
                      {needsWordBoundarySpace(segment.translation, segment.draftTranslation)
                        ? ' '
                        : null}
                      <span className="caption-draft-tail" style={{ opacity: 0.6 }}>
                        {segment.draftTranslation}
                      </span>
                    </>
                  )}
                </div>
              ) : null;

              return (
                <div
                  key={segment.id}
                  className={`live-caption-line ${segment.isFinal ? '' : 'partial'}`}
                  style={{
                    marginBottom:
                      isTranslationOnly || isOriginalOnly || !hasTranslation ? '4px' : '8px',
                    textAlign,
                  }}
                >
                  {isOriginalOnly && originalLine}
                  {isTranslationOnly &&
                    (translationLine || (
                      <div
                        className="caption-translation-line caption-translation-pending"
                        style={{
                          ...translationStyle,
                          opacity: 0.6,
                          fontStyle: 'italic',
                        }}
                      >
                        {segment.speaker && (
                          <span className="live-caption-speaker">[{segment.speaker.label}]</span>
                        )}
                        <span>{segment.text}</span>
                      </div>
                    ))}
                  {isTranslationMain && (
                    <>
                      {translationLine || originalLine}
                      {translationLine && originalLine}
                    </>
                  )}
                  {isOriginalMain && (
                    <>
                      {originalLine}
                      {translationLine}
                    </>
                  )}
                </div>
              );
            })}
      </div>
    </div>
  );
}
