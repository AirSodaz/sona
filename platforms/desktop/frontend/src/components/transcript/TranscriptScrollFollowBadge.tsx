import { ArrowDown, LocateFixed } from 'lucide-react';
import type React from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ScrollDetachMode } from '../../hooks/useTranscriptScrollFollow';

export interface TranscriptScrollFollowBadgeProps {
  detachMode: ScrollDetachMode;
  onResume: () => void;
  wordCount: number;
  className?: string;
}

/**
 * Integrated scroll follow indicator and word count badge.
 *
 * Sits at the bottom-right corner of the TranscriptEditor.
 * - In default/following state: shows word count smoothly.
 * - In detached state: smoothly slides out an interactive action button to the left
 *   (ArrowDown for streaming output, LocateFixed for audio playback).
 */
export function TranscriptScrollFollowBadge({
  detachMode,
  onResume,
  wordCount,
  className = '',
}: TranscriptScrollFollowBadgeProps): React.JSX.Element {
  const { t } = useTranslation();
  const [isExiting, setIsExiting] = useState(false);
  const activeModeRef = useRef<ScrollDetachMode>(detachMode);

  if (detachMode !== null) {
    activeModeRef.current = detachMode;
  }

  const isDetached = detachMode !== null;
  const displayMode = detachMode ?? activeModeRef.current;
  const shouldRenderButton = isDetached || isExiting;

  useEffect(() => {
    if (detachMode === null && activeModeRef.current !== null) {
      setIsExiting(true);
      const timer = window.setTimeout(() => {
        setIsExiting(false);
        activeModeRef.current = null;
      }, 300);
      return () => window.clearTimeout(timer);
    }
  }, [detachMode]);

  const tooltipText =
    displayMode === 'playback'
      ? t('editor.locate_playing', { defaultValue: 'Locate playing segment' })
      : t('editor.follow_bottom', { defaultValue: 'Scroll to bottom and resume follow' });

  const wordCountText = t('editor.word_count', {
    count: wordCount,
    defaultValue: `${wordCount} words`,
  });

  return (
    <div
      className={`word-count-badge transcript-scroll-badge ${isDetached ? 'is-detached' : ''} ${className}`}
      data-testid="transcript-scroll-badge"
    >
      <div className={`scroll-badge-action-slot ${isDetached ? 'is-visible' : ''}`}>
        {shouldRenderButton && (
          <button
            type="button"
            className="scroll-badge-action-btn"
            onClick={onResume}
            data-tooltip={tooltipText}
            data-tooltip-pos="top"
            aria-label={tooltipText}
            data-testid="scroll-follow-resume-btn"
            tabIndex={isDetached ? 0 : -1}
            aria-hidden={!isDetached}
            style={isDetached ? undefined : { pointerEvents: 'none' }}
          >
            {displayMode === 'playback' ? (
              <LocateFixed size={13} aria-hidden="true" />
            ) : (
              <ArrowDown size={13} aria-hidden="true" />
            )}
          </button>
        )}
      </div>
      <span className="scroll-badge-word-count">{wordCountText}</span>
    </div>
  );
}

export default TranscriptScrollFollowBadge;
