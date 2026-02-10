import React, { useMemo } from 'react';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { alignTokensToText } from '../../utils/segmentUtils';
import { useTranscriptStore } from '../../stores/transcriptStore';

/** Props for SegmentTokens component. */
export interface SegmentTokensProps {
    segment: TranscriptSegment;
    isActive: boolean;
    onSeek: (time: number) => void;
}

interface TokenListProps {
    segmentText: string;
    isFinal: boolean;
    alignedTokens: { text: string; timestamp: number }[] | null;
    activeTokenTimestamp: number;
    onSeek: (time: number) => void;
}

/**
 * Pure component to render the list of tokens.
 * Only re-renders when the active token changes.
 */
const TokenList = React.memo(({ segmentText, isFinal, alignedTokens, activeTokenTimestamp, onSeek }: TokenListProps) => {
    return (
        <p className={`segment-text ${!isFinal ? 'partial' : ''}`}>
            {alignedTokens ? (
                alignedTokens.map((tokenObj, i) => (
                    <span
                        key={i}
                        title={formatDisplayTime(tokenObj.timestamp)}
                        className={`token-hover ${tokenObj.timestamp === activeTokenTimestamp ? 'active-token' : ''}`}
                        onClick={(e) => {
                            e.stopPropagation();
                            onSeek(tokenObj.timestamp);
                        }}
                    >
                        {tokenObj.text}
                    </span>
                ))
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
    const currentTime = useTranscriptStore(state => state.currentTime);

    const activeTokenTimestamp = useMemo(() => {
        if (!alignedTokens || currentTime < 0) return -1;
        const nextTokenIndex = alignedTokens.findIndex(t => t.timestamp > currentTime);
        const activeIdx = nextTokenIndex === -1 ? alignedTokens.length - 1 : nextTokenIndex - 1;
        return activeIdx >= 0 ? alignedTokens[activeIdx].timestamp : -1;
    }, [alignedTokens, currentTime]);

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
        />
    );

    if (isActive) {
        return <ActiveSegmentWrapper alignedTokens={alignedTokens} renderTokenList={renderTokenList} />;
    }

    return renderTokenList(-1);
}

export const SegmentTokens = React.memo(SegmentTokensComponent);
