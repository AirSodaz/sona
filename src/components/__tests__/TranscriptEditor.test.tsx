import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import TranscriptEditor from '../TranscriptEditor';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { SegmentItem } from '../transcript/SegmentItem';

// Mock scrollTo
window.HTMLElement.prototype.scrollIntoView = vi.fn();

// Mock Virtuoso
const { mockScrollToIndex } = vi.hoisted(() => ({
    mockScrollToIndex: vi.fn(),
}));

vi.mock('react-virtuoso', async () => {
    const React = await import('react');
    return {
        Virtuoso: React.forwardRef((props: any, ref: any) => {
            React.useImperativeHandle(ref, () => ({
                scrollToIndex: mockScrollToIndex,
            }));
            return (
                <div data-testid="virtuoso-list">
                    {props.data?.map((item: any, index: number) =>
                        props.itemContent(index, item, props.context)
                    )}
                </div>
            );
        }),
    };
});

// Mock i18n
const t = (key: string) => key;
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t,
    }),
}));

// Mock dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
    ask: vi.fn(),
}));

// Mock SegmentItem for performance tracking
vi.mock('../transcript/SegmentItem', async () => {
    const React = await import('react');
    const mockFn = vi.fn(() => <div>Segment</div>);
    const Memoized = React.memo(mockFn);
    // Attach mock to the component for access in tests
    (Memoized as any).mock = mockFn;
    return {
        SegmentItem: Memoized
    };
});

describe('TranscriptEditor', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        act(() => {
            useTranscriptStore.setState({
                segments: [],
                activeSegmentId: null,
                activeSegmentIndex: -1,
                isPlaying: false,
                editingSegmentId: null
            });
        });
    });

    it('scrolls to active segment during playback', async () => {
        const segments = [
            { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
            { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
            { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
        ];

        // Set initial state
        act(() => {
            useTranscriptStore.setState({
                segments,
                activeSegmentId: '1',
                activeSegmentIndex: 0,
                isPlaying: false
            });
        });

        render(<TranscriptEditor />);

        // Start playing
        act(() => {
            useTranscriptStore.setState({ isPlaying: true });
        });

        // Changing active segment should trigger scroll
        act(() => {
            useTranscriptStore.setState({ activeSegmentId: '2', activeSegmentIndex: 1 });
        });

        expect(mockScrollToIndex).toHaveBeenCalledWith(expect.objectContaining({
            index: 1,
            align: 'center',
            behavior: 'smooth'
        }));
    });

    it('does not scroll if not playing', async () => {
        const segments = [
            { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
            { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
        ];

        act(() => {
            useTranscriptStore.setState({
                segments,
                activeSegmentId: '1',
                isPlaying: false
            });
        });

        render(<TranscriptEditor />);

        // Change active segment without playing
        act(() => {
            useTranscriptStore.setState({ activeSegmentId: '2' });
        });

        expect(mockScrollToIndex).not.toHaveBeenCalled();
    });

    it('renders only changed segments when context is stable (performance)', async () => {
        const segments = [
            { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] },
            { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] },
            { id: '3', start: 2, end: 3, text: 'Three', isFinal: true, tokens: [], timestamps: [] },
        ];

        // Initial render
        act(() => {
            useTranscriptStore.setState({ segments });
        });

        render(<TranscriptEditor />);

        const mockFn = (SegmentItem as any).mock;

        // Initial render should trigger 3 renders (one for each segment)
        expect(mockFn).toHaveBeenCalledTimes(3);
        mockFn.mockClear();

        // Update text of first segment
        act(() => {
             useTranscriptStore.getState().updateSegment('1', { text: 'One Updated' });
        });

        // With optimization, context is stable.
        // Segment 1: props changed (segment update).
        // Segment 2: props stable (same segment ref, same context).
        // Segment 3: props stable.
        // Expectation: 1
        expect(mockFn).toHaveBeenCalledTimes(1);
    });
});
