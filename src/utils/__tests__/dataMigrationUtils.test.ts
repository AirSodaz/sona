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

  it('passes through Lexical format (<p> prefix) unchanged', () => {
    expect(convertOldFormatToLexical('<p>Hello <strong>World</strong></p>')).toBe(
      '<p>Hello <strong>World</strong></p>',
    );
  });

  it('passes through Lexical format (<strong> prefix) unchanged', () => {
    expect(convertOldFormatToLexical('<strong>Bold text</strong>')).toBe(
      '<strong>Bold text</strong>',
    );
  });

  it('passes through Lexical format (<em> prefix) unchanged', () => {
    expect(convertOldFormatToLexical('<em>Italic text</em>')).toBe(
      '<em>Italic text</em>',
    );
  });

  it('passes through Lexical format (<u> prefix) unchanged', () => {
    expect(convertOldFormatToLexical('<u>Underlined text</u>')).toBe(
      '<u>Underlined text</u>',
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
