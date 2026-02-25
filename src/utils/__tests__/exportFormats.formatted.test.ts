import { toTXT, toSRT } from '../exportFormats';
import { TranscriptSegment } from '../../types/transcript';
import { describe, it, expect } from 'vitest';

describe('exportFormats with formatting', () => {
    const segments: TranscriptSegment[] = [
        {
            id: '1',
            start: 0,
            end: 1,
            text: 'Hello <b>World</b>',
            isFinal: true,
            tokens: [],
            timestamps: []
        },
        {
            id: '2',
            start: 1,
            end: 2,
            text: 'Line<br>Break',
            isFinal: true,
            tokens: [],
            timestamps: []
        }
    ];

    it('strips tags for TXT export', () => {
        const txt = toTXT(segments);
        // Expect 'Hello World\n\nLine\nBreak'
        // toTXT joins segments with \n\n.
        // Segment 1: "Hello World"
        // Segment 2: "Line\nBreak"
        expect(txt).toContain('Hello World');
        expect(txt).toContain('Line\nBreak');
        expect(txt).not.toContain('<b>');
        expect(txt).not.toContain('<br>');
    });

    it('preserves tags for SRT export', () => {
        const srt = toSRT(segments);
        // Expect 'Hello <b>World</b>'
        expect(srt).toContain('Hello <b>World</b>');
        // Expect 'Line\nBreak' (SRT uses newlines, not <br>)
        // My implementation replaces <br> with \n.
        expect(srt).toContain('Line\nBreak');
        expect(srt).not.toContain('<br>');
    });
});
