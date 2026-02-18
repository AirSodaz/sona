import React, { useRef, useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';

/** Props for the LiveCaptionOverlay component. */
interface LiveCaptionOverlayProps {
    /** Maximum number of segments to display. Defaults to 3. */
    maxLines?: number;
}

/**
 * Floating overlay that displays live transcription segments as subtitles.
 *
 * Shows the most recent segments at the bottom-center of the viewport.
 * Partial (in-progress) segments are displayed in italic.
 *
 * @param props Component props.
 * @return The rendered overlay, or null if no segments exist.
 */
export function LiveCaptionOverlay({ maxLines = 3 }: LiveCaptionOverlayProps): React.ReactElement | null {
    const segments = useTranscriptStore((state) => state.segments);
    const containerRef = useRef<HTMLDivElement>(null);

    // Auto-scroll to bottom when new segments arrive
    useEffect(() => {
        if (containerRef.current) {
            containerRef.current.scrollTop = containerRef.current.scrollHeight;
        }
    }, [segments]);

    const visibleSegments = segments.slice(-maxLines);

    if (visibleSegments.length === 0) {
        return null;
    }

    return (
        <div className="live-caption-overlay" ref={containerRef} role="status" aria-live="polite">
            {visibleSegments.map((seg) => (
                <p
                    key={seg.id}
                    className={`live-caption-line ${seg.isFinal ? '' : 'partial'}`}
                >
                    {seg.text}
                </p>
            ))}
        </div>
    );
}
