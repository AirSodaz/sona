import { describe, it, expect } from 'vitest';
import { exportSegments } from '../exportFormats';
import { TranscriptSegment } from '../../types/transcript';

describe('exportFormats', () => {
    const segments: TranscriptSegment[] = [
        {
            id: '1',
            start: 0,
            end: 2.5,
            text: 'Hello world',
            isFinal: true,
            translation: 'Bonjour le monde'
        },
        {
            id: '2',
            start: 3.0,
            end: 5.0,
            text: 'How are you?',
            isFinal: true,
            // Missing translation to test fallback/empty behavior
        },
        {
            id: '3',
            start: 6.0,
            end: 8.0,
            text: 'I am fine.',
            isFinal: true,
            translation: 'Je vais bien.'
        }
    ];

    describe('SRT Export', () => {
        it('should export original text correctly', () => {
            const result = exportSegments(segments, 'srt', 'original');
            expect(result).toContain('Hello world');
            expect(result).not.toContain('Bonjour le monde');
            expect(result).toContain('How are you?');
        });

        it('should export translation correctly', () => {
            const result = exportSegments(segments, 'srt', 'translation');
            expect(result).toContain('Bonjour le monde');
            expect(result).not.toContain('Hello world');
            // Segment 2 has no translation, should be empty or skipped?
            // Current plan: "If missing, output empty string."
            // In SRT, an empty subtitle might look weird but let's see implementation.
            // Wait, if text is empty, toSRT usually filters it out.
            // Let's assume we want to keep the timing but have empty text, or just skip?
            // Existing `toSRT` filters: `seg.text.trim().length > 0`.
            // So for translation mode, if translation is missing/empty, it should likely be filtered out too to avoid empty subtitles.
        });

        it('should export bilingual correctly', () => {
            const result = exportSegments(segments, 'srt', 'bilingual');
            // Line 1: Translation, Line 2: Original
            expect(result).toContain('Bonjour le monde\nHello world');
            expect(result).toContain('Je vais bien.\nI am fine.');
            // For segment 2 (missing translation):
            // Should be just "How are you?" on second line? Or empty line first?
            // "If missing, output empty string." -> "\nHow are you?"
            expect(result).toContain('\nHow are you?');
        });
    });

    describe('VTT Export', () => {
        it('should export bilingual correctly', () => {
            const result = exportSegments(segments, 'vtt', 'bilingual');
            expect(result).toContain('WEBVTT');
            expect(result).toContain('Bonjour le monde\nHello world');
        });
    });

    describe('TXT Export', () => {
        it('should export original text correctly', () => {
            const result = exportSegments(segments, 'txt', 'original');
            expect(result).toBe('Hello world\n\nHow are you?\n\nI am fine.');
        });

        it('should export translation correctly', () => {
            const result = exportSegments(segments, 'txt', 'translation');
            // Should filter out empty translations?
            // "If missing, output empty string."
            // If we map to translation and join with \n\n, we might get holes.
            // Let's assume we want to preserve structure or just list translations.
            // Existing `toTXT` filters empty segments.
            expect(result).toContain('Bonjour le monde');
            expect(result).toContain('Je vais bien.');
            expect(result).not.toContain('How are you?');
        });

        it('should export bilingual correctly', () => {
            const result = exportSegments(segments, 'txt', 'bilingual');
            // "Original line, followed by Translated line"
            // Wait, plan said: "TXT: Format as `text` + newline + `translation`."
            // Actually user said: "Original line, followed by Translated line? Yes."
            // So:
            // Hello world
            // Bonjour le monde
            //
            // How are you?
            //
            // I am fine.
            // Je vais bien.
            expect(result).toContain('Hello world\nBonjour le monde');
            expect(result).toContain('How are you?\n'); // Empty translation
            expect(result).toContain('I am fine.\nJe vais bien.');
        });
    });

    describe('JSON Export', () => {
        it('should export original correctly', () => {
            const result = exportSegments(segments, 'json', 'original');
            const data = JSON.parse(result);
            expect(data[0].text).toBe('Hello world');
            expect(data[0].translation).toBeUndefined();
        });

        it('should export translation correctly', () => {
            const result = exportSegments(segments, 'json', 'translation');
            const data = JSON.parse(result);
            expect(data[0].text).toBe('Bonjour le monde');
            expect(data[0].translation).toBeUndefined();
        });

        it('should export bilingual correctly', () => {
            const result = exportSegments(segments, 'json', 'bilingual');
            const data = JSON.parse(result);
            expect(data[0].text).toBe('Hello world');
            expect(data[0].translation).toBe('Bonjour le monde');
            expect(data[1].text).toBe('How are you?');
            // undefined or null or empty string?
            expect(data[1].translation).toBeUndefined(); // or check if key is missing
        });
    });
});
