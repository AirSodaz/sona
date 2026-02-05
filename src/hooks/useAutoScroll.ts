import { useEffect } from 'react';
import { VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';

/**
 * Automatically scrolls the transcript view to keep the active segment in view.
 *
 * @param virtuosoRef The ref to the Virtuoso list component.
 */
export function useAutoScroll(virtuosoRef: React.RefObject<VirtuosoHandle | null>) {
    // We don't need to track last index manually anymore as the store provides it

    useEffect(() => {
        const unsub = useTranscriptStore.subscribe((state, prevState) => {
            const { activeSegmentIndex, activeSegmentId, isPlaying } = state;
            const prevActiveId = prevState.activeSegmentId;
            const prevIsPlaying = prevState.isPlaying;

            // Only scroll if:
            // 1. The active segment changed (and exists)
            // 2. Playback just started (and we have an active segment)
            // 3. User explicitly sought to a time (force scroll)
            const shouldScroll = (activeSegmentId !== prevActiveId && activeSegmentId !== null) ||
                (isPlaying && !prevIsPlaying && activeSegmentId !== null) ||
                (state.lastSeekTimestamp !== prevState.lastSeekTimestamp);

            if (shouldScroll && virtuosoRef.current && activeSegmentIndex !== -1) {
                virtuosoRef.current.scrollToIndex({
                    index: activeSegmentIndex,
                    align: 'center',
                    behavior: 'smooth',
                });
            }
        });
        return unsub;
    }, [virtuosoRef]);
}
