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

    useEffect(() => {
        // Listen for segment updates from the main window
        const unlistenPromise = listen<TranscriptSegment[]>(CAPTION_EVENT_SEGMENTS, (event) => {
            setSegments(event.payload);
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

    // Auto-scroll to bottom
    useEffect(() => {
        if (containerRef.current) {
            containerRef.current.scrollTop = containerRef.current.scrollHeight;
        }
    }, [segments]);

    // Set transparent background for the window
    useEffect(() => {
        document.documentElement.style.background = 'transparent';
        document.body.style.background = 'transparent';

        return () => {
            document.documentElement.style.background = '';
            document.body.style.background = '';
        };
    }, []);

    const startDragging = () => {
        getCurrentWindow().startDragging();
    };

    return (
        <div className="caption-window-body">
            {/* Drag region for moving the window */}
            <div
                className="caption-drag-handle"
                data-tauri-drag-region
                onMouseDown={startDragging}
            >
                <div className="drag-indicator"></div>
            </div>

            <div className="live-caption-content" ref={containerRef}>
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
