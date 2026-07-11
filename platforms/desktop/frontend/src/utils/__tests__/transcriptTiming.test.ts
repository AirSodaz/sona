import { describe, expect, it } from 'vitest';
import { normalizeTranscriptSegment } from '../transcriptTiming';
import type { TranscriptSegment } from '../../types/transcript';

describe('normalizeTranscriptSegment', () => {
  it('does not preserve token timing text that no longer matches edited segment text', () => {
    const segment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Edited text',
      start: 0,
      end: 2,
      isFinal: true,
      timing: {
        level: 'token',
        source: 'model',
        units: [
          { text: 'Hello', start: 0, end: 1 },
          { text: 'world', start: 1, end: 2 },
        ],
      },
      tokens: ['Hello', 'world'],
      timestamps: [0, 1],
    };

    const normalized = normalizeTranscriptSegment(segment);
    const renderedText = normalized.timing?.units.map((unit) => unit.text).join('');

    expect(renderedText).toBe('Edited text');
  });

  it('does not preserve token timing text when edited text only changes punctuation', () => {
    const segment: TranscriptSegment = {
      id: 'seg-1',
      text: 'Hello, world',
      start: 0,
      end: 2,
      isFinal: true,
      timing: {
        level: 'token',
        source: 'model',
        units: [
          { text: 'Hello', start: 0, end: 0.5 },
          { text: ' ', start: 0.5, end: 1 },
          { text: 'world', start: 1, end: 2 },
        ],
      },
      tokens: ['Hello', 'world'],
      timestamps: [0, 1],
    };

    const normalized = normalizeTranscriptSegment(segment);
    const renderedText = normalized.timing?.units.map((unit) => unit.text).join('');

    expect(renderedText).toBe('Hello, world');
  });
});
