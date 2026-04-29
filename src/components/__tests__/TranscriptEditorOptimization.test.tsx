import { render, act } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import TranscriptEditor from '../TranscriptEditor';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { TranscriptSegment } from '../../types/transcript';

// Mock scrollTo
window.HTMLElement.prototype.scrollIntoView = vi.fn();

// Mock useAutoScroll
vi.mock('../../hooks/useAutoScroll', () => ({
    useAutoScroll: vi.fn()
}));

// Mock Virtuoso to render items
vi.mock('react-virtuoso', async () => {
    const React = await import('react');
    return {
        Virtuoso: React.forwardRef((props: any) => {
            return (
                <div data-testid="virtuoso-list">
                    {props.data?.map((item: any, index: number) => (
                        // Render ItemContent or component
                        // TranscriptEditor uses `itemContent` prop which returns <SegmentItem ... />
                        // We need to call that function.
                        <React.Fragment key={item.id}>
                            {props.itemContent(index, item, props.context)}
                        </React.Fragment>
                    ))}
                </div>
            );
        }),
    };
});

// Mock i18n
vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
    initReactI18next: {
        type: '3rdParty',
        init: () => undefined,
    },
}));

// Mock dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
    ask: vi.fn(),
}));

// Mock SearchUI
vi.mock('../SearchUI', () => ({
    SearchUI: () => <div>SearchUI</div>
}));

// Mock SegmentItem to inspect isNew state
vi.mock('../transcript/SegmentItem', async () => {
    const React = await import('react');
    const { useContext } = React;
    const { useStore } = await import('zustand');
    const { createStore } = await import('zustand/vanilla');
    const { TranscriptUIContext } = await import('../transcript/TranscriptUIContext');
    const fallbackStore = createStore(() => ({ newSegmentIds: new Set<string>() }));

    return {
        SegmentItem: (props: any) => {
            const store = useContext(TranscriptUIContext);
            const resolvedStore = store ?? fallbackStore;
            const isNew = useStore(resolvedStore, (s: any) => s.newSegmentIds.has(props.segment.id));

            if (!store) return <div>No Store</div>;

            return (
                <div
                    data-testid={`segment-${props.segment.id}`}
                    data-is-new={isNew ? 'true' : 'false'}
                >
                    {props.segment.text}
                </div>
            );
        }
    };
});

describe('TranscriptEditor Optimization', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        useTranscriptStore.setState({
            segments: [],
            activeSegmentId: null,
            editingSegmentId: null,
            aligningSegmentIds: new Set(),
            config: {
                enableTimeline: true,
                streamingModelPath: "/path/to/model",
                offlineModelPath: '',
                language: 'en',
                appLanguage: 'en',
            } as any
        });
    });

    it('should suppress animations for bulk load (> 50 segments)', async () => {
        // Create 100 segments
        const segments: TranscriptSegment[] = Array.from({ length: 100 }, (_, i) => ({
            id: `seg-${i}`,
            start: i,
            end: i + 1,
            text: `Segment ${i}`,
            isFinal: true,
            tokens: [],
            timestamps: []
        }));

        act(() => {
            useTranscriptStore.setState({ segments });
        });

        const { getByTestId } = render(<TranscriptEditor />);

        // Verify first segment
        const segment0 = getByTestId('segment-seg-0');

        // BEFORE FIX: Should be 'true' (all are new)
        // AFTER FIX: Should be 'false' (bulk load suppressed)

        // Assert failure (expecting 'false' but getting 'true' currently)
        expect(segment0.getAttribute('data-is-new')).toBe('false');

        // Verify last segment
        const segment99 = getByTestId('segment-seg-99');
        expect(segment99.getAttribute('data-is-new')).toBe('false');
    });

    it('should animate small additions', async () => {
        const segments: TranscriptSegment[] = [
            { id: '1', start: 0, end: 1, text: 'One', isFinal: true, tokens: [], timestamps: [] }
        ];

        act(() => {
            useTranscriptStore.setState({ segments });
        });

        const { getByTestId } = render(<TranscriptEditor />);

        // First item might be considered new?
        // With optimization, size=1 (<50), so it is 'true'.
        // Wait, initial load of small list SHOULD animate?
        // Or logic: "If newIds.size > 50".
        // Size 1 is not > 50. So it remains "new".
        // So initial load of small list animates. This is acceptable/current behavior.

        const seg1 = getByTestId('segment-1');
        expect(seg1.getAttribute('data-is-new')).toBe('true');

        // Now add another segment
        const newSegments = [
            ...segments,
            { id: '2', start: 1, end: 2, text: 'Two', isFinal: true, tokens: [], timestamps: [] }
        ];

        act(() => {
            useTranscriptStore.setState({ segments: newSegments });
        });

        // Rerender happens automatically due to store update

        const seg2 = getByTestId('segment-2');
        expect(seg2.getAttribute('data-is-new')).toBe('true');

        // Segment 1 should still be 'true' if it hasn't animated yet.
        // In this test environment, animationEnd never fires unless we trigger it.
        // So both are 'true'.
        expect(getByTestId('segment-1').getAttribute('data-is-new')).toBe('true');
    });
});
