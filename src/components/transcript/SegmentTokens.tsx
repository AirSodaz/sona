import React, { useContext, useCallback } from 'react';
import { useStore } from 'zustand';
import { TranscriptSegment } from '../../types/transcript';
import { formatDisplayTime } from '../../utils/exportFormats';
import { alignTokensToText } from '../../utils/segmentUtils';
import { TranscriptUIContext } from './TranscriptUIContext';

/** Props for SegmentTokens component. */
export interface SegmentTokensProps {
    segment: TranscriptSegment;
    isActive: boolean;
    onSeek: (time: number) => void;
}

/**
 * Renders the text content of a segment, handling token highlighting.
 * Isolated component to prevent parent re-renders on every time update.
 */
function SegmentTokensComponent({
    segment,
    isActive,
    onSeek,
}: SegmentTokensProps): React.JSX.Element {
    const uiStore = useContext(TranscriptUIContext);
    if (!uiStore) throw new Error('SegmentTokens must be used within TranscriptUIContext');

    // Only subscribe to current time if this segment is active
    // If inactive, currentTime is -1 (or doesn't matter, effectively)
    const currentTime = useStore(uiStore, useCallback((state) => isActive ? state.currentTime : -1, [isActive]));

    // Align tokens with formatted text
    const alignedTokens = React.useMemo(() => {
        if (!segment.tokens || !segment.timestamps) return null;
        return alignTokensToText(segment.text, segment.tokens, segment.timestamps);
    }, [segment.text, segment.tokens, segment.timestamps]);

    // Determine active token timestamp if segment is active
    const activeTokenTimestamp = React.useMemo(() => {
        if (!isActive || !alignedTokens || currentTime < 0) return -1;
        // Find the last token that has started
        const nextTokenIndex = alignedTokens.findIndex(t => t.timestamp > currentTime);
        const activeIdx = nextTokenIndex === -1 ? alignedTokens.length - 1 : nextTokenIndex - 1;
        return activeIdx >= 0 ? alignedTokens[activeIdx].timestamp : -1;
    }, [isActive, alignedTokens, currentTime]);

    return (
        <p className={`segment-text ${!segment.isFinal ? 'partial' : ''}`}>
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
                segment.text || '(empty)'
            )}
        </p>
    );
}

export const SegmentTokens = React.memo(SegmentTokensComponent);
