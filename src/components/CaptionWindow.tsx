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
            if (containerRef.current) {
                // Measure content height
                const contentHeight = containerRef.current.scrollHeight;

                // Calculate total height needed:
                // Drag handle height (32px) + content height
                // Note: contentHeight includes padding due to box-sizing if padding is set on container
                // containerRef is .live-caption-content which has padding.
                const totalHeight = contentHeight + 32;

                // Get current window scale factor and size to preserve width
                const currentWindow = getCurrentWindow();
                const factor = await currentWindow.scaleFactor();
                const size = await currentWindow.innerSize();
                const logicalWidth = size.width / factor;

                // Set new size
                // We add a small buffer (e.g. 10px) to prevent scrollbars or tight fits if necessary,
                // but scrollHeight usually covers it.
                await currentWindow.setSize(new LogicalSize(logicalWidth, totalHeight));
            }
        };

        updateHeight();
    }, [segments]);

    const startDragging = () => {
        getCurrentWindow().startDragging();
    };

    return (
        <div className="caption-window-body" style={{ height: 'auto', minHeight: 'auto' }}>
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
                {segments.length === 0 ? (
                    <div className="caption-placeholder">Waiting for speech...</div>
                ) : (
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
