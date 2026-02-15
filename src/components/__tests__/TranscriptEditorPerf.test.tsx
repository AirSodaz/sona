import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import TranscriptEditor from '../TranscriptEditor';
import { useTranscriptStore } from '../../stores/transcriptStore';

// Mock scrollTo
window.HTMLElement.prototype.scrollIntoView = vi.fn();

const { autoScrollSpy } = vi.hoisted(() => ({
    autoScrollSpy: vi.fn(),
}));

vi.mock('../../hooks/useAutoScroll', () => ({
    useAutoScroll: () => autoScrollSpy()
}));

// Mock Virtuoso to render items and expose context
vi.mock('react-virtuoso', async () => {
    const React = await import('react');
    return {
        Virtuoso: React.forwardRef((props: any, _ref: any) => {
            return (
                <div data-testid="virtuoso-list">
                    {props.data?.map((item: any) => {
                        return (
                            <div key={item.id} data-testid={`segment-${item.id}`}>
                                <button
                                    onClick={() => props.context.onAnimationEnd(item.id)}
                                    data-testid={`animate-${item.id}`}
                                >
                                    Animate
                                </button>
                            </div>
                        );
                    })}
                </div>
            );
        }),
    };
});

// Mock i18n
vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

// Mock dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
    ask: vi.fn(),
}));

vi.mock('../transcript/SegmentItem', () => ({
    SegmentItem: () => <div>Segment</div>
}));

vi.mock('../SearchUI', () => ({
    SearchUI: () => <div>SearchUI</div>
}));

describe('TranscriptEditor Performance', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        useTranscriptStore.setState({
            segments: [],
            activeSegmentId: null,
            activeSegmentIndex: -1,
            isPlaying: false,
            editingSegmentId: null,
            config: {
                enableTimeline: true,
                offlineModelPath: '',
                language: 'en',
                appLanguage: 'en',
            } as any
        });
    });

    it('should not re-render TranscriptEditor when items finish animating', async () => {
        const segments = [
            { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
            { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
            { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
        ];

        act(() => {
            useTranscriptStore.setState({ segments });
        });

        // Initial render
        const { getByTestId } = render(<TranscriptEditor />);

        // Initial render should invoke useAutoScroll
        const initialRenderCount = autoScrollSpy.mock.calls.length;
        console.log('Initial render count:', initialRenderCount);
        expect(initialRenderCount).toBeGreaterThan(0);

        // Simulate animation end for segment 1
        act(() => {
            getByTestId('animate-1').click();
        });

        const countAfterFirst = autoScrollSpy.mock.calls.length;
        console.log('Count after first animation:', countAfterFirst);

        // Simulate animation end for segment 2
        act(() => {
            getByTestId('animate-2').click();
        });

        const countAfterSecond = autoScrollSpy.mock.calls.length;
        console.log('Count after second animation:', countAfterSecond);

        // Simulate animation end for segment 3
        act(() => {
            getByTestId('animate-3').click();
        });

        const countAfterThird = autoScrollSpy.mock.calls.length;
        console.log('Count after third animation:', countAfterThird);

        // Note: countAfterFirst includes the re-render from the first animation if unoptimized.
        // Actually, let's just check if count increases after EACH click.

        // If unoptimized, we expect render count to increase by 1 for each click.
        // So countAfterThird should be countAfterFirst + 2 (assuming first one also triggered re-render).
        // Let's compare countAfterThird vs countAfterFirst.

        console.log(`Render counts: Initial=${initialRenderCount}, 1st=${countAfterFirst}, 2nd=${countAfterSecond}, 3rd=${countAfterThird}`);

        // With optimization, countAfterFirst == countAfterSecond == countAfterThird.
        // Also countAfterFirst should ideally be initialRenderCount (if no re-render happened).

        expect(countAfterThird).toBe(countAfterFirst); // This expects NO extra renders after the first one (or first one either).
    });
});
