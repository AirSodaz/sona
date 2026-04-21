import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { TranscriptSegment } from '../types/transcript';
import '../styles/index.css';
import { logger } from '../utils/logger';
import {
    CAPTION_EVENT_STATE,
    CAPTION_WINDOW_LABEL,
    DEFAULT_CAPTION_WINDOW_STATE,
} from '../services/captionWindowService';
import { useAuxWindowState } from '../hooks/useAuxWindowState';

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

    const [displaySegments, setDisplaySegments] = useState<TranscriptSegment[]>([]);

    // Track target width to enforce it during height resizing
    const containerRef = useRef<HTMLDivElement>(null);
    const rootRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        setDisplaySegments(captionState.segments);
    }, [captionState.revision, captionState.segments]);

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
            const timer = setTimeout(() => {
                setDisplaySegments([]);
            }, 3000);
            return () => clearTimeout(timer);
        }
    }, [displaySegments]);

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
                        const { PhysicalSize } = await import('@tauri-apps/api/dpi');
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

    return (
        <div
            className="caption-window-body"
            ref={rootRef}
            style={{
                height: 'auto',
                minHeight: 'auto',
                userSelect: 'none',
                cursor: 'default',
                background: `rgba(0, 0, 0, ${captionState.style.backgroundOpacity})`,
            }}
        >
            {/* Drag region for moving the window */}
            <div
                className="caption-drag-handle"
                data-tauri-drag-region
                onMouseDown={startDragging}
            >
                <div className="drag-indicator" />
            </div>

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
                {displaySegments.length === 0 ? null : (
                    displaySegments.map((segment) => (
                        <p
                            key={segment.id}
                            className={`live-caption-line ${segment.isFinal ? '' : 'partial'}`}
                            style={{ color: captionState.style.color }}
                        >
                            {typeof segment.text === 'string' ? segment.text : ''}
                        </p>
                    ))
                )}
            </div>
        </div>
    );
}
