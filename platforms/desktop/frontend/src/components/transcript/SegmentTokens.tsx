import type React from 'react';
import { useCallback, useMemo, useRef } from 'react';
import type { Match } from '../../stores/searchStore';
import { useTranscriptPlaybackStore } from '../../stores/transcriptPlaybackStore';
import type { TranscriptSegment, TranscriptTimingUnit } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { sanitizeTranscriptHtml } from '../../utils/transcriptTextUtils';
import { useReadonlySegmentContextMenu } from './context-menu/useReadonlySegmentContextMenu';

/** Props for SegmentTokens component. */
export interface SegmentTokensProps {
  segment: TranscriptSegment;
  isActive: boolean;
  onSeek: (time: number) => void;
  onMatchClick?: (index: number) => void;
  matches?: Match[];
  activeMatch?: Match | null;
  onEditTranslation?: () => void;
  onEnrollSample?: () => void;
}

interface TokenListProps {
  segmentText: string;
  isFinal: boolean;
  alignedUnits: TranscriptTimingUnit[] | null;
  activeUnitStart: number;
  onSeek: (time: number) => void;
  onMatchClick?: (index: number) => void;
  matches?: Match[];
  activeMatch?: Match | null;
  rootRef: React.RefObject<HTMLParagraphElement | null>;
  onContextMenu: React.MouseEventHandler<HTMLElement>;
  onContextMenuKeyDown: React.KeyboardEventHandler<HTMLElement>;
}

/**
 * Helper to determine if a token overlaps with any search matches.
 *
 * @param tokenStart Start index of the token in the segment text.
 * @param tokenEnd End index of the token in the segment text.
 * @param matches List of matches in this segment.
 * @param activeMatch The currently active match (globally).
 * @returns Object containing match status and index.
 */
function checkTokenMatch(
  tokenStart: number,
  tokenEnd: number,
  matches: Match[] | undefined,
  activeMatch: Match | null | undefined
): { isMatch: boolean; isActiveMatch: boolean; matchIndex: number } {
  let isMatch = false;
  const isActiveMatch = false;
  let matchIndex = -1;

  if (!matches || matches.length === 0) {
    return { isMatch, isActiveMatch, matchIndex };
  }

  // Check active match first
  if (activeMatch) {
    const matchEnd = activeMatch.startIndex + activeMatch.length;
    if (activeMatch.startIndex < tokenEnd && matchEnd > tokenStart) {
      return {
        isMatch: true,
        isActiveMatch: true,
        matchIndex: activeMatch.globalIndex ?? -1,
      };
    }
  }

  // Check other matches
  for (const m of matches) {
    const mEnd = m.startIndex + m.length;
    if (m.startIndex < tokenEnd && mEnd > tokenStart) {
      isMatch = true;
      if (m.globalIndex !== undefined) {
        matchIndex = m.globalIndex;
      }
      break;
    }
  }

  return { isMatch, isActiveMatch, matchIndex };
}

function doTimingUnitsMatchSegmentText(
  alignedUnits: TranscriptTimingUnit[] | null,
  segmentText: string
): boolean {
  if (!alignedUnits) {
    return true;
  }

  const timingText = alignedUnits.map((unit) => unit.text).join('');
  return timingText === segmentText;
}

/**
 * Pure component to render the list of tokens.
 * Only re-renders when the active token changes.
 */
function TokenList({
  segmentText,
  isFinal,
  alignedUnits,
  activeUnitStart,
  onSeek,
  onMatchClick,
  matches,
  activeMatch,
  rootRef,
  onContextMenu,
  onContextMenuKeyDown,
}: TokenListProps): React.JSX.Element {
  // Calculate token indices for highlighting
  // Optimized by React Compiler to re-calculate only when alignedUnits changes.

  const tokensWithIndices = useMemo(() => {
    if (!alignedUnits) return null;
    let idx = 0;
    return alignedUnits.map((unit) => {
      const start = idx;
      idx += unit.text.length;
      return { ...unit, startIndex: start, endIndex: idx };
    });
  }, [alignedUnits]);

  return (
    <p
      ref={rootRef}
      className={`segment-text ${!isFinal ? 'partial' : ''}`}
      style={{ whiteSpace: 'pre-wrap' }}
      tabIndex={0}
      aria-haspopup="menu"
      onContextMenu={onContextMenu}
      onKeyDown={onContextMenuKeyDown}
    >
      {tokensWithIndices ? (
        tokensWithIndices.map((tokenObj, i) => {
          const isTimeActive = tokenObj.start === activeUnitStart;

          const { isMatch, isActiveMatch, matchIndex } = checkTokenMatch(
            tokenObj.startIndex,
            tokenObj.endIndex,
            matches,
            activeMatch
          );

          let searchClass = '';
          if (isActiveMatch) {
            searchClass = 'search-match-active';
          } else if (isMatch) {
            searchClass = 'search-match';
          }

          const className = ['token-hover', isTimeActive ? 'active-token' : '', searchClass]
            .filter(Boolean)
            .join(' ');

          return (
            <span
              key={`${tokenObj.start}-${tokenObj.end}-${i}`}
              className={className}
              data-tooltip={formatDisplayTime(tokenObj.start)}
              data-tooltip-pos="top"
              role="button"
              tabIndex={0}
              onClick={(e) => {
                e.stopPropagation();
                onSeek(tokenObj.start);
                if (isMatch && matchIndex !== -1 && onMatchClick) {
                  onMatchClick(matchIndex);
                }
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  e.stopPropagation();
                  onSeek(tokenObj.start);
                  if (isMatch && matchIndex !== -1 && onMatchClick) {
                    onMatchClick(matchIndex);
                  }
                }
              }}
              dangerouslySetInnerHTML={{ __html: sanitizeTranscriptHtml(tokenObj.text) }}
            />
          );
        })
      ) : (
        <span dangerouslySetInnerHTML={{ __html: sanitizeTranscriptHtml(segmentText) }} />
      )}
      {!isFinal && <span className="transcript-caret-pulse" aria-hidden="true" />}
    </p>
  );
}

/**
 * Helper component that subscribes to the store for time updates.
 * Only mounted for the active segment.
 */
function ActiveSegmentWrapper({
  alignedUnits,
  renderTokenList,
}: {
  alignedUnits: TranscriptTimingUnit[] | null;
  renderTokenList: (timestamp: number) => React.JSX.Element;
}) {
  // Selector to compute active timestamp directly from store state
  // This avoids re-renders when currentTime changes but the active token remains the same
  const activeUnitStart = useTranscriptPlaybackStore(
    useCallback(
      (state) => {
        const currentTime = state.currentTime;
        if (!alignedUnits || currentTime < 0) return -1;

        const activeUnit = alignedUnits.find(
          (unit, index) =>
            currentTime >= unit.start &&
            (currentTime < unit.end || index === alignedUnits.length - 1)
        );

        return activeUnit ? activeUnit.start : -1;
      },
      [alignedUnits]
    )
  );

  return renderTokenList(activeUnitStart);
}

/**
 * Renders the text content of a segment, handling token highlighting.
 *
 * Optimization: Only the active segment subscribes to high-frequency time updates.
 * Inactive segments render a static list, avoiding 1000s of unnecessary selector executions per frame.
 */
export function SegmentTokens({
  segment,
  isActive,
  onSeek,
  onMatchClick,
  matches,
  activeMatch,
  onEditTranslation,
  onEnrollSample,
}: SegmentTokensProps): React.JSX.Element {
  const rootRef = useRef<HTMLParagraphElement>(null);
  const contextMenuHandlers = useReadonlySegmentContextMenu({
    segmentId: segment.id,
    rootRef,
    onEditTranslation,
    hasTranslation: Boolean(segment.translation),
    onEnrollSample,
  });
  const alignedUnits = useMemo(
    () => (segment.timing?.level === 'token' ? segment.timing.units : null),
    [segment.timing]
  );
  const renderAlignedUnits = doTimingUnitsMatchSegmentText(alignedUnits, segment.text)
    ? alignedUnits
    : null;

  // Stable render prop
  const renderTokenList = (timestamp: number) => (
    <TokenList
      segmentText={segment.text}
      isFinal={segment.isFinal ?? true}
      alignedUnits={renderAlignedUnits}
      activeUnitStart={timestamp}
      onSeek={onSeek}
      onMatchClick={onMatchClick}
      matches={matches}
      activeMatch={activeMatch}
      rootRef={rootRef}
      onContextMenu={contextMenuHandlers.onContextMenu}
      onContextMenuKeyDown={contextMenuHandlers.onKeyDown}
    />
  );

  if (isActive) {
    return (
      <ActiveSegmentWrapper alignedUnits={renderAlignedUnits} renderTokenList={renderTokenList} />
    );
  }

  return renderTokenList(-1);
}
