import { describe, it, expect, beforeEach } from 'vitest';
import { StreamLineBuffer } from '../streamBuffer';

describe('StreamLineBuffer', () => {
    let buffer: StreamLineBuffer;

    beforeEach(() => {
        buffer = new StreamLineBuffer();
    });

    it('should handle complete lines in a single chunk', () => {
        const lines = buffer.process('line1\nline2\n');
        expect(lines).toEqual(['line1', 'line2']);
        expect(buffer.flush()).toEqual([]);
    });

    it('should handle split chunks', () => {
        const lines1 = buffer.process('li');
        expect(lines1).toEqual([]);

        const lines2 = buffer.process('ne1\n');
        expect(lines2).toEqual(['line1']);
    });

    it('should handle multiple lines with partial end', () => {
        const lines = buffer.process('line1\nline2\npartial');
        expect(lines).toEqual(['line1', 'line2']);

        const flushed = buffer.flush();
        expect(flushed).toEqual(['partial']);
    });

    it('should handle JSON chunks correctly', () => {
        // Simulate a JSON object split across chunks
        const part1 = '{"id": 1, ';
        const part2 = '"text": "hello"}\n';

        expect(buffer.process(part1)).toEqual([]);
        expect(buffer.process(part2)).toEqual(['{"id": 1, "text": "hello"}']);
    });

    it('should ignore empty lines from split if multiple newlines', () => {
        const lines = buffer.process('line1\n\nline2\n');
        expect(lines).toEqual(['line1', '', 'line2']);
    });

    it('should handle window style line endings if necessary (not strictly required but good)', () => {
        // split('\n') leaves \r at the end of the line. The consumer should trim() if needed.
        // But let's verify behavior.
        const lines = buffer.process('line1\r\nline2\r\n');
        expect(lines).toEqual(['line1\r', 'line2\r']);
    });
});
