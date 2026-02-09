import { createContext } from 'react';
import { StoreApi } from 'zustand';

/**
 * UI State for TranscriptEditor that changes frequently (e.g. animation states).
 * Separated from the main store to prevent full list re-renders.
 */
export interface TranscriptUIState {
    /** Set of segment IDs that are considered "new" and should be animated. */
    newSegmentIds: Set<string>;
    /** ID of the currently active segment. */
    activeSegmentId: string | null;
    /** ID of the segment currently being edited. */
    editingSegmentId: string | null;
    /** Current playback time in seconds. */
    currentTime: number;
    /** Total number of segments (for calculating hasNext). */
    totalSegments: number;
}

/**
 * Context to provide the local UI store to SegmentItems.
 * This allows items to subscribe to specific UI state changes (like isNew)
 * without re-rendering the entire parent list.
 */
export const TranscriptUIContext = createContext<StoreApi<TranscriptUIState> | null>(null);
