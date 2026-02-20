import { useEffect, useState, useRef, useLayoutEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
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

    // Dynamic height: The height of the window must change to fit the number of lines.
    useLayoutEffect(() => {
        const updateHeight = async () => {
            if (rootRef.current) {
                // Measure total height of the component root (includes drag handle + content + padding/borders)
                const totalHeight = rootRef.current.offsetHeight;

                // Get current window scale factor and size to preserve width
                const currentWindow = getCurrentWindow();
                const factor = await currentWindow.scaleFactor();
                const size = await currentWindow.innerSize();
                const logicalWidth = size.width / factor;

                // Set new size
                await currentWindow.setSize(new LogicalSize(logicalWidth, totalHeight));
            }
        };

        updateHeight();
    }, [segments]);

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
