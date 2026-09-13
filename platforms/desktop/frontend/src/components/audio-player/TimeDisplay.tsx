import type React from 'react';
import { useEffect, useRef } from 'react';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import { formatDisplayTime } from '../../utils/exportFormats';

/**
 * Displays the current audio time.
 * Subscribes directly to store to avoid React re-renders on every tick.
 */
export function TimeDisplay(): React.JSX.Element {
  const spanRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    let lastDisplay = formatDisplayTime(useTranscriptPlaybackStore.getState().currentTime);
    if (spanRef.current) spanRef.current.textContent = lastDisplay;

    const unsubscribe = useTranscriptPlaybackStore.subscribe((state, prevState) => {
      if (state.currentTime === prevState.currentTime) return;

      const newDisplay = formatDisplayTime(state.currentTime);
      if (newDisplay !== lastDisplay) {
        lastDisplay = newDisplay;
        if (spanRef.current) {
          spanRef.current.textContent = newDisplay;
        }
      }
    });
    return unsubscribe;
  }, []);

  return (
    <span ref={spanRef} className="audio-time">
      {formatDisplayTime(useTranscriptPlaybackStore.getState().currentTime)}
    </span>
  );
}
