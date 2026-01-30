import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import TranscriptEditor from '../TranscriptEditor';
import { useTranscriptStore } from '../../stores/transcriptStore';

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
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

// Mock dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
    ask: vi.fn(),
}));

describe('TranscriptEditor', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        act(() => {
            useTranscriptStore.setState({
                segments: [],
                activeSegmentId: null,
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
            useTranscriptStore.setState({ activeSegmentId: '2' });
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

    it('renders empty state with action buttons and switches mode', () => {
        act(() => {
            useTranscriptStore.setState({ segments: [] });
        });

        // Spy on setMode
        const setModeSpy = vi.spyOn(useTranscriptStore.getState(), 'setMode');

        const { getByText, getByLabelText } = render(<TranscriptEditor />);

        expect(getByText('editor.empty_state')).toBeTruthy();

        const liveButton = getByLabelText('panel.live_record');
        expect(liveButton).toBeTruthy();

        act(() => {
            liveButton.click();
        });

        expect(setModeSpy).toHaveBeenCalledWith('live');

        const batchButton = getByLabelText('panel.batch_import');
        expect(batchButton).toBeTruthy();

        act(() => {
            batchButton.click();
        });

        expect(setModeSpy).toHaveBeenCalledWith('batch');
    });
});
