import { vi, describe, it, expect, beforeEach } from 'vitest';
import { useTranscriptStore, resetTranscriptStores } from '../../../test-utils/transcriptStoreTestUtils';
import { normalizeTranscriptSegment } from '../../../utils/transcriptTiming';
import { splitTranscriptSegment } from '../../../stores/transcriptCoordinator';
// Mock i18n
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

// Mock Icons
vi.mock('../../Icons', () => ({
    EditIcon: () => <span data-testid="edit-icon" />,
    TrashIcon: () => <span data-testid="trash-icon" />,
    MergeIcon: () => <span data-testid="merge-icon" />,
}));

// Mock SegmentTimestamp
vi.mock('../SegmentTimestamp', () => ({
    SegmentTimestamp: ({ start }: { start: number }) => <span>{start}</span>,
}));

describe('Segment Split Logic', () => {
    beforeEach(() => {
        resetTranscriptStores();
    });

    it('splitTranscriptSegment updates coordinator segments list and starts editing right segment', () => {
        const initialSegment = normalizeTranscriptSegment({
            id: 'seg-1',
            start: 10.0,
            end: 20.0,
            text: 'Hello world',
            isFinal: true,
            timing: {
                level: 'token',
                source: 'model',
                units: [
                    { text: 'Hello', start: 10.0, end: 14.0 },
                    { text: ' ', start: 14.0, end: 15.0 },
                    { text: 'world', start: 15.0, end: 20.0 }
                ]
            }
        });

        useTranscriptStore.setState({
            segments: [initialSegment]
        });

        // Split after 'Hello ' (offset 6)
        const newSegId = splitTranscriptSegment('seg-1', 'Hello ', 'world');
        expect(newSegId).toBeTruthy();

        const segments = useTranscriptStore.getState().segments;
        expect(segments.length).toBe(2);

        const leftSeg = segments[0];
        const rightSeg = segments[1];

        // Left segment timing and text
        expect(leftSeg.id).toBe('seg-1');
        expect(leftSeg.text).toBe('Hello ');
        expect(leftSeg.start).toBe(10.0);
        expect(leftSeg.end).toBe(15.0); // Timing unit end or next unit start

        // Right segment timing and text
        expect(rightSeg.id).toBe(newSegId);
        expect(rightSeg.text).toBe('world');
        expect(rightSeg.start).toBe(15.0);
        expect(rightSeg.end).toBe(20.0);

        // Focus transition
        expect(useTranscriptStore.getState().editingSegmentId).toBe(newSegId);
    });
});
