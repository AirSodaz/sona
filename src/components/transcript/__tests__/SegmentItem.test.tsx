
import { render, screen, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from '../TranscriptUIContext';
import { createStore } from 'zustand/vanilla';
// import React from 'react'; // React 17+ JSX transform doesn't need this, but we use React.useMemo in component.
// Wait, the test uses React? No.
import React from 'react';

// Mock i18n
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
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

describe('SegmentItem Highlighting', () => {
    let uiStore: any;

    const segment = {
        id: 'test-seg',
        start: 0,
        end: 5,
        text: 'Hello world test',
        isFinal: true,
        tokens: ['Hello', 'world', 'test'],
        timestamps: [0.0, 1.5, 3.0]
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
        uiStore = createStore<TranscriptUIState>(() => ({
            newSegmentIds: new Set(),
            activeSegmentId: 'test-seg', // Active segment
            editingSegmentId: null,
            currentTime: 0,
            totalSegments: 1,
        }));
    });

    const renderComponent = () => render(
        <TranscriptUIContext.Provider value={uiStore}>
            <SegmentItem {...defaultProps} />
        </TranscriptUIContext.Provider>
    );

    it('highlights the first token at start time', () => {
        // currentTime 0 (start)
        uiStore.setState({ currentTime: 0 });
        renderComponent();

        const token0 = screen.getByText('Hello');
        const token1 = screen.getByText('world');

        expect(token0.className).toContain('active-token');
        expect(token1.className).not.toContain('active-token');
    });

    it('highlights the second token when time advances', () => {
        uiStore.setState({ currentTime: 2.0 }); // 2.0 > 1.5 (world starts at 1.5)
        renderComponent();

        const token0 = screen.getByText('Hello');
        const token1 = screen.getByText('world');
        const token2 = screen.getByText('test');

        expect(token0.className).not.toContain('active-token');
        expect(token1.className).toContain('active-token');
        expect(token2.className).not.toContain('active-token');
    });

    it('highlights the last token when time is near end', () => {
        uiStore.setState({ currentTime: 4.0 }); // 4.0 > 3.0 (test starts at 3.0)
        renderComponent();

        const token1 = screen.getByText('world');
        const token2 = screen.getByText('test');

        expect(token1.className).not.toContain('active-token');
        expect(token2.className).toContain('active-token');
    });

    it('updates highlighting when store updates (re-render check)', async () => {
        uiStore.setState({ currentTime: 0 });
        const { rerender } = renderComponent();

        expect(screen.getByText('Hello').className).toContain('active-token');

        // Update store
        act(() => {
            uiStore.setState({ currentTime: 2.0 });
        });

        // Re-render implicitly handled by store subscription?
        // Wait, testing-library render doesn't auto-update from external store unless component re-renders.
        // Zustand `useStore` triggers React re-render.

        expect(screen.getByText('world').className).toContain('active-token');
        expect(screen.getByText('Hello').className).not.toContain('active-token');
    });

    it('does not highlight tokens if segment is not active', () => {
        uiStore.setState({ currentTime: 0, activeSegmentId: 'other-seg' });
        renderComponent();

        const token0 = screen.getByText('Hello');
        expect(token0.className).not.toContain('active-token');
    });
});
