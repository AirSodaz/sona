import React from 'react';
import { useTranslation } from 'react-i18next';
import { formatDisplayTime } from '../../utils/exportFormats';

/** Props for SegmentTimestamp component. */
export interface SegmentTimestampProps {
    start: number;
    onSeek: (time: number) => void;
}

/**
 * Memoized timestamp display to prevent flickering during text updates.
 * Only re-renders when the start time changes.
 */
function SegmentTimestampComponent({
    start,
    onSeek,
}: SegmentTimestampProps): React.JSX.Element {
    const { t } = useTranslation();

    function handleClick(e: React.MouseEvent): void {
        e.stopPropagation();
        onSeek(start);
    }

    function handleKeyDown(e: React.KeyboardEvent): void {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            e.stopPropagation();
            onSeek(start);
        }
    }

    return (
        <span
            className="segment-timestamp"
            onClick={handleClick}
            onKeyDown={handleKeyDown}
            role="button"
            tabIndex={0}
            aria-label={t('editor.seek_label', { time: formatDisplayTime(start) })}
            data-tooltip={t('editor.seek_tooltip')}
        >
            {formatDisplayTime(start)}
        </span>
    );
}

export const SegmentTimestamp = React.memo(SegmentTimestampComponent, (prevProps, nextProps) => prevProps.start === nextProps.start);
