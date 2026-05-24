import { fireEvent, render } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from '../TranscriptUIContext';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore, resetTranscriptStores } from '../../../test-utils/transcriptStoreTestUtils';
import { normalizeTranscriptSegment } from '../../../utils/transcriptTiming';
import { splitTranscriptSegment } from '../../../stores/transcriptCoordinator';
import { splitTranscriptText } from '../richText';

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

    it('splitTranscriptText correctly splits plain text', () => {
        const text = 'Hello world';
        const [left, right] = splitTranscriptText(text, 5);
        expect(left).toBe('Hello');
        expect(right).toBe(' world');
    });

    it('splitTranscriptText correctly splits rich text with active formatting tags', () => {
        const text = 'Hello <b>world</b>';
        // 'Hello ' is 6 chars, split at 7 ('w' is index 6, split after 'w' at offset 7)
        const [left, right] = splitTranscriptText(text, 7);
        expect(left).toBe('Hello <b>w</b>');
        expect(right).toBe('<b>orld</b>');
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
        const newSegId = splitTranscriptSegment('seg-1', 6, 'Hello world');
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

describe('SegmentItem Shift + Enter Split Keydown', () => {
    let uiStore: any;

    const segment = normalizeTranscriptSegment({
        id: 'test-seg',
        start: 0,
        end: 5,
        text: 'Hello world',
        isFinal: true
    });

    const defaultProps = {
        segment,
        index: 0,
        onSeek: vi.fn(),
        onEdit: vi.fn(),
        onSave: vi.fn(),
        onDelete: vi.fn(),
        onMergeWithNext: vi.fn(),
        onSplit: vi.fn(),
        onAnimationEnd: vi.fn(),
    };

    beforeEach(() => {
        uiStore = createStore<TranscriptUIState>(() => ({
            newSegmentIds: new Set(),
            activeSegmentId: 'test-seg',
            editingSegmentId: 'test-seg',
            totalSegments: 1,
            aligningSegmentIds: new Set(),
        }));
    });

    it('triggers onSplit when Shift + Enter is pressed', () => {
        const { container } = render(
            <TranscriptUIContext.Provider value={uiStore}>
                <SegmentItem {...defaultProps} />
            </TranscriptUIContext.Provider>
        );

        const input = container.querySelector('.segment-input') as HTMLDivElement;
        expect(input).toBeTruthy();

        input.innerHTML = 'Hello world';
        fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });

        expect(defaultProps.onSplit).toHaveBeenCalledWith('test-seg', expect.any(Number), 'Hello world');
    });
});
