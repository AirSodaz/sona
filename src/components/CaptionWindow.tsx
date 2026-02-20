import { useEffect, useState, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { TranscriptSegment } from '../types/transcript';
import '../styles/index.css'; // Import styles to ensure variables are available

const CAPTION_EVENT_SEGMENTS = 'caption:segments';
const CAPTION_EVENT_CLOSE = 'caption:close';

/**
 * Root component for the always-on-top caption window.
 * Manages its own state by listening to Tauri events from the main window.
 */
export function CaptionWindow() {
    const [segments, setSegments] = useState<TranscriptSegment[]>([]);
    const containerRef = useRef<HTMLDivElement>(null);
    const rootRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        // Listen for segment updates from the main window
        const unlistenPromise = listen<TranscriptSegment[]>(CAPTION_EVENT_SEGMENTS, (event) => {
            // Only show the newest segment. New text must replace old text.
            const payload = event.payload;
            if (payload && payload.length > 0) {
                // Take the last segment from the list
                const lastSegment = payload[payload.length - 1];
                setSegments([lastSegment]);
            } else {
                setSegments([]);
            }
        });

        return () => {
            unlistenPromise.then((unlisten) => unlisten());
        };
    }, []);

    // Listen for close command
    useEffect(() => {
        const unlistenPromise = listen(CAPTION_EVENT_CLOSE, () => {
            console.log('[CaptionWindow] Received close command, closing window');
            getCurrentWindow().close();
        });

        return () => {
            unlistenPromise.then((unlisten) => unlisten());
        };
    }, []);

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
        if (segments.length > 0) {
            const timer = setTimeout(() => {
                setSegments([]);
            }, 3000);
            return () => clearTimeout(timer);
        }
    }, [segments]);

    // Dynamic height: Use ResizeObserver to ensure window always matches content height accurately
    useEffect(() => {
        if (!rootRef.current) return;

        let resizeTimeout: NodeJS.Timeout;

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

                    // Avoid unnecessary resize calls if the physical height matches
                    if (Math.abs(size.height - targetPhysicalHeight) > 1) {
                        const { PhysicalSize } = await import('@tauri-apps/api/dpi');
                        const targetSize = new PhysicalSize(size.width, targetPhysicalHeight);

                        // Use Min/Max Size Lock trick to prevent manual dragging by the user
                        await currentWindow.setMinSize(targetSize);
                        await currentWindow.setMaxSize(targetSize);
                        await currentWindow.setSize(targetSize);
                    }
                } catch (e) {
                    console.error("[CaptionWindow] Failed to resize window:", e);
                }
            }, 50);
        });

        observer.observe(rootRef.current);

        return () => {
            observer.disconnect();
            clearTimeout(resizeTimeout);
        };
    }, []);

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
                cursor: 'default'
            }}
        >
            {/* Drag region for moving the window */}
            <div
                className="caption-drag-handle"
                data-tauri-drag-region
                onMouseDown={startDragging}
            >
                <div className="drag-indicator"></div>
            </div>

            <div
                className="live-caption-content"
                ref={containerRef}
                style={{ maxHeight: 'none', height: 'auto' }}
            >
                {segments.length === 0 ? null : (
                    segments.map((seg) => (
                        <p
                            key={seg.id}
                            className={`live-caption-line ${seg.isFinal ? '' : 'partial'}`}
                        >
                            {seg.text}
                        </p>
                    ))
                )}
            </div>
        </div>
    );
}
