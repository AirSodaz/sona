import React, { useMemo, useCallback } from 'react';
import { TranscriptSegment, TranscriptTimingUnit } from '../../types/transcript';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { formatDisplayTime } from '../../utils/exportFormats';
import { Match } from '../../stores/searchStore';

/** Props for SegmentTokens component. */
export interface SegmentTokensProps {
    segment: TranscriptSegment;
    isActive: boolean;
    onSeek: (time: number) => void;
    onMatchClick?: (index: number) => void;
    matches?: Match[];
    activeMatch?: Match | null;
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
                matchIndex: activeMatch.globalIndex ?? -1
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

/**
 * Pure component to render the list of tokens.
 * Only re-renders when the active token changes.
 */
function TokenListComponent({
    segmentText,
    isFinal,
    alignedUnits,
    activeUnitStart,
    onSeek,
    onMatchClick,
    matches,
    activeMatch
}: TokenListProps): React.JSX.Element {
    // Calculate token indices for highlighting
    // This is cheap enough to do in render for a single segment, but could be memoized if needed.
    // Since this is inside a React.memo, it runs only when props change.

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
        <p className={`segment-text ${!isFinal ? 'partial' : ''}`} style={{ whiteSpace: 'pre-wrap' }}>
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

                    const className = [
                        'token-hover',
                        isTimeActive ? 'active-token' : '',
                        searchClass
                    ].filter(Boolean).join(' ');

                    return (
                        <span
                            key={i}
                            data-tooltip={formatDisplayTime(tokenObj.start)}
                            data-tooltip-pos="top"
                            className={className}
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
                            dangerouslySetInnerHTML={{ __html: tokenObj.text }}
                        />
                    );
                })
            ) : (
                <span dangerouslySetInnerHTML={{ __html: segmentText || '(empty)' }} />
            )}
        </p>
    );
}

const TokenList = React.memo(TokenListComponent);
TokenList.displayName = 'TokenList';

/**
 * Helper component that subscribes to the store for time updates.
 * Only mounted for the active segment.
 */
function ActiveSegmentWrapper({
    alignedUnits,
    renderTokenList
}: {
    alignedUnits: TranscriptTimingUnit[] | null,
    renderTokenList: (timestamp: number) => React.JSX.Element
}) {
    // Selector to compute active timestamp directly from store state
    // This avoids re-renders when currentTime changes but the active token remains the same
    const activeUnitStart = useTranscriptStore(useCallback((state) => {
        const currentTime = state.currentTime;
        if (!alignedUnits || currentTime < 0) return -1;

        const activeUnit = alignedUnits.find((unit, index) => (
            currentTime >= unit.start &&
            (currentTime < unit.end || index === alignedUnits.length - 1)
        ));

        return activeUnit ? activeUnit.start : -1;
    }, [alignedUnits]));

    return renderTokenList(activeUnitStart);
}

/**
 * Renders the text content of a segment, handling token highlighting.
 *
 * Optimization: Only the active segment subscribes to high-frequency time updates.
 * Inactive segments render a static list, avoiding 1000s of unnecessary selector executions per frame.
 */
function SegmentTokensComponent({
    segment,
    isActive,
    onSeek,
    onMatchClick,
    matches,
    activeMatch
}: SegmentTokensProps): React.JSX.Element {
    const alignedUnits = useMemo(() => (
        segment.timing?.level === 'token' ? segment.timing.units : null
    ), [segment.timing]);

    // Stable render prop
    const renderTokenList = (timestamp: number) => (
        <TokenList
            segmentText={segment.text}
            isFinal={segment.isFinal ?? true}
            alignedUnits={alignedUnits}
            activeUnitStart={timestamp}
            onSeek={onSeek}
            onMatchClick={onMatchClick}
            matches={matches}
            activeMatch={activeMatch}
        />
    );

    if (isActive) {
        return <ActiveSegmentWrapper alignedUnits={alignedUnits} renderTokenList={renderTokenList} />;
    }

    return renderTokenList(-1);
}

export const SegmentTokens = React.memo(SegmentTokensComponent);
