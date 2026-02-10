import { describe, it, expect } from 'vitest';
import { toSRT, toVTT, toJSON, toTXT } from '../exportFormats';
import { TranscriptSegment } from '../../types/transcript';

describe('exportFormats', () => {
    const segments: TranscriptSegment[] = [
        {
            id: '1',
            text: 'Hello world',
            start: 0.5,
            end: 2.5,
            isFinal: true,
        },
        {
            id: '2',
            text: 'This is a test',
            start: 3.0,
            end: 5.0,
            isFinal: true,
        },
        {
            id: '3',
            text: '', // Empty text should be filtered out for SRT/VTT/TXT, but kept for JSON
            start: 6.0,
            end: 7.0,
            isFinal: true,
        },
        {
            id: '4',
            text: 'Not final',
            start: 8.0,
            end: 9.0,
            isFinal: false, // Not final should be filtered out
        },
    ];

    it('should format to SRT correctly', () => {
        // Current implementation produces blocks separated by newlines, with a trailing newline in each block
        // Block1\n\nBlock2\n
        const expected = `1
00:00:00,500 --> 00:00:02,500
Hello world

2
00:00:03,000 --> 00:00:05,000
This is a test
`;
        expect(toSRT(segments)).toBe(expected);
    });

    it('should format to VTT correctly', () => {
        // Current implementation: Header\n\nBlock1\n\nBlock2\n
        const expected = `WEBVTT

00:00:00.500 --> 00:00:02.500
Hello world

00:00:03.000 --> 00:00:05.000
This is a test
`;
        expect(toVTT(segments)).toBe(expected);
    });

    it('should format to JSON correctly', () => {
        // JSON keeps empty text segments
        const expected = [
            {
                start: 0.5,
                end: 2.5,
                text: 'Hello world',
            },
            {
                start: 3.0,
                end: 5.0,
                text: 'This is a test',
            },
            {
                start: 6.0,
                end: 7.0,
                text: '',
            },
        ];
        expect(JSON.parse(toJSON(segments))).toEqual(expected);
    });

    it('should format to TXT correctly', () => {
        const expected = `Hello world

This is a test`;
        expect(toTXT(segments)).toBe(expected);
    });
});
