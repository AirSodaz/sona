import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import React from 'react';
import { SegmentItem } from '../SegmentItem';
import { TranscriptUIContext, TranscriptUIState } from '../TranscriptUIContext';
import { createStore } from 'zustand/vanilla';
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

vi.mock('../SegmentTokens', () => ({
    SegmentTokens: () => <div data-testid="segment-tokens" />,
}));

describe('SegmentItem Performance', () => {
    let uiStore: any;
    let renderCount = 0;

    // A wrapper component to track renders via Profiler
    const ProfilerWrapper = ({ children, id }: { children: React.ReactNode, id: string }) => {
        return (
            <React.Profiler id={id} onRender={() => { renderCount++; }}>
                {children}
            </React.Profiler>
        );
    };

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
        renderCount = 0;

        // Reset search store
        useSearchStore.setState({
            matches: [],
            currentMatchIndex: -1
        });

        // Create a fresh UI store for each test
        uiStore = createStore<TranscriptUIState>(() => ({
            newSegmentIds: new Set(),
            activeSegmentId: null,
            editingSegmentId: null,
            totalSegments: 1,
            aligningSegmentIds: new Set(),
        }));
    });

    it('should NOT re-render when matches update for OTHER segments', async () => {
        // Initial render
        render(
            <TranscriptUIContext.Provider value={uiStore}>
                <ProfilerWrapper id="SegmentItem">
                    <SegmentItem {...defaultProps} />
                </ProfilerWrapper>
            </TranscriptUIContext.Provider>
        );

        expect(renderCount).toBe(1);

        // Update search store with matches for DIFFERENT segments
        // In the current inefficient implementation, this WILL cause a re-render
        // because the component subscribes to the entire `matches` array.
        const newMatches = [
            { segmentId: 'seg-2', startIndex: 0, length: 5, text: 'Other', globalIndex: 0 },
            { segmentId: 'seg-3', startIndex: 0, length: 5, text: 'Another', globalIndex: 1 }
        ];

        // Act: Update the store
        await act(async () => {
            useSearchStore.setState({ matches: newMatches });
        });

        // If optimized, renderCount should still be 1 because the selector for seg-1 matches
        // should return an empty array (shallowly equal to previous empty array).
        // Currently, it re-renders because it subscribes to state.matches.
        expect(renderCount).toBe(1);
    });
});
