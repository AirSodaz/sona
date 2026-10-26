import { useEffect, useMemo, useRef, useState } from 'react';
import { useAuxWindowState } from '../hooks/useAuxWindowState';
import {
  CAPTION_EVENT_STATE,
  CAPTION_WINDOW_LABEL,
  DEFAULT_CAPTION_WINDOW_STATE,
} from '../services/captionWindowService';
import { listen } from '../services/tauri/platform/events';
import { getCurrentWindow, PhysicalSize } from '../services/tauri/platform/windows';
import type { TranscriptSegment } from '../types/transcript';
import { needsWordBoundarySpace } from '../utils/cjkTypography';
import { hexToRgba } from '../utils/colorUtils';
import { logger } from '../utils/logger';
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

  const [clearedRevision, setClearedRevision] = useState<number | null>(null);
  const [translationError, setTranslationError] = useState<string | null>(null);
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

  // Clear text: Clear the window if no new text appears for 3 seconds.
  useEffect(() => {
    if (displaySegments.length > 0) {
      const revision = captionState.revision;
      const timer = setTimeout(() => {
        setClearedRevision(revision);
      }, 3000);
      return () => clearTimeout(timer);
    }
  }, [captionState.revision, displaySegments.length]);

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
  const displayMode = captionState.style.displayMode ?? 'bilingual';
  const bilingualOrder = captionState.style.bilingualOrder ?? 'original_first';
  const translationColor = captionState.style.translationColor ?? '#6ee7b7';
  const translationFontSize =
    captionState.style.translationFontSize ??
    Math.max(14, Math.round(captionState.style.fontSize * 0.85));

  return (
    <div
      className="caption-window-body"
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
      }}
    >
      {/* Drag region for moving the window */}
      <div className="caption-drag-handle" data-tauri-drag-region onMouseDown={startDragging}>
        <div className="drag-indicator" />
      </div>
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
          fontSize: `${captionState.style.fontSize}px`,
          lineHeight: 1.5,
          color: captionState.style.color,
        }}
      >
        {displaySegments.length === 0
          ? null
          : displaySegments.map((segment) => {
              const hasTranslation = Boolean(segment.translation || segment.draftTranslation);
              const originalLine = (
                <div
                  className="caption-original-line"
                  style={{
                    color: captionState.style.color,
                    marginTop:
                      bilingualOrder === 'translation_first' &&
                      displayMode === 'bilingual' &&
                      hasTranslation
                        ? '2px'
                        : 0,
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
                  className="caption-translation-line"
                  style={{
                    fontSize: `${translationFontSize}px`,
                    color: translationColor,
                    marginTop:
                      displayMode === 'bilingual' && bilingualOrder !== 'translation_first'
                        ? '2px'
                        : 0,
                  }}
                >
                  {displayMode === 'translation' && segment.speaker && (
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
                  style={{ marginBottom: displayMode === 'bilingual' ? '8px' : '4px' }}
                >
                  {displayMode === 'original' && originalLine}
                  {displayMode === 'translation' &&
                    (translationLine || (
                      <div
                        className="caption-translation-line caption-translation-pending"
                        style={{
                          fontSize: `${translationFontSize}px`,
                          color: translationColor,
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
                  {displayMode === 'bilingual' &&
                    (bilingualOrder === 'translation_first' ? (
                      <>
                        {translationLine}
                        {originalLine}
                      </>
                    ) : (
                      <>
                        {originalLine}
                        {translationLine}
                      </>
                    ))}
                </div>
              );
            })}
      </div>
    </div>
  );
}
