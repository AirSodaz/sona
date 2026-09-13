import React from 'react';
import { useTranscriptSessionStore } from '../../stores/transcriptSessionStore';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { formatDisplayTime } from '../../utils/exportFormats';

/**
 * Ultra-subtle typing caret anticipation element displayed in the transcript editor.
 * Informs the user where the next transcribed element will appear with minimal perceptual load.
 */
export const TranscriptAnticipationIndicator = React.memo(
  function TranscriptAnticipationIndicator(): React.JSX.Element | null {
    const isRecording = useTranscriptStore((state) => state.isRecording);
    const isPaused = useTranscriptStore((state) => state.isPaused);
    const segments = useTranscriptSessionStore((state) => state.segments);

    // Only render when recording is active, not paused, and we have existing segments
    if (!isRecording || isPaused || segments.length === 0) {
      return null;
    }

    const lastSegment = segments[segments.length - 1];
    const isLastSegmentPartial = !lastSegment.isFinal;

    // When the last segment is partial, the inline caret inside SegmentTokens is already active.
    if (isLastSegmentPartial) {
      return null;
    }

    const nextStartTime = formatDisplayTime(lastSegment.end);

    return (
      <div className="transcript-ghost-segment" aria-hidden="true">
        <div className="transcript-ghost-main">
          {/* Subtle timestamp gutter in muted gray */}
          <div className="transcript-ghost-gutter">{nextStartTime}</div>

          {/* Pure breathing caret at the start of the next line */}
          <div className="transcript-ghost-content">
            <span className="transcript-caret-pulse" />
          </div>

          {/* Empty actions slot to preserve 3-column grid alignment */}
          <div style={{ width: '40px' }} />
        </div>
      </div>
    );
  }
);

TranscriptAnticipationIndicator.displayName = 'TranscriptAnticipationIndicator';
