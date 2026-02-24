import { describe, it, expect } from 'vitest';
import { computeSegmentsFingerprint } from '../segmentUtils';
import { TranscriptSegment } from '../../types/transcript';

describe('computeSegmentsFingerprint', () => {
    it('returns a fingerprint for a list of segments', () => {
        const segments: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: true },
            { id: '2', text: 'World', start: 1, end: 2, isFinal: true }
        ];
        const fingerprint = computeSegmentsFingerprint(segments);
        expect(fingerprint).toBe('1:Hello:0:1:true:|2:World:1:2:true:');
    });

    it('returns a different fingerprint when translation changes', () => {
        const segments1: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }
        ];
        const segments2: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: true, translation: 'Bonjour' }
        ];
        const fingerprint1 = computeSegmentsFingerprint(segments1);
        const fingerprint2 = computeSegmentsFingerprint(segments2);

        expect(fingerprint1).not.toBe(fingerprint2);
        expect(fingerprint2).toContain('Bonjour');
    });

    it('returns a different fingerprint when isFinal changes', () => {
        const segments1: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: false }
        ];
        const segments2: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }
        ];
        const fingerprint1 = computeSegmentsFingerprint(segments1);
        const fingerprint2 = computeSegmentsFingerprint(segments2);

        expect(fingerprint1).not.toBe(fingerprint2);
    });

    it('returns a different fingerprint when timing changes', () => {
        const segments1: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1, isFinal: true }
        ];
        const segments2: TranscriptSegment[] = [
            { id: '1', text: 'Hello', start: 0, end: 1.5, isFinal: true }
        ];
        const fingerprint1 = computeSegmentsFingerprint(segments1);
        const fingerprint2 = computeSegmentsFingerprint(segments2);

        expect(fingerprint1).not.toBe(fingerprint2);
    });
});
