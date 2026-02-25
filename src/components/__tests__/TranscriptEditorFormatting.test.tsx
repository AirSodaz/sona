import { render, screen } from '@testing-library/react';
import { TranscriptEditor } from '../TranscriptEditor';
import { useTranscriptStore } from '../../stores/transcriptStore';
import { vi, describe, it, expect, beforeEach } from 'vitest';

// Mock i18next
vi.mock('react-i18next', () => ({
    useTranslation: () => ({
        t: (key: string) => key,
    }),
}));

// Mock virtualization
vi.mock('react-virtuoso', () => ({
    Virtuoso: ({ itemContent, data, context }: any) => (
        <div>
            {data.map((item: any, index: number) => (
                <div key={item.id} data-testid="virtuoso-item">
                    {itemContent(index, item, context)}
                </div>
            ))}
        </div>
    ),
}));

// Mock SegmentItem to avoid full complexity?
// No, we want to test SegmentItem -> SegmentTokens integration.
// But SegmentItem uses ScrollIntoView?
// SegmentItem uses icons.
// Icons might need mocking if they use assets? usually SVG components. fine.

describe('TranscriptEditor Formatting', () => {
    beforeEach(() => {
        // Reset store
        useTranscriptStore.setState({
            segments: [],
            activeSegmentId: null,
            editingSegmentId: null,
        });
    });

    it('renders formatted text in view mode', () => {
        useTranscriptStore.setState({
            segments: [
                {
                    id: '1',
                    start: 0,
                    end: 1,
                    text: 'Hello <b>World</b>',
                    isFinal: true,
                    tokens: [],
                    timestamps: []
                }
            ]
        });

        render(<TranscriptEditor />);

        // "Hello" might be separate token or merged depending on lexer.
        // "Hello " (space) "<b>World</b>".
        // Expect "World" to be rendered.
        const world = screen.getByText('World');
        expect(world.tagName).toBe('B');
    });

    it('renders newline in view mode', () => {
         useTranscriptStore.setState({
            segments: [
                {
                    id: '2',
                    start: 0,
                    end: 1,
                    text: 'Line\nBreak',
                    isFinal: true,
                    tokens: [],
                    timestamps: []
                }
            ]
         });

         render(<TranscriptEditor />);

         // Find the paragraph container
         // Text is likely split into "Line", "\n", "Break" tokens.
         // Or "Line\nBreak" if no match?
         // Lexer splits by whitespace.
         // "Line", "\n", "Break".
         // Rendered as spans.
         // Container <p> should have pre-wrap.

         // getByText might behave weirdly with multiple spans.
         // Let's find the p tag.
         const p = document.querySelector('.segment-text') as HTMLElement;
         expect(p).not.toBeNull();
         expect(p.style.whiteSpace).toBe('pre-wrap');
    });
});
