import { useEffect, useRef } from 'react';
import { useTranscriptStore } from '../stores/transcriptStore';
import { historyService } from '../services/historyService';
import { useHistoryStore } from '../stores/historyStore';
import { TranscriptSegment } from '../types/transcript';

/**
 * Computes a lightweight fingerprint for a list of segments.
 * Used to detect changes without deep comparison of every field.
 */
function computeSegmentsFingerprint(segments: TranscriptSegment[]): string {
    // Length + Concatenated ID|Text of first, middle, and last items (sampling) + Total text length
    // Actually for accuracy we probably need to check all texts, but let's try a robust summary
    // Since users edit text, we need to detect text changes.
    // Concatenating all IDs and Texts might be heavy for very long transcripts.
    // But given we want to save on ANY edit, we should be precise.

    // JSON stringify is actually quite fast for reasonable sizes.
    // Let's try a custom string builder to avoid full JSON overhead if possible,
    // or just rely on the fact that we only run this when zustand notifies us.

    // Let's stick to a simple strategy:
    // We only care about ID (order/existence) and Text (content).
    // Start/End times usually change with alignment or merge, which also changes ID/Text structure often.
    return segments.map(s => `${s.id}:${s.text}:${s.start}:${s.end}`).join('|');
}

/**
 * Hook to auto-save transcript changes to the history file.
 *
 * - Debounces saves by 2 seconds
 * - Flushes pending saves immediately when switching history items
 * - Updates in-memory history list metadata (preview text)
 */
export function useAutoSaveTranscript() {
    const isSavingRef = useRef(false);
    const timeoutRef = useRef<NodeJS.Timeout | null>(null);
    const lastFingerprintRef = useRef<string>('');
    const updateItemMeta = useHistoryStore(state => state.updateItemMeta);

    // Subscribe to store changes
    useEffect(() => {
        // Initialize fingerprint with current state
        lastFingerprintRef.current = computeSegmentsFingerprint(useTranscriptStore.getState().segments);

        const unsubscribe = useTranscriptStore.subscribe(
            (state, prevState) => {
                const currentId = state.sourceHistoryId;
                const prevId = prevState.sourceHistoryId;

                // 1. Handle Switching History Items (Flush pending save for previous item)
                if (prevId && prevId !== currentId) {
                    if (timeoutRef.current) {
                        clearTimeout(timeoutRef.current);
                        timeoutRef.current = null;

                        // Flush save for PREVIOUS item using PREVIOUS segments
                        const prevSegments = prevState.segments;
                        console.log('[AutoSave] Switching items, flushing save for:', prevId);
                        saveToHistory(prevId, prevSegments);
                    }

                    // Reset fingerprint for new item
                    lastFingerprintRef.current = computeSegmentsFingerprint(state.segments);
                    return;
                }

                // 2. Handle Edit in Current Item
                if (currentId) {
                    const currentFingerprint = computeSegmentsFingerprint(state.segments);

                    if (currentFingerprint !== lastFingerprintRef.current) {
                        // Change detected
                        lastFingerprintRef.current = currentFingerprint;

                        // Debounce save
                        if (timeoutRef.current) {
                            clearTimeout(timeoutRef.current);
                        }

                        timeoutRef.current = setTimeout(() => {
                            console.log('[AutoSave] Debounce triggered for:', currentId);
                            saveToHistory(currentId, state.segments);
                        }, 2000);
                    }
                }
            }
        );

        return () => {
            unsubscribe();
            if (timeoutRef.current) {
                clearTimeout(timeoutRef.current);
            }
        };
    }, []); // Run once on mount

    const saveToHistory = async (historyId: string, segments: TranscriptSegment[]) => {
        if (isSavingRef.current) return;

        try {
            isSavingRef.current = true;
            console.log('[AutoSave] Saving transcript...', historyId);

            // 1. Persist to disk
            await historyService.updateTranscript(historyId, segments);

            // 2. Update in-memory metadata (Preview Text & Search Content)
            const previewText = segments.map(s => s.text).join(' ').substring(0, 100) + (segments.length > 0 ? '...' : '');
            const searchContent = segments.map(s => s.text).join(' ');

            updateItemMeta(historyId, {
                previewText,
                searchContent
            });

        } catch (err) {
            console.error('[AutoSave] Failed to save:', err);
        } finally {
            isSavingRef.current = false;
        }
    };
}
