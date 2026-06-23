import { describe, expect, it } from 'vitest';
import { convertOldFormatToLexical } from '../dataMigrationUtils';

describe('convertOldFormatToLexical', () => {
  it('converts <b> to <strong> and wraps in <p>', () => {
    expect(convertOldFormatToLexical('Hello <b>World</b>')).toBe(
      '<p>Hello <strong>World</strong></p>',
    );
  });

  it('converts <i> to <em> and wraps in <p>', () => {
    expect(convertOldFormatToLexical('Hello <i>World</i>')).toBe(
      '<p>Hello <em>World</em></p>',
    );
  });

  it('preserves <u> tags and wraps in <p>', () => {
    expect(convertOldFormatToLexical('Hello <u>World</u>')).toBe(
      '<p>Hello <u>World</u></p>',
    );
  });

  it('is idempotent for full Lexical format (re-wraps in <p>)', () => {
    expect(convertOldFormatToLexical('<p>Hello <strong>World</strong></p>')).toBe(
      '<p>Hello <strong>World</strong></p>',
    );
  });

  it('wraps Lexical-style tags without <p> wrapper', () => {
    expect(convertOldFormatToLexical('<strong>Bold text</strong>')).toBe(
      '<p><strong>Bold text</strong></p>',
    );
  });

  it('wraps <em> text in <p>', () => {
    expect(convertOldFormatToLexical('<em>Italic text</em>')).toBe(
      '<p><em>Italic text</em></p>',
    );
  });

  it('wraps <u> text in <p>', () => {
    expect(convertOldFormatToLexical('<u>Underlined text</u>')).toBe(
      '<p><u>Underlined text</u></p>',
    );
  });

  it('wraps plain text in <p>', () => {
    expect(convertOldFormatToLexical('Hello world')).toBe('<p>Hello world</p>');
  });

  it('returns empty string for empty input', () => {
    expect(convertOldFormatToLexical('')).toBe('');
  });

  it('handles mixed old format tags', () => {
    expect(convertOldFormatToLexical('<b>Bold</b> and <i>italic</i>')).toBe(
      '<p><strong>Bold</strong> and <em>italic</em></p>',
    );
  });
});
