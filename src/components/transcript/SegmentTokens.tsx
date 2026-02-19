import React, { useMemo, useCallback } from 'react';
import { TranscriptSegment } from '../../types/transcript';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { alignTokensToText } from '../../utils/segmentUtils';
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
    alignedTokens: { text: string; timestamp: number }[] | null;
    activeTokenTimestamp: number;
    onSeek: (time: number) => void;
    onMatchClick?: (index: number) => void;
    matches?: Match[];
    activeMatch?: Match | null;
}

/**
 * Pure component to render the list of tokens.
 * Only re-renders when the active token changes.
 */
const TokenList = React.memo(({ segmentText, isFinal, alignedTokens, activeTokenTimestamp, onSeek, onMatchClick, matches, activeMatch }: TokenListProps) => {
    // Calculate token indices for highlighting
    // This is cheap enough to do in render for a single segment, but could be memoized if needed.
    // Since this is inside a React.memo, it runs only when props change.

    const tokensWithIndices = useMemo(() => {
        if (!alignedTokens) return null;
        let idx = 0;
        return alignedTokens.map(t => {
            const start = idx;
            idx += t.text.length;
            return { ...t, start, end: idx };
        });
    }, [alignedTokens]);

    return (
        <p className={`segment-text ${!isFinal ? 'partial' : ''}`}>
            {tokensWithIndices ? (
                tokensWithIndices.map((tokenObj, i) => {
                    const isTimeActive = tokenObj.timestamp === activeTokenTimestamp;

                    // Check search matches
                    let isMatch = false;
                    let isActiveMatch = false;
                    let matchIndex = -1;

                    if (matches && matches.length > 0) {
                        // Check if this token overlaps with any match
                        // Use a loose check: if match overlaps with token
                        // match.startIndex < token.end && (match.startIndex + match.length) > token.start

                        // Check active match first
                        if (activeMatch) {
                            const matchEnd = activeMatch.startIndex + activeMatch.length;
                            if (activeMatch.startIndex < tokenObj.end && matchEnd > tokenObj.start) {
                                isActiveMatch = true;
                                isMatch = true;
                                if (activeMatch.globalIndex !== undefined) {
                                    matchIndex = activeMatch.globalIndex;
                                }
                            }
                        }

                        if (!isActiveMatch) {
                            // Check other matches
                            // Optimization: matches are sorted? Not necessarily by segment logic, but searchStore puts them in order.
                            // Simple iteration is O(M) where M is matches in segment. usually small.
                            for (const m of matches) {
                                const mEnd = m.startIndex + m.length;
                                if (m.startIndex < tokenObj.end && mEnd > tokenObj.start) {
                                    isMatch = true;
                                    if (m.globalIndex !== undefined) {
                                        matchIndex = m.globalIndex;
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    const className = [
                        'token-hover',
                        isTimeActive ? 'active-token' : '',
                        isActiveMatch ? 'search-match-active' : (isMatch ? 'search-match' : '')
                    ].filter(Boolean).join(' ');

                    return (
                        <span
                            key={i}
                            title={formatDisplayTime(tokenObj.timestamp)}
                            className={className}
                            onClick={(e) => {
                                e.stopPropagation();
                                onSeek(tokenObj.timestamp);
                                if (isMatch && matchIndex !== -1 && onMatchClick) {
                                    onMatchClick(matchIndex);
                                }
                            }}
                        >
                            {tokenObj.text}
                        </span>
                    );
                })
            ) : (
                segmentText || '(empty)'
            )}
        </p>
    );
});

TokenList.displayName = 'TokenList';

/**
 * Helper component that subscribes to the store for time updates.
 * Only mounted for the active segment.
 */
function ActiveSegmentWrapper({
    alignedTokens,
    renderTokenList
}: {
    alignedTokens: { text: string; timestamp: number }[] | null,
    renderTokenList: (timestamp: number) => React.JSX.Element
}) {
    // Selector to compute active timestamp directly from store state
    // This avoids re-renders when currentTime changes but the active token remains the same
    const activeTokenTimestamp = useTranscriptStore(useCallback((state) => {
        const currentTime = state.currentTime;
        if (!alignedTokens || currentTime < 0) return -1;

        // Find the token that is currently active (timestamp <= currentTime < nextTokenTimestamp)
        const nextTokenIndex = alignedTokens.findIndex(t => t.timestamp > currentTime);
        const activeIdx = nextTokenIndex === -1 ? alignedTokens.length - 1 : nextTokenIndex - 1;

        return activeIdx >= 0 ? alignedTokens[activeIdx].timestamp : -1;
    }, [alignedTokens]));

    return renderTokenList(activeTokenTimestamp);
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
    // Memoize token alignment to avoid re-calculation
    const alignedTokens = useMemo(() => {
        if (!segment.tokens || !segment.timestamps) return null;
        return alignTokensToText(segment.text, segment.tokens, segment.timestamps);
    }, [segment.text, segment.tokens, segment.timestamps]);

    // Stable render prop
    const renderTokenList = (timestamp: number) => (
        <TokenList
            segmentText={segment.text}
            isFinal={segment.isFinal ?? true}
            alignedTokens={alignedTokens}
            activeTokenTimestamp={timestamp}
            onSeek={onSeek}
            onMatchClick={onMatchClick}
            matches={matches}
            activeMatch={activeMatch}
        />
    );

    if (isActive) {
        return <ActiveSegmentWrapper alignedTokens={alignedTokens} renderTokenList={renderTokenList} />;
    }

    return renderTokenList(-1);
}

export const SegmentTokens = React.memo(SegmentTokensComponent);
