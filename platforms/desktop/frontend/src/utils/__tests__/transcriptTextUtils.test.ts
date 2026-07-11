import { describe, expect, it } from 'vitest';
import {
  alignTextToTimedTokens,
  stripHtmlTags,
  sanitizeTranscriptHtml,
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

  it('sanitizes HTML by escaping unsafe tags, preserving text between them', () => {
    expect(sanitizeTranscriptHtml('<script>alert(1)</script>Hello')).toBe(
      '&lt;script&gt;alert(1)&lt;/script&gt;Hello'
    );
  });

  it('sanitizes HTML by removing unsafe attributes', () => {
    expect(sanitizeTranscriptHtml('<strong onclick="alert(1)">Bold</strong>')).toBe(
      '<strong>Bold</strong>',
    );
  });

  it('preserves safe tags and their class attributes', () => {
    expect(sanitizeTranscriptHtml('<strong class="editor-bold">Bold</strong>')).toBe(
      '<strong class="editor-bold">Bold</strong>',
    );
  });

  it('preserves <em> and <u> tags', () => {
    expect(sanitizeTranscriptHtml('<em>Italic</em> and <u>underline</u>')).toBe(
      '<em>Italic</em> and <u>underline</u>',
    );
  });

  it('preserves old format <b> and <i> tags', () => {
    expect(sanitizeTranscriptHtml('<b>Bold</b> <i>Italic</i>')).toBe(
      '<b>Bold</b> <i>Italic</i>',
    );
  });

  it('escapes unsafe <img> tags', () => {
    expect(sanitizeTranscriptHtml('text <img src=x onerror="alert(1)"> more')).toBe(
      'text &lt;img src=x onerror="alert(1)"&gt; more'
    );
  });

  it('handles empty input', () => {
    expect(sanitizeTranscriptHtml('')).toBe('');
  });

  it('preserves <p> tags', () => {
    expect(sanitizeTranscriptHtml('<p>Hello <strong>World</strong></p>')).toBe(
      '<p>Hello <strong>World</strong></p>',
    );
  });

  it('preserves <br> tags', () => {
    expect(sanitizeTranscriptHtml('Line1<br>Line2')).toBe('Line1<br>Line2');
  });

  it('escapes literal < followed by non-tag content', () => {
    expect(sanitizeTranscriptHtml('2 < 3 and < something > else')).toBe(
      '2 &lt; 3 and &lt; something &gt; else',
    );
  });

  it('escapes unclosed unsafe tags', () => {
    expect(sanitizeTranscriptHtml('<script src="evil.js"')).toBe(
      '&lt;script src="evil.js"'
    );
    expect(sanitizeTranscriptHtml('<img src=x onerror="alert(1)"')).toBe(
      '&lt;img src=x onerror="alert(1)"'
    );
  });

  it('handles attributes containing > correctly', () => {
    expect(sanitizeTranscriptHtml('<strong class="bold" title="a > b">Bold</strong>')).toBe(
      '<strong class="bold">Bold</strong>'
    );
    expect(sanitizeTranscriptHtml('<img src="x" onerror="if(1>2)alert(1)">')).toBe(
      '&lt;img src="x" onerror="if(1&gt;2)alert(1)"&gt;'
    );
  });

  it('handles class attributes with single and no quotes', () => {
    expect(sanitizeTranscriptHtml("<strong class='editor-bold'>Bold</strong>")).toBe(
      '<strong class="editor-bold">Bold</strong>'
    );
    expect(sanitizeTranscriptHtml('<strong class=editor-bold>Bold</strong>')).toBe(
      '<strong class="editor-bold">Bold</strong>'
    );
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
