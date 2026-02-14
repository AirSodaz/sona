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
    // We need to detect any text or structural changes.
    // A simple concatenation of fields works well here.
    // It avoids the overhead of full JSON serialization.
    // Since this runs on every update, efficiency is key.
    // We verify ID, text, and timing for each segment.
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
            // 2. Update in-memory metadata (Preview Text & Search Content)
            const fullText = segments.map(s => s.text).join(' ');
            const previewText = fullText.substring(0, 100) + (fullText.length > 100 ? '...' : '');

            updateItemMeta(historyId, {
                previewText,
                searchContent: fullText
            });

        } catch (err) {
            console.error('[AutoSave] Failed to save:', err);
        } finally {
            isSavingRef.current = false;
        }
    };
}
