import { useEffect, useRef } from 'react';
import { VirtuosoHandle } from 'react-virtuoso';
import { useTranscriptStore } from '../stores/transcriptStore';

export function useAutoScroll(virtuosoRef: React.RefObject<VirtuosoHandle | null>) {
    const lastActiveIndexRef = useRef<number>(-1);

    useEffect(() => {
        const unsub = useTranscriptStore.subscribe((state, prevState) => {
            const { activeSegmentId, isPlaying, segments } = state;
            const prevActiveId = prevState.activeSegmentId;
            const prevIsPlaying = prevState.isPlaying;

            // Only scroll if activeSegmentId changed OR isPlaying became true
            const shouldScroll = (activeSegmentId !== prevActiveId && activeSegmentId) ||
                (isPlaying && !prevIsPlaying && activeSegmentId);

            if (shouldScroll && isPlaying && virtuosoRef.current) {
                let activeIndex = -1;

                // Optimization: Check near the last known index first (O(1) for sequential playback)
                const lastIndex = lastActiveIndexRef.current;
                if (lastIndex >= 0 && lastIndex < segments.length) {
                    if (segments[lastIndex].id === activeSegmentId) {
                        activeIndex = lastIndex;
                    } else if (lastIndex + 1 < segments.length && segments[lastIndex + 1].id === activeSegmentId) {
                        activeIndex = lastIndex + 1;
                    }
                }

                // Fallback to full search if not found (O(N))
                if (activeIndex === -1) {
                    activeIndex = segments.findIndex((s) => s.id === activeSegmentId);
                }

                if (activeIndex !== -1) {
                    lastActiveIndexRef.current = activeIndex;
                    virtuosoRef.current.scrollToIndex({
                        index: activeIndex,
                        align: 'center',
                        behavior: 'smooth',
                    });
                }
            }
        });
        return unsub;
    }, [virtuosoRef]);
}
