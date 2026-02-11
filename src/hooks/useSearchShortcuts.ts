import { useEffect, RefObject } from 'react';
import { VirtuosoHandle } from 'react-virtuoso';
import { TranscriptSegment } from '../types/transcript';
import { useSearchStore } from '../stores/searchStore';

/**
 * Hook to handle search shortcuts and scrolling synchronization.
 *
 * It listens for Ctrl+F/Cmd+F to open the search UI and automatically
 * scrolls the virtualized list to the active search match.
 *
 * @param virtuosoRef Ref to the Virtuoso list component.
 * @param segmentsRef Ref to the segments list (stable ref) to find indexes.
 */
export function useSearchShortcuts(
    virtuosoRef: RefObject<VirtuosoHandle | null>,
    segmentsRef: RefObject<TranscriptSegment[]>
): void {
    const {
        isOpen: isSearchOpen,
        open: openSearch,
        matches: searchMatches,
        currentMatchIndex: searchMatchIndex
    } = useSearchStore();

    // Keyboard shortcut to open search
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                e.preventDefault();
                openSearch();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [openSearch]);

    // Scroll to active match
    useEffect(() => {
        if (isSearchOpen && searchMatches.length > 0 && searchMatchIndex >= 0 && virtuosoRef.current && segmentsRef.current) {
            const match = searchMatches[searchMatchIndex];
            const segmentIndex = segmentsRef.current.findIndex(s => s.id === match.segmentId);

            if (segmentIndex !== -1) {
                virtuosoRef.current.scrollToIndex({
                    index: segmentIndex,
                    align: 'center',
                    behavior: 'smooth'
                });
            }
        }
    }, [isSearchOpen, searchMatchIndex, searchMatches, virtuosoRef, segmentsRef]);
}
