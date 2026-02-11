import { create } from 'zustand';
import { TranscriptSegment } from '../types/transcript';

export interface Match {
    segmentId: string;
    startIndex: number;
    length: number;
    text: string; // The actual matched text (useful for case-insensitive display)
    globalIndex?: number; // Optional index in the global matches array
}

interface SearchState {
    isOpen: boolean;
    query: string;
    replaceQuery: string;
    isReplaceOpen: boolean;
    matches: Match[];
    currentMatchIndex: number;

    // Actions
    open: () => void;
    close: () => void;
    setQuery: (query: string) => void;
    setReplaceQuery: (query: string) => void;
    toggleReplace: () => void;
    nextMatch: () => void;
    prevMatch: () => void;
    setActiveMatch: (index: number) => void;
    performSearch: (segments: TranscriptSegment[]) => void;
}

export const useSearchStore = create<SearchState>((set, get) => ({
    isOpen: false,
    query: '',
    replaceQuery: '',
    isReplaceOpen: false,
    matches: [],
    currentMatchIndex: -1,

    open: () => set({ isOpen: true }),
    close: () => set({ isOpen: false, query: '', matches: [], currentMatchIndex: -1 }),

    setQuery: (query: string) => {
        set({ query });
        // Note: performSearch needs to be called by the component with segments
        // or we need a way to access segments here. 
        // For now, we'll rely on the consuming component to trigger search 
        // or we can subscribe to transcriptStore if we want to couple them.
        // Decoupled is better for testing.
    },

    setReplaceQuery: (replaceQuery: string) => set({ replaceQuery }),

    toggleReplace: () => set((state) => ({ isReplaceOpen: !state.isReplaceOpen })),

    nextMatch: () => {
        const { matches, currentMatchIndex } = get();
        if (matches.length === 0) return;
        const nextIndex = (currentMatchIndex + 1) % matches.length;
        set({ currentMatchIndex: nextIndex });
    },

    prevMatch: () => {
        const { matches, currentMatchIndex } = get();
        if (matches.length === 0) return;
        const prevIndex = (currentMatchIndex - 1 + matches.length) % matches.length;
        set({ currentMatchIndex: prevIndex });
    },

    setActiveMatch: (index: number) => {
        const { matches } = get();
        if (index >= 0 && index < matches.length) {
            set({ currentMatchIndex: index });
        }
    },

    performSearch: (segments: TranscriptSegment[]) => {
        const { query } = get();
        if (!query.trim()) {
            set({ matches: [], currentMatchIndex: -1 });
            return;
        }

        const matches: Match[] = [];
        const lowerQuery = query.toLowerCase();

        segments.forEach((segment) => {
            const text = segment.text;
            const lowerText = text.toLowerCase();
            let startIndex = 0;
            let index = lowerText.indexOf(lowerQuery, startIndex);

            while (index !== -1) {
                matches.push({
                    segmentId: segment.id,
                    startIndex: index,
                    length: query.length,
                    text: text.substr(index, query.length),
                    globalIndex: matches.length
                });
                startIndex = index + 1;
                index = lowerText.indexOf(lowerQuery, startIndex);
            }
        });

        set({
            matches,
            currentMatchIndex: matches.length > 0 ? 0 : -1
        });
    },
}));
