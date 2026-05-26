import { describe, expect, it } from 'vitest';
import {
  alignTextToTimedTokens,
  stripHtmlTags,
} from '../transcriptTextUtils';

describe('transcriptTextUtils', () => {
  it('strips html tags from formatted transcript text', () => {
    expect(stripHtmlTags('<b>Hello</b> <i>world</i>')).toBe('Hello world');
  });

  it('aligns formatted text to token timing metadata', () => {
    const result = alignTextToTimedTokens(
      '<b>Hello</b>, <i>world</i>',
      [
        { text: 'hello', timing: { timestamp: 1 } },
        { text: 'world', timing: { timestamp: 2 } },
      ],
    );

    expect(result).toEqual([
      { text: '<b>Hello</b>,', timing: { timestamp: 1 } },
      { text: ' ', timing: { timestamp: 2 } },
      { text: '<i>world</i>', timing: { timestamp: 2 } },
    ]);
  });

  it('preserves arbitrary timing payloads when tokens are aligned', () => {
    const result = alignTextToTimedTokens(
      'Edited text',
      [
        { text: 'edited', timing: { start: 0, end: 0.5 } },
        { text: 'text', timing: { start: 1, end: 2 } },
      ],
    );

    expect(result).toEqual([
      { text: 'Edited', timing: { start: 0, end: 0.5 } },
      { text: ' ', timing: { start: 1, end: 2 } },
      { text: 'text', timing: { start: 1, end: 2 } },
    ]);
  });

  it('returns the full text with fallback timing when inputs cannot be aligned', () => {
    const result = alignTextToTimedTokens('Hello', [
      { text: 'hello', timing: { timestamp: 5 } },
      { text: 'extra', timing: { timestamp: 6 } },
    ]);

    expect(result).toEqual([
      { text: 'Hello', timing: { timestamp: 5 } },
    ]);
  });

  it('falls back to the last token with normalized text when trailing tokens normalize empty', () => {
    const result = alignTextToTimedTokens('Hello extra', [
      { text: 'hello', timing: { timestamp: 5 } },
      { text: '.', timing: { timestamp: 6 } },
    ]);

    expect(result).toEqual([
      { text: 'Hello', timing: { timestamp: 5 } },
      { text: ' ', timing: { timestamp: 5 } },
      { text: 'extra', timing: { timestamp: 5 } },
    ]);
  });
});
