import { render, screen, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from '../TranscriptUIContext';
import { createStore } from 'zustand/vanilla';
import { useTranscriptStore } from '../../../stores/transcriptStore';
import { useSearchStore } from '../../../stores/searchStore';

// Mock dependencies
vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('../../Icons', () => ({
    EditIcon: () => <div data-testid="edit-icon" />,
    TrashIcon: () => <div data-testid="trash-icon" />,
    MergeIcon: () => <div data-testid="merge-icon" />,
}));

vi.mock('../SegmentTimestamp', () => ({
    SegmentTimestamp: () => <div data-testid="timestamp" />,
}));

// Mock SegmentTokens to track renders
const SegmentTokensMock = vi.fn(({ activeMatch }) => (
    <div data-testid="segment-tokens">
        {activeMatch ? `Active: ${activeMatch.globalIndex}` : 'No Active Match'}
    </div>
));

vi.mock('../SegmentTokens', () => ({
    SegmentTokens: (props: any) => SegmentTokensMock(props),
}));

describe('SegmentItem Search Optimization', () => {
    let uiStore: any;

    const segment = {
        id: 'seg-1',
        start: 0,
        end: 5,
        text: 'Hello world',
        isFinal: true,
        tokens: [],
        timestamps: []
    };

    const defaultProps = {
        segment,
        index: 0,
        onSeek: vi.fn(),
        onEdit: vi.fn(),
        onSave: vi.fn(),
        onDelete: vi.fn(),
        onMergeWithNext: vi.fn(),
        onAnimationEnd: vi.fn(),
    };

    beforeEach(() => {
        useTranscriptStore.setState({ currentTime: 0 });
        useSearchStore.setState({
            matches: [],
            currentMatchIndex: -1
        });

        uiStore = createStore<TranscriptUIState>(() => ({
            newSegmentIds: new Set(),
            activeSegmentId: null,
            editingSegmentId: null,
            totalSegments: 1,
            aligningSegmentIds: new Set(),
        }));

        SegmentTokensMock.mockClear();
    });

    const renderComponent = () => render(
        <TranscriptUIContext.Provider value={uiStore}>
            <SegmentItem {...defaultProps} />
        </TranscriptUIContext.Provider>
    );

    it('re-renders only when active match enters/leaves this segment', async () => {
        // Setup matches:
        // Match 0: seg-1 (this segment)
        // Match 1: seg-2 (other segment)
        // Match 2: seg-1 (this segment)
        // Match 3: seg-3 (other segment)

        const matches = [
            { segmentId: 'seg-1', startIndex: 0, length: 5, text: 'Hello', globalIndex: 0 },
            { segmentId: 'seg-2', startIndex: 0, length: 5, text: 'world', globalIndex: 1 },
            { segmentId: 'seg-1', startIndex: 6, length: 5, text: 'world', globalIndex: 2 },
            { segmentId: 'seg-3', startIndex: 0, length: 5, text: 'foo', globalIndex: 3 },
        ];

        useSearchStore.setState({ matches, currentMatchIndex: -1 });

        renderComponent();
        expect(SegmentTokensMock).toHaveBeenCalledTimes(1);
        expect(screen.getByTestId('segment-tokens').textContent).toBe('No Active Match');

        // 1. Activate Match 0 (In this segment) -> Should re-render
        act(() => {
            useSearchStore.setState({ currentMatchIndex: 0 });
        });

        expect(SegmentTokensMock).toHaveBeenCalledTimes(2);
        expect(screen.getByTestId('segment-tokens').textContent).toBe('Active: 0');

        // 2. Activate Match 1 (In OTHER segment) -> Should re-render (active match leaves)
        act(() => {
            useSearchStore.setState({ currentMatchIndex: 1 });
        });

        expect(SegmentTokensMock).toHaveBeenCalledTimes(3);
        expect(screen.getByTestId('segment-tokens').textContent).toBe('No Active Match');

        // 3. Activate Match 2 (In THIS segment again) -> Should re-render (active match enters)
        act(() => {
            useSearchStore.setState({ currentMatchIndex: 2 });
        });

        expect(SegmentTokensMock).toHaveBeenCalledTimes(4);
        expect(screen.getByTestId('segment-tokens').textContent).toBe('Active: 2');

        // 4. Activate Match 1 (Outside).
        // Change 2 -> 1. Inside -> Outside. Re-render expected.
        act(() => {
            useSearchStore.setState({ currentMatchIndex: 1 });
        });
        expect(SegmentTokensMock).toHaveBeenCalledTimes(5);
        expect(screen.getByTestId('segment-tokens').textContent).toBe('No Active Match');

        // 5. Activate Match 3 (Outside).
        // Change 1 -> 3. Outside -> Outside.
        // Expect NO RE-RENDER!
        SegmentTokensMock.mockClear();

        act(() => {
            useSearchStore.setState({ currentMatchIndex: 3 });
        });

        expect(SegmentTokensMock).toHaveBeenCalledTimes(0); // Optimization verified!
    });
});
