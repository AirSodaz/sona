import { useRef, useCallback, useEffect } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { transcriptionService } from '../services/transcriptionService';

/**
 * Hook to manage segment re-alignment requests.
 *
 * This hook handles debouncing alignment requests and triggering the sidecar
 * to produce fresh tokens, timestamps, and durations for a segment.
 *
 * @return The requestAlignment function.
 */
export function useSegmentAlignment(): (segmentId: string) => void {
    // Debounced alignment timers per segment
    const alignTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

    // Cleanup alignment timers on unmount
    useEffect(() => {
        return () => {
            // eslint-disable-next-line react-hooks/exhaustive-deps
            for (const timer of alignTimersRef.current.values()) {
                clearTimeout(timer);
            }
            alignTimersRef.current.clear();
        };
    }, []);

    /**
     * Requests CTC re-alignment for a segment after a debounce period.
     * Spawns the sidecar to produce fresh tokens/timestamps/durations.
     *
     * @param segmentId The ID of the segment to align.
     */
    const requestAlignment = useCallback((segmentId: string) => {
        // Cancel any pending alignment for this segment
        const existing = alignTimersRef.current.get(segmentId);
        if (existing) {
            clearTimeout(existing);
        }

        const timer = setTimeout(async () => {
            alignTimersRef.current.delete(segmentId);

            // Re-read segment from store (may have been edited again or deleted)
            const segment = useTranscriptStore.getState().segments.find(s => s.id === segmentId);
            if (!segment) return;

            const store = useTranscriptStore.getState();
            store.addAligningSegmentId(segmentId);

            try {
                const result = await transcriptionService.alignSegment(segment);
                // Verify segment still exists before applying
                const current = useTranscriptStore.getState().segments.find(s => s.id === segmentId);
                if (current && result) {
                    useTranscriptStore.getState().updateSegment(segmentId, {
                        tokens: result.tokens,
                        timestamps: result.timestamps,
                        durations: result.durations,
                    });
                }
            } catch (error) {
                console.error('[useSegmentAlignment] Alignment failed:', error);
            } finally {
                useTranscriptStore.getState().removeAligningSegmentId(segmentId);
            }
        }, 1500);

        alignTimersRef.current.set(segmentId, timer);
    }, []);

    return requestAlignment;
}
