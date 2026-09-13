import type React from 'react';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { formatDisplayTime } from '../../utils/exportFormats';

/**
 * Displays the current audio time.
 */
export function TimeDisplay(): React.JSX.Element {
  const currentTime = useTranscriptPlaybackStore((state) => state.currentTime);

  return <span className="audio-time">{formatDisplayTime(currentTime)}</span>;
}
