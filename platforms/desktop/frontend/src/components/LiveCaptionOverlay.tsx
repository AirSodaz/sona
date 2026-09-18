import type React from 'react';
import { useEffect, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useCaptionConfig } from '../stores/configStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { hexToRgba } from '../utils/colorUtils';

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
export function LiveCaptionOverlay({
  maxLines = 3,
}: LiveCaptionOverlayProps): React.ReactElement | null {
  // Subscribe only to the visible slice; useShallow prevents re-renders
  // when the slice content hasn't changed (shallow comparison).
  const visibleSegments = useTranscriptSessionStore(
    useShallow((state) => state.segments.slice(-maxLines))
  );
  const { captionBackgroundOpacity = 0.6, captionBackgroundColor = '#000000' } = useCaptionConfig();
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new segments arrive
  useEffect(() => {
    if (containerRef.current && visibleSegments.length > 0) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [visibleSegments]);

  if (visibleSegments.length === 0) {
    return null;
  }

  return (
    <div
      className="live-caption-overlay"
      ref={containerRef}
      role="status"
      aria-live="polite"
      style={{
        background: hexToRgba(captionBackgroundColor, captionBackgroundOpacity + 0.18),
      }}
    >
      {visibleSegments.map((seg) => (
        <p key={seg.id} className={`live-caption-line ${seg.isFinal ? '' : 'partial'}`}>
          {seg.speaker && <span className="live-caption-speaker">[{seg.speaker.label}]</span>}
          {seg.text}
        </p>
      ))}
    </div>
  );
}
